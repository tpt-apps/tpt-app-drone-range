//! Input model: motor / prop / battery / airframe / payload specs in the
//! units drone builders actually use (grams, inches, mAh, S, Kv).

use std::sync::LazyLock;

/// One brushless motor's bench-sheet numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct MotorSpec {
    /// Kv in rpm/V.
    pub kv_rpm_v: f64,
    /// Phase-to-phase winding resistance, ohm (bench: often quoted per-phase
    /// delta/star — datasheets for hobby motors quote the number to use here).
    pub resistance_ohm: f64,
    /// No-load current at 10 V, A — models bearing/aero friction torque.
    pub no_load_current_a: f64,
    /// Continuous current the motor survives, A (from the datasheet/bench).
    pub max_current_a: f64,
    /// Weight of one motor, g.
    pub weight_g: f64,
}

/// One propeller (all motors carry the same one).
#[derive(Debug, Clone, PartialEq)]
pub struct PropSpec {
    /// Diameter, inches.
    pub diameter_in: f64,
    /// Pitch, inches (geometric, as printed on the prop).
    pub pitch_in: f64,
    /// Blade count (2 for classic, 3 for modern freestyle/long-range).
    pub blades: u32,
    /// Weight of one propeller, g.
    pub weight_g: f64,
}

/// A battery pack.
#[derive(Debug, Clone, PartialEq)]
pub struct BatterySpec {
    /// Nominal capacity, mAh.
    pub capacity_mah: f64,
    /// Series cell count (the "S").
    pub cells_s: u32,
    /// Total pack internal resistance, milliohm.
    pub pack_resistance_mohm: f64,
    /// Pack weight, g.
    pub weight_g: f64,
    /// Discharge rating, C (continuous). 0 = unknown, skips the check.
    pub c_rating: f64,
    /// Usable fraction of nominal capacity before the low-voltage cutoff
    /// (0.8 = land at 20% — the conservative default for LiPo longevity).
    pub usable_fraction: f64,
}

/// Flight configuration class — selects the aerodynamic model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirframeKind {
    /// Multirotor: momentum-theory induced power + parasite drag.
    Multirotor,
    /// Fixed-wing: lift/drag polar with Oswald efficiency.
    FixedWing,
}

/// One complete build configuration (everything except the prop/motor
/// mounting details that don't affect the estimate).
#[derive(Debug, Clone, PartialEq)]
pub struct BuildSpec {
    pub name: String,
    pub kind: AirframeKind,

    pub motor: MotorSpec,
    pub motor_count: u32,
    pub prop: PropSpec,
    pub battery: BatterySpec,

    /// Everything that hangs off the frame except motors, props, battery and
    /// payload: frame, ESCs, flight controller, RX, wiring, shell.
    pub airframe_g: f64,
    /// Payload: camera, gimbal, sensors, delivery mass.
    pub payload_g: f64,

    /// Parasite drag reference area Cd·A, m². Multirotor: body + arms + the
    /// frontal prop_blade_effective sweep; fixed wing: fuselage + canopy +
    /// wetted-area equivalent.
    pub drag_area_m2: f64,

    // ---- fixed-wing only (ignored by the multirotor model) ----
    /// Wing area, m².
    pub wing_area_m2: f64,
    /// Wing span, m (gives aspect ratio AR = b²/S).
    pub wing_span_m: f64,
    /// Zero-lift drag coefficient of the whole airframe.
    pub cd0: f64,
    /// Oswald span efficiency factor (0.7–0.85 for typical RC planforms).
    pub oswald_e: f64,
    /// Maximum lift coefficient (flaps-up clean; 1.0–1.3 typical).
    pub cl_max: f64,
    /// Requested cruise speed, m/s. Used as-is for the cruise line of the
    /// estimate; best-range/best-endurance speeds come from the sweep.
    pub cruise_speed_ms: f64,
}

impl BuildSpec {
    /// All-up weight in g: airframe + motors + props + battery + payload.
    pub fn all_up_weight_g(&self) -> f64 {
        self.airframe_g
            + self.payload_g
            + self.motor.weight_g * self.motor_count as f64
            + self.prop.weight_g * self.motor_count as f64
            + self.battery.weight_g
    }

    /// Nominal pack voltage (3.7 V per Li/LiPo cell), V.
    pub fn pack_voltage_v(&self) -> f64 {
        3.7 * self.battery.cells_s as f64
    }

    /// Aspect ratio (fixed wing). Degrades to a plausible 6 for degenerate
    /// spans so the polar stays solvable instead of dividing by zero.
    pub fn aspect_ratio(&self) -> f64 {
        if self.wing_area_m2 <= 0.0 || self.wing_span_m <= 0.0 {
            return 6.0;
        }
        (self.wing_span_m * self.wing_span_m / self.wing_area_m2).clamp(3.0, 20.0)
    }
}

/// A built-in sample build (`SAMPLE_BUILDS`): realistic component specs for
/// common build classes. Component numbers are representative bench-sheet
/// values for the class (typical published Kv/Rm/Io figures), not vendor
/// listings — the samples exist to give users an honest starting point and
/// to validate the model against known flight times, not to advertise parts.
#[allow(clippy::too_many_arguments)]
fn sample(
    name: &str,
    kind: AirframeKind,
    kv: f64,
    rm: f64,
    io: f64,
    imax: f64,
    motor_g: f64,
    prop: (f64, f64, u32, f64),
    cells: u32,
    mah: f64,
    pack_rm: f64,
    batt_g: f64,
    c_rating: f64,
    airframe_g: f64,
    payload_g: f64,
    cda: f64,
    cruise_ms: f64,
    wing: (f64, f64, f64, f64, f64),
) -> BuildSpec {
    BuildSpec {
        name: name.to_string(),
        kind,
        motor: MotorSpec {
            kv_rpm_v: kv,
            resistance_ohm: rm,
            no_load_current_a: io,
            max_current_a: imax,
            weight_g: motor_g,
        },
        motor_count: 4,
        prop: PropSpec {
            diameter_in: prop.0,
            pitch_in: prop.1,
            blades: prop.2,
            weight_g: prop.3,
        },
        battery: BatterySpec {
            capacity_mah: mah,
            cells_s: cells,
            pack_resistance_mohm: pack_rm,
            weight_g: batt_g,
            c_rating,
            usable_fraction: 0.8,
        },
        airframe_g,
        payload_g,
        drag_area_m2: cda,
        wing_area_m2: wing.0,
        wing_span_m: wing.1,
        cd0: wing.2,
        oswald_e: wing.3,
        cl_max: wing.4,
        cruise_speed_ms: cruise_ms,
    }
}

/// Built-in sample builds shown in the UI's "try a sample" row and used by
/// the validation tests. Multirotors first, then the fixed wing. A
/// `LazyLock` (not a `const`) because the names are heap `String`s.
pub static SAMPLE_BUILDS: LazyLock<Vec<BuildSpec>> = LazyLock::new(|| {
    vec![
        // 5" freestyle quad, 6S 1300 mAh — ~4-6 min of hard flying.
        sample(
            "5\" freestyle quad (6S 1300)",
            AirframeKind::Multirotor,
            1750.0, 0.08, 0.6, 42.0, 32.0,
            (5.1, 4.1, 3, 3.2),
            6, 1300.0, 9.0, 250.0, 100.0,
            290.0, 0.0, 0.010, 15.0,
            (0.0, 0.0, 0.0, 0.0, 0.0),
        ),
        // 7" long-range quad, 6S 4000 mAh Li-ion — 20-30 min cruising class.
        sample(
            "7\" long-range quad (6S 4000 Li-ion)",
            AirframeKind::Multirotor,
            1300.0, 0.06, 0.8, 38.0, 48.0,
            (7.0, 4.0, 3, 6.5),
            6, 4000.0, 40.0, 390.0, 15.0,
            380.0, 50.0, 0.012, 19.0,
            (0.0, 0.0, 0.0, 0.0, 0.0),
        ),
        // 10" camera quad, 4S 8000 mAh — the mapping/inspection workhorse class.
        sample(
            "10\" camera quad (4S 8000)",
            AirframeKind::Multirotor,
            920.0, 0.12, 0.5, 19.0, 55.0,
            (10.0, 4.5, 2, 12.0),
            4, 8000.0, 15.0, 740.0, 25.0,
            480.0, 250.0, 0.020, 14.0,
            (0.0, 0.0, 0.0, 0.0, 0.0),
        ),
        // Mini camera-drone class (sub-250 g, 2S): DJI Mini 2 approximated.
        sample(
            "sub-250 g camera drone (2S)",
            AirframeKind::Multirotor,
            1100.0, 0.12, 0.35, 10.0, 6.0,
            (5.5, 3.0, 2, 2.0),
            2, 2250.0, 15.0, 80.0, 4.0,
            137.0, 0.0, 0.012, 7.0,
            (0.0, 0.0, 0.0, 0.0, 0.0),
        ),
        // 1.5 m flying wing, 4S 6000 mAh Li-ion — the 40-60 min long-range
        // class. Li-ion packs routinely run to 85% usable with a proper
        // low-voltage cutoff, hence the tweak from the 0.8 default.
        {
            let mut wing = sample(
                "1.5 m long-range wing (4S 6000)",
                AirframeKind::FixedWing,
                900.0, 0.08, 0.6, 25.0, 58.0,
                (9.0, 5.0, 2, 9.0),
                4, 6000.0, 30.0, 560.0, 10.0,
                620.0, 100.0, 0.020, 17.0,
                (0.36, 1.5, 0.028, 0.85, 1.1),
            );
            wing.battery.usable_fraction = 0.85;
            wing
        },
    ]
});
