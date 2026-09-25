//! The solve pipeline: spec in, [`Estimate`] out. Runs the hover anchor
//! (multirotor), sweeps level-flight speeds, and collects feasibility
//! issues into the estimate rather than erroring — the UI renders them.
//!
//! Current bookkeeping: *phase* current (motor-side) drives the throttle
//! and motor-rating checks; *pack* current (battery-side) comes from the
//! power balance `P_shaft/η/V` and drives endurance and the C-rating
//! checks. Conflating the two would misstate both.

use crate::physics::{
    level_flight_power, polar_drag, stall_speed_ms, Constants, HoverPoint, MotorModel, PowerTrain,
    PropModel,
};
use crate::preset::{AirframeKind, BuildSpec};

/// A speed point on the flight envelope sweep (constant-speed flight).
#[derive(Debug, Clone, Copy)]
pub struct SpeedPoint {
    pub speed_kmh: f64,
    /// Total electrical power draw, W.
    pub power_w: f64,
    /// Endurance flying at this speed, minutes.
    pub endurance_min: f64,
    /// Distance covered on the usable energy budget at this speed, km.
    pub range_km: f64,
    /// True when the drivetrain can actually hold this speed.
    pub feasible: bool,
}

/// Anything the builder should know about: hard infeasibilities
/// (`SolveIssue::Error`) and margin advisories (`SolveIssue::Warning`).
#[derive(Debug, Clone, PartialEq)]
pub enum SolveIssue {
    Error(String),
    Warning(String),
}

/// The result of solving one [`BuildSpec`].
#[derive(Debug, Clone)]
pub struct Estimate {
    pub feasible: bool,
    pub issues: Vec<SolveIssue>,
    pub all_up_weight_g: f64,
    /// Nominal pack energy, Wh.
    pub battery_energy_wh: f64,
    /// What the usable-fraction cutoff leaves to fly with, Wh.
    pub usable_energy_wh: f64,
    /// Maximum continuous current the pack is rated for (C × capacity), A.
    pub pack_max_current_a: f64,

    // Multirotor only (None for fixed wing).
    /// Hover pack current, A (battery-side).
    pub hover_current_a: Option<f64>,
    pub hover_throttle_pct: Option<f64>,
    pub hover_endurance_min: Option<f64>,
    /// Static thrust-to-weight ratio at the motor's continuous current.
    pub thrust_to_weight: Option<f64>,
    /// All-up weight per total rotor disk area, g/dm².
    pub disc_loading_g_dm2: Option<f64>,

    // Fixed wing only (None for multirotor).
    pub stall_speed_kmh: Option<f64>,
    /// Wing loading, g/dm².
    pub wing_loading_g_dm2: Option<f64>,

    /// Best-endurance point from the sweep.
    pub best_endurance_min: f64,
    pub best_endurance_speed_kmh: f64,
    /// Best-range point from the sweep (max distance on the energy budget).
    pub max_range_km: f64,
    pub best_range_speed_kmh: f64,

    /// The requested cruise speed's numbers (defaults to the best-range
    /// speed when the spec asks for 0 or an unflyable speed).
    pub cruise_speed_kmh: f64,
    pub cruise_current_a: f64,
    pub cruise_endurance_min: f64,
    pub cruise_range_km: f64,

    /// The flight envelope for charting, ascending by speed.
    pub speed_slice: Vec<SpeedPoint>,
}

impl Estimate {
    fn push_error(&mut self, msg: impl Into<String>) {
        self.feasible = false;
        self.issues.push(SolveIssue::Error(msg.into()));
    }
    fn push_warning(&mut self, msg: impl Into<String>) {
        self.issues.push(SolveIssue::Warning(msg.into()));
    }
}

/// Structural input errors that make a solve meaningless.
fn validate(spec: &BuildSpec) -> Vec<String> {
    let mut errors = Vec::new();
    let positive = [
        ("motor Kv", spec.motor.kv_rpm_v),
        ("motor resistance", spec.motor.resistance_ohm),
        ("prop diameter", spec.prop.diameter_in),
        ("prop pitch", spec.prop.pitch_in),
        ("battery capacity", spec.battery.capacity_mah),
        ("battery weight", spec.battery.weight_g),
        ("airframe weight", spec.airframe_g),
    ];
    for (label, value) in positive {
        if value <= 0.0 {
            errors.push(format!("{label} must be positive"));
        }
    }
    if spec.motor_count == 0 {
        errors.push("motor count must be at least 1".to_string());
    }
    let usable = spec.battery.usable_fraction;
    if !(0.05..=1.0).contains(&usable) {
        errors.push("battery usable fraction must be between 0.05 and 1".to_string());
    }
    match spec.kind {
        AirframeKind::FixedWing => {
            if spec.wing_area_m2 <= 0.0 {
                errors.push("wing area must be positive for a fixed-wing build".to_string());
            }
        }
        AirframeKind::Multirotor => {}
    }
    errors
}

/// Pack current from a shaft-power draw: the battery-side current the
/// endurance math (and the C-rating) actually sees.
fn pack_current_a(shaft_power_w: f64, spec: &BuildSpec) -> f64 {
    shaft_power_w / Constants::MOTOR_EFFICIENCY / spec.pack_voltage_v()
}

/// Endurance in minutes at a given pack current.
fn endurance_min(pack_current_a: f64, usable_capacity_ah: f64) -> f64 {
    if pack_current_a > 0.0 {
        usable_capacity_ah / pack_current_a * 60.0
    } else {
        0.0
    }
}

/// Solves one build configuration.
pub fn solve(spec: &BuildSpec) -> Estimate {
    let mut est = Estimate {
        feasible: true,
        issues: Vec::new(),
        all_up_weight_g: spec.all_up_weight_g(),
        battery_energy_wh: spec.pack_voltage_v() * spec.battery.capacity_mah / 1000.0,
        usable_energy_wh: 0.0,
        pack_max_current_a: spec.battery.c_rating * spec.battery.capacity_mah / 1000.0,
        hover_current_a: None,
        hover_throttle_pct: None,
        hover_endurance_min: None,
        thrust_to_weight: None,
        disc_loading_g_dm2: None,
        stall_speed_kmh: None,
        wing_loading_g_dm2: None,
        best_endurance_min: 0.0,
        best_endurance_speed_kmh: 0.0,
        max_range_km: 0.0,
        best_range_speed_kmh: 0.0,
        cruise_speed_kmh: 0.0,
        cruise_current_a: 0.0,
        cruise_endurance_min: 0.0,
        cruise_range_km: 0.0,
        speed_slice: Vec::new(),
    };

    // Structural errors make the solve meaningless; report the first one.
    if let Some(error) = validate(spec).first() {
        est.push_error(error.clone());
        return est;
    }

    let usable_fraction = spec.battery.usable_fraction;
    est.usable_energy_wh = est.battery_energy_wh * usable_fraction;
    // Capacity the flight may actually spend, Ah.
    let usable_capacity_ah = spec.battery.capacity_mah / 1000.0 * usable_fraction;

    let prop = PropModel::new(&spec.prop);
    let motor = MotorModel::new(&spec.motor);
    let train = PowerTrain::new(&spec.battery, &motor, spec.motor_count);
    let propulsive_efficiency = match spec.kind {
        AirframeKind::Multirotor => 0.75,
        AirframeKind::FixedWing => 0.70,
    };

    // ---- hover anchor (multirotor): the calibration point for the sweep ----
    let mut hover_sag_ratio = 1.0;
    let hover = match spec.kind {
        AirframeKind::Multirotor => {
            let weight_n = est.all_up_weight_g * Constants::G / 1000.0;
            let thrust_per_motor_n = weight_n / spec.motor_count as f64;
            let revs = prop.revs_for_thrust(thrust_per_motor_n);
            let shaft_per_motor_w = prop.shaft_power(revs);
            match train.solve_point(revs, shaft_per_motor_w) {
                Some(point) => {
                    let shaft_power_total_w = shaft_per_motor_w * spec.motor_count as f64;
                    let i_pack = pack_current_a(shaft_power_total_w, spec);
                    est.hover_current_a = Some(i_pack);
                    est.hover_throttle_pct = Some(point.throttle * 100.0);
                    est.hover_endurance_min = Some(endurance_min(i_pack, usable_capacity_ah));
                    // Static thrust ceiling at ~85% of the motor's phase
                    // current rating, through the prop curve.
                    let i_phase_max = if spec.motor.max_current_a > 0.0 {
                        spec.motor.max_current_a
                    } else {
                        point.phase_current_a * 3.0
                    };
                    let p_shaft_max_per_motor =
                        0.85 * i_phase_max * point.v_sag.max(1.0) * Constants::MOTOR_EFFICIENCY;
                    let static_max_thrust_n = prop
                        .thrust(prop.revs_for_power(p_shaft_max_per_motor))
                        * spec.motor_count as f64;
                    est.thrust_to_weight = Some(static_max_thrust_n / weight_n);
                    let disc_area_dm2 = prop.disk_area(spec.motor_count) * 100.0;
                    if disc_area_dm2 > 0.0 {
                        est.disc_loading_g_dm2 = Some(est.all_up_weight_g / disc_area_dm2);
                    }
                    hover_sag_ratio = point.v_sag / spec.pack_voltage_v();
                    Some(HoverPoint {
                        rev_per_s: revs,
                        shaft_power_total_w,
                        phase_current_a: point.phase_current_a,
                        throttle: point.throttle,
                        v_sag: point.v_sag,
                    })
                }
                None => {
                    est.push_error(
                        "cannot hover: the drivetrain cannot produce the hover point below \
                         full throttle — more cells, a lower Kv, or a bigger prop is needed",
                    );
                    return est;
                }
            }
        }
        AirframeKind::FixedWing => None,
    };
    let hover = hover.unwrap_or(HoverPoint {
        rev_per_s: 0.0,
        shaft_power_total_w: 0.0,
        phase_current_a: 0.0,
        throttle: 0.0,
        v_sag: spec.pack_voltage_v(),
    });

    // ---- speed sweep ----
    let (v_lo, v_hi) = match spec.kind {
        AirframeKind::Multirotor => (2.0, 40.0),
        AirframeKind::FixedWing => (4.0, 45.0),
    };
    let steps = 96;
    let mut points = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let speed = v_lo + (v_hi - v_lo) * i as f64 / steps as f64;
        let (revs, elec_power_w) =
            level_flight_power(spec, &prop, speed, &hover, propulsive_efficiency);
        // Phase-side flyability: the throttle demanded at this power must
        // stay under 100% (multirotor revs; fixed wing passes revs=0 since
        // its feasibility is the polar's stall gate, checked implicitly by
        // polar_drag returning None → NaN power → filtered below).
        let phase_ok = if elec_power_w.is_finite() && elec_power_w > 0.0 {
            match train.solve_point(revs, elec_power_w / spec.motor_count as f64) {
                Some(point) => point.throttle <= 1.0,
                None => false,
            }
        } else {
            false
        };
        let sp = if phase_ok {
            let i_pack = elec_power_w / spec.pack_voltage_v();
            let endurance_min = endurance_min(i_pack, usable_capacity_ah);
            SpeedPoint {
                speed_kmh: speed * 3.6,
                power_w: elec_power_w,
                endurance_min,
                range_km: speed * 3.6 * (endurance_min / 60.0),
                feasible: true,
            }
        } else {
            SpeedPoint {
                speed_kmh: speed * 3.6,
                power_w: 0.0,
                endurance_min: 0.0,
                range_km: 0.0,
                feasible: false,
            }
        };
        points.push(sp);
    }

    // Best points from the flyable part of the envelope: max endurance
    // (the min-power speed) and max range (the max v·t speed).
    let flyable: Vec<&SpeedPoint> =
        points.iter().filter(|p| p.feasible && p.endurance_min > 0.0).collect();
    if flyable.is_empty() {
        est.push_error(
            "no flyable speed found: the drivetrain cannot sustain level flight anywhere in \
             the sweep — check motor/prop/battery combination",
        );
        est.speed_slice = points;
        return est;
    }
    let best_endurance = *flyable
        .iter()
        .copied()
        .max_by(|a, b| a.endurance_min.total_cmp(&b.endurance_min))
        .unwrap();
    let best_range = *flyable
        .iter()
        .copied()
        .max_by(|a, b| a.range_km.total_cmp(&b.range_km))
        .unwrap();
    est.best_endurance_min = best_endurance.endurance_min;
    est.best_endurance_speed_kmh = best_endurance.speed_kmh;
    est.max_range_km = best_range.range_km;
    est.best_range_speed_kmh = best_range.speed_kmh;

    // Cruise line: the requested speed if flyable, else the best-range point.
    let requested_kmh = spec.cruise_speed_ms * 3.6;
    let cruise = if requested_kmh > 1.0 {
        *flyable
            .iter()
            .copied()
            .min_by(|a, b| {
                (a.speed_kmh - requested_kmh)
                    .abs()
                    .total_cmp(&(b.speed_kmh - requested_kmh).abs())
            })
            .unwrap()
    } else {
        best_range
    };
    est.cruise_speed_kmh = cruise.speed_kmh;
    est.cruise_current_a = cruise.power_w / spec.pack_voltage_v();
    est.cruise_endurance_min = cruise.endurance_min;
    est.cruise_range_km = cruise.range_km;

    // Fixed-wing numbers.
    if spec.kind == AirframeKind::FixedWing {
        est.stall_speed_kmh = stall_speed_ms(spec).map(|v| v * 3.6);
        let wing_area_dm2 = spec.wing_area_m2 * 100.0;
        if wing_area_dm2 > 0.0 {
            est.wing_loading_g_dm2 = Some(est.all_up_weight_g / wing_area_dm2);
        }
    }

    // ---- feasibility checks & advisories ----
    // Sustained operating points this app actually recommends (hover + the
    // best-endurance/best-range/cruise lines) — sprinting to the envelope's
    // fastest flyable point briefly is normal flying and must not flag a
    // continuous rating.
    let sustained_pack_currents: Vec<f64> = {
        let mut v = vec![
            est.cruise_current_a,
            best_endurance.power_w / spec.pack_voltage_v(),
            best_range.power_w / spec.pack_voltage_v(),
        ];
        if let Some(hover_i) = est.hover_current_a {
            v.push(hover_i);
        }
        v
    };

    let hover_throttle = est.hover_throttle_pct.unwrap_or(0.0);
    if hover_throttle > 90.0 {
        est.push_error(format!(
            "hover throttle {hover_throttle:.0}% — no power reserve left for control; this build cannot fly safely"
        ));
    } else if hover_throttle > 70.0 {
        est.push_warning(format!(
            "hover throttle {hover_throttle:.0}% — thin power reserve; expect poor handling and short motor life"
        ));
    }
    if let Some(twr) = est.thrust_to_weight {
        if twr < 1.15 {
            est.push_error(format!(
                "static thrust-to-weight {twr:.2}:1 — below the ~1.15 minimum for controlled multirotor flight"
            ));
        } else if twr < 1.6 {
            est.push_warning(format!(
                "static thrust-to-weight {twr:.2}:1 — flyable but sluggish; 2:1+ is the usual target"
            ));
        }
    }

    // Motor phase-current ceiling at hover (the worst sustained phase load).
    // Fixed wing has no hover anchor; its cruise currents are checked via
    // the pack C-rating above.
    if spec.kind == AirframeKind::Multirotor && spec.motor.max_current_a > 0.0 {
        if let Some(hover_point) = train.solve_point(hover.rev_per_s, hover.shaft_power_total_w / spec.motor_count as f64) {
            let i_phase = hover_point.phase_current_a;
            if i_phase > spec.motor.max_current_a {
                est.push_error(format!(
                    "hover draws {i_phase:.0} A phase current per motor — over the {:.0} A continuous rating",
                    spec.motor.max_current_a
                ));
            } else if i_phase > spec.motor.max_current_a * 0.8 {
                est.push_warning(format!(
                    "hover draws {i_phase:.0} A phase current per motor — over 80% of the {:.0} A continuous rating",
                    spec.motor.max_current_a
                ));
            }
        }
    }

    // Battery C-rating check across the sustained operating points.
    let capacity_ah = spec.battery.capacity_mah / 1000.0;
    if spec.battery.c_rating > 0.0 {
        let worst_pack_c = sustained_pack_currents
            .iter()
            .copied()
            .fold(0.0_f64, f64::max)
            / capacity_ah;
        if worst_pack_c > spec.battery.c_rating {
            est.push_error(format!(
                "pack sees ~{worst_pack_c:.0}C sustained — above its {0:.0}C continuous rating",
                spec.battery.c_rating
            ));
        }
    }

    // Sag advisory: high pack resistance shows up as voltage droop at hover.
    if spec.kind == AirframeKind::Multirotor {
        let sag_pct = (1.0 - hover_sag_ratio) * 100.0;
        if sag_pct > 12.0 {
            est.push_warning(format!(
                "pack voltage sags {sag_pct:.0}% at hover — internal resistance is high for this current draw"
            ));
        }
    }

    // Fixed-wing stall margins.
    if let Some(stall) = est.stall_speed_kmh {
        if cruise.speed_kmh < stall * 1.2 {
            est.push_error(format!(
                "cruise {0:.1} km/h is under 1.2× stall ({stall:.1} km/h) — the wing cannot fly this loading at that speed",
                cruise.speed_kmh
            ));
        }
    }

    est.speed_slice = points;
    est
}

/// Convenience: level-flight drag at a speed (used by tests and the Pro UI).
pub fn drag_at(spec: &BuildSpec, speed_ms: f64) -> Option<f64> {
    polar_drag(spec, speed_ms)
}
