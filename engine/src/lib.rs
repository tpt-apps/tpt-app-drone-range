//! TPT Drone Range — engine crate (the moat).
//!
//! Flight-performance estimation for custom drone builds in pure Rust with
//! zero UI dependencies: shared verbatim by the free WASM edition and the
//! paid desktop edition, and unit-testable on the host.
//!
//! # Model overview
//!
//! A [`BuildSpec`] (motor, prop, battery, airframe, payload — in the units
//! builders actually use) solves to an [`Estimate`]: hover/cruise currents,
//! endurance and range at the best-endurance, best-range, and requested
//! cruise speeds, plus feasibility warnings (throttle margin, motor current,
//! battery C-rating, stall). `AirframeKind::Multirotor` uses actuator-disk
//! momentum theory for induced power and a parasite-drag term for forward
//! flight; `AirframeKind::FixedWing` uses the lift/drag polar with an
//! Oswald efficiency factor. Both share the same electrical chain: pack sag
//! from internal resistance, DC-motor torque/back-EMF equations, and a
//! usable-fraction-limited battery energy budget.
//!
//! Accuracy class: rule-of-thumb replacement (±20%), not a wind-tunnel.
//! Prop thrust/power coefficients are empirical fits around the UIUC
//! propeller database's typical values; see `physics::prop_coefficients`.

pub mod estimate;
pub mod physics;
pub mod preset;
#[cfg(feature = "pro")]
pub mod sweep;

pub use estimate::{Estimate, SolveIssue, SpeedPoint};
pub use physics::{Constants, HoverPoint, MotorModel, PowerTrain, PropModel};
pub use preset::{AirframeKind, BatterySpec, BuildSpec, MotorSpec, PropSpec, SAMPLE_BUILDS};
#[cfg(feature = "pro")]
pub use sweep::PayloadPoint;

/// Air density used throughout (ISA sea level, 15 °C), kg/m³.
pub const AIR_DENSITY: f64 = 1.225;

/// Solves one build configuration to its performance estimate. Always
/// returns an [`Estimate`]; infeasibility shows up as `feasible: false`
/// plus entries in `warnings` (the UI renders them inline rather than
/// treating the solve as an error).
pub fn solve(spec: &BuildSpec) -> Estimate {
    estimate::solve(spec)
}

/// The constant g the model assumes, exposed for tests and UI footnotes.
pub const G: f64 = Constants::G;
