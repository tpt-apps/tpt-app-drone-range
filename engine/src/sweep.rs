//! (Pro) Payload trade-off analysis: sweep payload mass through the build's
//! feasible envelope and report range/endurance at each step.

use crate::estimate;
use crate::preset::BuildSpec;

/// One point on the payload trade-off curve.
#[derive(Debug, Clone, Copy)]
pub struct PayloadPoint {
    /// Payload carried, g.
    pub payload_g: f64,
    /// Best-range distance at this payload, km.
    pub range_km: f64,
    /// Endurance at the best-endurance speed at this payload, min.
    pub endurance_min: f64,
    /// All-up weight at this payload, g.
    pub all_up_weight_g: f64,
    /// False once the build can no longer fly at this payload.
    pub flyable: bool,
}

/// Sweeps payload from 0 to `max_payload_g` in `steps` increments (both
/// inclusive). Points past the build's flight ceiling come back with
/// `flyable: false` so the UI can end the curve there instead of plotting
/// garbage.
pub fn payload_sweep(spec: &BuildSpec, max_payload_g: f64, steps: usize) -> Vec<PayloadPoint> {
    let steps = steps.max(1);
    let mut points = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let payload_g = max_payload_g * i as f64 / steps as f64;
        let mut trial = spec.clone();
        trial.payload_g = payload_g;
        let est = estimate::solve(&trial);
        points.push(PayloadPoint {
            payload_g,
            range_km: est.max_range_km,
            endurance_min: est.best_endurance_min,
            all_up_weight_g: est.all_up_weight_g,
            flyable: est.feasible,
        });
    }
    points
}

/// The largest payload (g) this build can carry and still fly, searched by
/// bisection on the feasibility flag within `hard_cap_g`.
pub fn max_flyable_payload_g(spec: &BuildSpec, hard_cap_g: f64) -> Option<f64> {
    let flyable = |payload_g: f64| {
        let mut trial = spec.clone();
        trial.payload_g = payload_g;
        estimate::solve(&trial).feasible
    };
    if !flyable(0.0) {
        return None;
    }
    if flyable(hard_cap_g) {
        return Some(hard_cap_g);
    }
    let (mut lo, mut hi) = (0.0_f64, hard_cap_g);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if flyable(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}
