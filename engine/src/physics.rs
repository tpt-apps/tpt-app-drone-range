//! Physics core: propeller coefficients, DC-motor equations, pack sag, and
//! the two airframe power models. Pure math, no I/O, host-testable.
//!
//! Calibration anchors (public bench data, see tests/validation.rs):
//! momentum-theory ideal hover power scaled by a figure of merit in the
//! 0.45-0.60 band reproduces measured hover draws from sub-250 g camera
//! drones through 10" camera quads within the ±30% accuracy class, and the
//! DC-motor equations reproduce the throttle/phase-current behaviour
//! bench tests show at those points.

use crate::preset::{AirframeKind, BatterySpec, BuildSpec, MotorSpec, PropSpec};

/// ISA sea-level constants and unit conversions used across the model.
pub struct Constants;

impl Constants {
    pub const G: f64 = 9.80665;
    pub const RHO: f64 = crate::AIR_DENSITY;
    /// Nominal voltage per Li/LiPo cell, V.
    pub const CELL_V: f64 = 3.7;
    /// Combined motor + ESC efficiency: electrical power in / shaft power
    /// out at cruise-class loads. Hobby BLDC benchmarks land in the
    /// 0.70-0.85 band; the model takes the conservative middle.
    pub const MOTOR_EFFICIENCY: f64 = 0.75;
    /// RPM → rad/s.
    const RPM_TO_RAD_S: f64 = std::f64::consts::PI * 2.0 / 60.0;

    /// Kv rpm/V → rad/s/V (the inverse of Kt, N·m/A).
    pub fn kv_si(kv_rpm_v: f64) -> f64 {
        kv_rpm_v * Self::RPM_TO_RAD_S
    }
}

/// Propeller static coefficient model.
///
/// `T = Ct·ρ·n²·D⁴` and `P = Cp·ρ·n³·D⁵` with n in rev/s, D in m, ρ in
/// kg/m³. Ct follows the empirical band real RC props measure at static
/// conditions (Ct ≈ 0.30-0.45 across pitch/diameter ratios), and Cp is
/// derived from Ct through the static figure of merit
/// `FM = (Ct^1.5/Cp)·√½` with FM ≈ 0.45-0.70 — the band real props span
/// between draggy tri-blades and clean slow-flyers. Blade-count factors
/// lift both coefficients for tri-blades. This reproduces measured bench
/// thrust/RPM/power triples to the ±10-15% class for conventional props.
pub struct PropModel {
    /// Diameter, m.
    pub diameter_m: f64,
    pub ct: f64,
    pub cp: f64,
}

impl PropModel {
    pub fn new(prop: &PropSpec) -> Self {
        let diameter_m = prop.diameter_in * 0.0254;
        // Pitch/diameter ratio drives how aggressively the blade bites.
        let p = if prop.diameter_in > 0.0 {
            (prop.pitch_in / prop.diameter_in).clamp(0.25, 1.2)
        } else {
            0.5
        };
        let blades = prop.blades.clamp(2, 6) as f64;
        let two_blade_ct = (0.36 + 0.15 * (p - 0.5)).clamp(0.30, 0.45);
        let fm = (0.52 - 0.08 * (p - 0.5)).clamp(0.42, 0.67);
        let two_blade_cp = (two_blade_ct.powf(1.5) * std::f64::consts::FRAC_1_SQRT_2 / fm)
            .clamp(0.16, 0.45);
        let ct = two_blade_ct * (1.0 + 0.12 * (blades - 2.0));
        let cp = two_blade_cp * (1.0 + 0.25 * (blades - 2.0));
        Self { diameter_m, ct, cp }
    }

    /// Total rotor disk area for `motor_count` of this prop, m².
    pub fn disk_area(&self, motor_count: u32) -> f64 {
        std::f64::consts::PI * (self.diameter_m / 2.0).powi(2) * motor_count as f64
    }

    /// Static thrust at `rev_per_s`, N.
    pub fn thrust(&self, rev_per_s: f64) -> f64 {
        self.ct * Constants::RHO * rev_per_s.powi(2) * self.diameter_m.powi(4)
    }

    /// Shaft power at `rev_per_s`, W.
    pub fn shaft_power(&self, rev_per_s: f64) -> f64 {
        self.cp * Constants::RHO * rev_per_s.powi(3) * self.diameter_m.powi(5)
    }

    /// Rev/s needed for `thrust_n` of static thrust.
    pub fn revs_for_thrust(&self, thrust_n: f64) -> f64 {
        if thrust_n <= 0.0 {
            return 0.0;
        }
        (thrust_n / (self.ct * Constants::RHO * self.diameter_m.powi(4))).sqrt()
    }

    /// Rev/s at which the prop absorbs `power_w` of shaft power (the
    /// operating point the phase-current solve must be consistent with).
    pub fn revs_for_power(&self, power_w: f64) -> f64 {
        if power_w <= 0.0 {
            return 0.0;
        }
        (power_w / (self.cp * Constants::RHO * self.diameter_m.powi(5))).powf(1.0 / 3.0)
    }
}

/// One brushless motor, modelled by the classic DC-machine equations:
/// back-EMF E = ω/Kv_si and torque current I_t = P_shaft/E, plus the
/// no-load current branch for bearing/aero friction. This gives the
/// *phase* current (what heats the windings and sets the throttle) — the
/// pack-side current comes from the power balance in `estimate`.
pub struct MotorModel<'a> {
    pub motor: &'a MotorSpec,
    /// Kv in rad/s/V.
    kv_si: f64,
}

impl<'a> MotorModel<'a> {
    pub fn new(motor: &'a MotorSpec) -> Self {
        Self { kv_si: Constants::kv_si(motor.kv_rpm_v), motor }
    }

    /// Phase current (A) a motor draws spinning at `omega` (rad/s) while
    /// producing `shaft_power_w`, at applied (post-throttle, post-sag)
    /// voltage `v_applied`. `None` when unreachable: back-EMF at or above
    /// applied voltage means the point needs more than full throttle.
    pub fn phase_current_at(&self, omega: f64, shaft_power_w: f64, v_applied: f64) -> Option<f64> {
        if shaft_power_w <= 0.0 {
            return Some(self.motor.no_load_current_a);
        }
        let back_emf = omega / self.kv_si;
        if back_emf <= 0.0 || back_emf >= v_applied {
            return None;
        }
        Some(shaft_power_w / back_emf + self.motor.no_load_current_a)
    }

    /// Throttle fraction (0–1) that produces the applied voltage the motor
    /// needs (back-EMF + IR drop) out of the nominal pack voltage.
    /// `None` = unreachable (would need more than 100% throttle).
    pub fn throttle_at(&self, omega: f64, phase_current_a: f64, v_pack: f64) -> Option<f64> {
        if v_pack <= 0.0 {
            return None;
        }
        let back_emf = omega / self.kv_si;
        Some((back_emf + phase_current_a * self.motor.resistance_ohm) / v_pack)
    }
}

/// One converged phase-side operating point.
#[derive(Debug, Clone, Copy)]
pub struct OperatingPoint {
    /// Per-motor phase current, A (winding-side, not pack-side).
    pub phase_current_a: f64,
    /// Throttle fraction (0–1); >1 means unreachable.
    pub throttle: f64,
    /// Sagged pack voltage, V.
    pub v_sag: f64,
}

/// The phase-side electrical chain: `motor_count` motors on one pack,
/// sagging the pack voltage under total phase-current draw.
pub struct PowerTrain<'a> {
    pub battery: &'a BatterySpec,
    pub motor: &'a MotorModel<'a>,
    pub motor_count: u32,
    /// Pack resistance, ohm.
    pub r_pack: f64,
}

impl<'a> PowerTrain<'a> {
    pub fn new(battery: &'a BatterySpec, motor: &'a MotorModel<'a>, motor_count: u32) -> Self {
        Self {
            r_pack: battery.pack_resistance_mohm / 1000.0,
            battery,
            motor,
            motor_count,
        }
    }

    /// Converges the phase-side operating point for a per-motor mechanical
    /// requirement (`rev_per_s`, `shaft_power_w`). Returns `None` when the
    /// required point exceeds full throttle even with zero sag.
    pub fn solve_point(&self, rev_per_s: f64, shaft_power_w: f64) -> Option<OperatingPoint> {
        let omega = rev_per_s * std::f64::consts::TAU;
        let v_nominal = Constants::CELL_V * self.battery.cells_s as f64;

        // Zero-sag ceiling check first — anything above it never converges.
        let i_zero_sag = self.motor.phase_current_at(omega, shaft_power_w, v_nominal)?;
        if self.motor.throttle_at(omega, i_zero_sag, v_nominal)? > 1.0 {
            return None;
        }

        // Iterate sag: phase current → pack voltage drop → re-solve. The
        // sagging load is roughly the pack-side current (power balance),
        // approximated here from the phase quantities.
        let mut v_sag = v_nominal;
        let mut point = None;
        for _ in 0..8 {
            let i_phase = self.motor.phase_current_at(omega, shaft_power_w, v_sag)?;
            let throttle = self.motor.throttle_at(omega, i_phase, v_nominal)?;
            if throttle > 1.0 {
                return None;
            }
            let i_pack_per_motor = throttle * i_phase;
            let v_next = (v_nominal
                - i_pack_per_motor * self.motor_count as f64 * self.r_pack)
                .max(v_nominal * 0.6);
            point = Some(OperatingPoint { phase_current_a: i_phase, throttle, v_sag });
            if (v_next - v_sag).abs() < 0.01 {
                break;
            }
            v_sag = v_next;
        }
        point
    }
}

/// Parasite drag power at `speed_ms` for the spec's Cd·A, W.
pub fn parasite_power(spec: &BuildSpec, speed_ms: f64) -> f64 {
    0.5 * Constants::RHO * speed_ms.powi(2) * spec.drag_area_m2 * speed_ms
}

/// Fixed-wing level-flight drag at `speed_ms` from the polar, N.
/// `None` when the speed needs more lift than CL_max allows (stall).
pub fn polar_drag(spec: &BuildSpec, speed_ms: f64) -> Option<f64> {
    if spec.wing_area_m2 <= 0.0 || speed_ms <= 0.0 {
        return None;
    }
    let q = 0.5 * Constants::RHO * speed_ms.powi(2);
    let weight_n = spec.all_up_weight_g() * Constants::G / 1000.0;
    let cl = weight_n / (q * spec.wing_area_m2);
    if cl > spec.cl_max * 0.95 {
        return None; // stalled — the wing cannot hold this speed level
    }
    let k = 1.0 / (std::f64::consts::PI * spec.oswald_e * spec.aspect_ratio());
    let cd = spec.cd0 + k * cl * cl;
    Some(q * spec.wing_area_m2 * cd)
}

/// Stall speed from CL_max, m/s.
pub fn stall_speed_ms(spec: &BuildSpec) -> Option<f64> {
    if spec.wing_area_m2 <= 0.0 || spec.cl_max <= 0.0 {
        return None;
    }
    let weight_n = spec.all_up_weight_g() * Constants::G / 1000.0;
    Some((2.0 * weight_n / (Constants::RHO * spec.wing_area_m2 * spec.cl_max)).sqrt())
}

/// Level-flight electrical power requirement at `speed_ms`, W (the whole
/// drivetrain's draw), plus the multirotor's per-motor rev/s for the
/// phase-side solve (0 for fixed wing, whose power path is direct).
///
/// Multirotor: induced power from the actuator-disk inflow relation
/// (v_i = v_h²/√(v² + v_i²), so induced power *falls* with speed — the
/// reason a cruising quad beats a hovering one on efficiency), calibrated
/// to the converged hover point; parasite drag work added through the
/// propulsive efficiency. Fixed wing: drag·speed through prop + motor
/// efficiency. Both divided by [`Constants::MOTOR_EFFICIENCY`] to get the
/// electrical side.
pub fn level_flight_power(
    spec: &BuildSpec,
    prop: &PropModel,
    speed_ms: f64,
    hover: &HoverPoint,
    propulsive_efficiency: f64,
) -> (f64, f64) {
    let weight_n = spec.all_up_weight_g() * Constants::G / 1000.0;
    let shaft_power = match spec.kind {
        AirframeKind::Multirotor => {
            let drag_n = parasite_power(spec, speed_ms) / speed_ms.max(0.1);
            let thrust_n = (weight_n * weight_n + drag_n * drag_n).sqrt();
            let disk_area = prop.disk_area(spec.motor_count);
            let v_h = (weight_n / (2.0 * Constants::RHO * disk_area)).sqrt();
            let mut v_i = v_h;
            for _ in 0..12 {
                v_i = v_h * v_h / (speed_ms * speed_ms + v_i * v_i).sqrt();
            }
            let induced = hover.shaft_power_total_w * (thrust_n * v_i) / (weight_n * v_h);
            induced + parasite_power(spec, speed_ms) / propulsive_efficiency
        }
        AirframeKind::FixedWing => {
            let drag_n = polar_drag(spec, speed_ms).unwrap_or(f64::NAN);
            drag_n * speed_ms / propulsive_efficiency
        }
    };
    let per_motor = shaft_power / spec.motor_count as f64;
    (prop.revs_for_power(per_motor), shaft_power / Constants::MOTOR_EFFICIENCY)
}

/// Converged hover operating point (multirotor) — the calibration anchor
/// for the whole flight envelope.
#[derive(Debug, Clone, Copy)]
pub struct HoverPoint {
    pub rev_per_s: f64,
    pub shaft_power_total_w: f64,
    pub phase_current_a: f64,
    pub throttle: f64,
    pub v_sag: f64,
}
