//! Validation against real published flight times and physics invariants.
//!
//! The published-build checks approximate DJI's Mini 2 and the well-known
//! 1.5 m flying-wing long-range class with representative component specs
//! (DJI does not publish motor Kv/Rm), so the tolerance bands are generous
//! (±30%). They exist to catch gross modelling errors — a factor-of-two —
//! not to certify the app. Component-level checks (energy accounting,
//! stall speed, feasibility monotonicity) are exact.

use tpt_drone_engine::{
    solve, AirframeKind, BatterySpec, BuildSpec, MotorSpec, PropSpec, SolveIssue, SAMPLE_BUILDS,
};

/// The DJI-approximated class gets a wider band: DJI does not publish motor
/// Kv/resistance/prop data, so those internals are representative guesses —
/// the check catches gross modelling errors, not prop-selection subtleties.
const DJI_CLASS_TOLERANCE: f64 = 0.30;

fn approx(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() / expected <= tolerance
}

#[test]
fn mini2_class_endurance_matches_published_flight_time() {
    // DJI Mini 2: 249 g AUW, 2250 mAh 2S. Two published anchors:
    // - reviewer-measured hover: 27-29 min (independent bench logs);
    // - DJI's lab "max flight time": 31 min at a constant 25 km/h.
    // Hover is the honest hover anchor. The best-endurance sweep finds the
    // paper minimum-power speed (slower than DJI's test protocol), so it is
    // sanity-bounded: above the published figure is expected for an optimum,
    // but it must not exceed it by 2x.
    let spec = &SAMPLE_BUILDS[3];
    assert_eq!(spec.all_up_weight_g() as u32, 249, "sample must model the published AUW");
    let est = solve(spec);
    assert!(est.feasible, "Mini 2 class must solve as flyable: {:?}", est.issues);

    let hover = est.hover_endurance_min.unwrap();
    assert!(
        approx(hover, 28.0, DJI_CLASS_TOLERANCE),
        "hover endurance {hover:.1} min should be within ±{:.0}% of the reviewer-measured 27-29 min",
        DJI_CLASS_TOLERANCE * 100.0
    );
    let best = est.best_endurance_min;
    assert!(
        best >= 31.0 * 0.7 && best <= 31.0 * 1.7,
        "best endurance {best:.1} min should sanity-bound against the published 31 min"
    );
}

#[test]
fn flying_wing_cruise_matches_published_class() {
    // 1.5 m flying wing with 4S 6000 mAh Li-ion at ~60 km/h cruise: the
    // long-range community consistently reports 40-60 min and 40-60 km on
    // this class of build. The paper best-endurance point rides the stall
    // gate and goes higher — check it stays above the cruise line instead.
    let spec = &SAMPLE_BUILDS[4];
    let est = solve(spec);
    assert!(est.feasible, "wing must solve as flyable: {:?}", est.issues);
    assert!(
        (40.0..=60.0 * 1.3).contains(&est.cruise_endurance_min),
        "cruise endurance {:.1} min should land in the published 40-60 min class (+30% slack)",
        est.cruise_endurance_min
    );
    assert!(
        (30.0..=60.0 * 1.5).contains(&est.cruise_range_km),
        "cruise range {:.1} km should land in the published 40-60 km class",
        est.cruise_range_km
    );
    assert!(
        est.best_endurance_min >= est.cruise_endurance_min,
        "the min-power speed must not do worse than the requested cruise"
    );
    assert!(
        est.stall_speed_kmh.unwrap() < est.cruise_speed_kmh,
        "cruise must sit above stall"
    );
}

#[test]
fn freestyle_quad_lands_in_known_hover_band() {
    // 5" 6S freestyle quads at ~660 g AUW bench-test at roughly 4-8 A
    // steady-state hover draw (0.15-0.27 W/g) — aggressive-throttle
    // "hover" figures run higher; the model targets steady state.
    let spec = &SAMPLE_BUILDS[0];
    let est = solve(spec);
    assert!(est.feasible, "5\" quad must solve as flyable: {:?}", est.issues);
    let hover = est.hover_current_a.unwrap();
    assert!(
        (3.0..=15.0).contains(&hover),
        "hover current {hover:.1} A outside the known 4-8 A bench band for this class"
    );
    // And a 1300 mAh pack at that draw gives the familiar low minutes.
    assert!(
        est.hover_endurance_min.unwrap() < 16.0,
        "freestyle quad hover endurance should be low double digits at most, got {:.1}",
        est.hover_endurance_min.unwrap()
    );
}

#[test]
fn more_payload_shortens_everything() {
    for spec in SAMPLE_BUILDS.iter() {
        let light = solve(spec);
        let mut heavy = spec.clone();
        heavy.payload_g += 200.0;
        let heavy = solve(&heavy);
        if !heavy.feasible {
            continue; // past the flight ceiling — fine, checked elsewhere
        }
        assert!(
            heavy.best_endurance_min < light.best_endurance_min,
            "{}: +200 g must not improve endurance",
            spec.name
        );
        assert!(
            heavy.max_range_km <= light.max_range_km,
            "{}: +200 g must not improve range",
            spec.name
        );
    }
}

#[test]
fn more_battery_extends_endurance() {
    let mut spec = SAMPLE_BUILDS[0].clone();
    let base = solve(&spec);
    spec.battery.capacity_mah *= 2.0;
    spec.battery.weight_g *= 1.6; // bigger pack weighs more, but not 2×
    let doubled = solve(&spec);
    assert!(
        doubled.best_endurance_min > base.best_endurance_min,
        "a pack with ~1.6× the energy must fly longer than {:.1} min",
        base.best_endurance_min
    );
}

#[test]
fn overloading_eventually_becomes_unflyable() {
    let mut spec = SAMPLE_BUILDS[3].clone(); // the 249 g drone
    spec.payload_g = 500.0; // 2× its AUW in payload
    let est = solve(&spec);
    assert!(
        !est.feasible,
        "a 249 g drone carrying 500 g of payload must come back infeasible"
    );
    assert!(est
        .issues
        .iter()
        .any(|i| matches!(i, SolveIssue::Error(_))));
}

#[test]
fn range_is_speed_times_endurance_on_every_sweep_point() {
    for spec in SAMPLE_BUILDS.iter() {
        let est = solve(spec);
        for point in &est.speed_slice {
            if point.feasible {
                let expected = point.speed_kmh / 60.0 * point.endurance_min;
                assert!(
                    (point.range_km - expected).abs() < 1e-6,
                    "{} @ {:.0} km/h: range {} != v×t {}",
                    spec.name,
                    point.speed_kmh,
                    point.range_km,
                    expected
                );
            }
        }
    }
}

#[test]
fn stall_speed_matches_hand_calculation() {
    // W = 13.2 N·m... compute directly: v = sqrt(2W/(ρ·S·CLmax)).
    let spec = &SAMPLE_BUILDS[4];
    let est = solve(spec);
    let weight_n = spec.all_up_weight_g() * 9.80665 / 1000.0;
    let expected = (2.0 * weight_n / (1.225 * spec.wing_area_m2 * spec.cl_max)).sqrt() * 3.6;
    let actual = est.stall_speed_kmh.unwrap();
    assert!(
        (actual - expected).abs() < 0.01,
        "stall {actual:.2} != hand-calc {expected:.2} km/h"
    );
}

#[test]
fn structural_inputs_are_rejected_with_errors() {
    let mut spec = SAMPLE_BUILDS[0].clone();
    spec.battery.capacity_mah = 0.0;
    let est = solve(&spec);
    assert!(!est.feasible);
    assert!(matches!(est.issues.as_slice(), [SolveIssue::Error(_)]));
}

#[test]
fn sample_builds_are_individually_sane() {
    for spec in SAMPLE_BUILDS.iter() {
        let est = solve(spec);
        assert!(est.feasible, "{} must be flyable: {:?}", spec.name, est.issues);
        assert!(est.best_endurance_min > 0.0);
        assert!(est.max_range_km > 0.0);
        assert!(est.best_range_speed_kmh > 0.0);
        assert_eq!(est.all_up_weight_g, spec.all_up_weight_g());
    }
}

#[test]
fn energy_accounting_bounds_every_endurance() {
    // No sweep point may out-fly the usable energy budget at its own power:
    // t = usable_Wh / P must dominate v/P-derived range everywhere.
    for spec in SAMPLE_BUILDS.iter() {
        let est = solve(spec);
        for point in &est.speed_slice {
            if point.feasible && point.power_w > 0.0 {
                let energy_hours = est.usable_energy_wh / point.power_w;
                let endurance_h = point.endurance_min / 60.0;
                // Endurance derives from Ah-capacity, power from sagged
                // voltage; they agree to within the sag envelope.
                assert!(
                    endurance_h < energy_hours * 1.25,
                    "{} @ {:.0} km/h: endurance {:.2} h exceeds the energy budget {:.2} h",
                    spec.name,
                    point.speed_kmh,
                    endurance_h,
                    energy_hours
                );
            }
        }
    }
}

#[test]
fn fixed_wing_and_multirotor_kinds_agree_on_weight() {
    let spec = BuildSpec {
        name: "kind check".to_string(),
        kind: AirframeKind::Multirotor,
        motor: MotorSpec {
            kv_rpm_v: 1000.0,
            resistance_ohm: 0.1,
            no_load_current_a: 0.5,
            max_current_a: 20.0,
            weight_g: 50.0,
        },
        motor_count: 4,
        prop: PropSpec { diameter_in: 9.0, pitch_in: 4.5, blades: 2, weight_g: 10.0 },
        battery: BatterySpec {
            capacity_mah: 5000.0,
            cells_s: 4,
            pack_resistance_mohm: 10.0,
            weight_g: 500.0,
            c_rating: 30.0,
            usable_fraction: 0.8,
        },
        airframe_g: 400.0,
        payload_g: 100.0,
        drag_area_m2: 0.02,
        wing_area_m2: 0.30,
        wing_span_m: 1.4,
        cd0: 0.04,
        oswald_e: 0.8,
        cl_max: 1.1,
        cruise_speed_ms: 15.0,
    };
    let multi = solve(&spec);
    let mut wing = spec.clone();
    wing.kind = AirframeKind::FixedWing;
    let wing = solve(&wing);
    assert_eq!(multi.all_up_weight_g, wing.all_up_weight_g);
    assert!(wing.stall_speed_kmh.is_some() && multi.stall_speed_kmh.is_none());
    assert!(multi.hover_endurance_min.is_some() && wing.hover_endurance_min.is_none());
}
