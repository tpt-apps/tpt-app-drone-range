//! Free WASM edition UI (tpt-appfront-dom), single build configuration.
//!
//! Rendering architecture (mirrors the fea-lite/cutlist split):
//! - the **input panel is mounted once** (`tpt_appfront_dom::mount`) — typing
//!   never loses focus; field values are read straight from the DOM when
//!   Solve is pressed;
//! - the **results section re-renders fine-grained** via
//!   `tpt_appfront_dom::render` driven by the `outcome` Signal;
//! - the **envelope chart** is a raw web-sys `<canvas>` (the DOM backend has
//!   no canvas node kind) redrawn from an effect that watches `outcome`.

use std::rc::Rc;

use tpt_appfront_core::{create_effect, Signal, UITree};
use tpt_drone_engine::{solve, AirframeKind, BuildSpec, Estimate, SolveIssue, SAMPLE_BUILDS};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone)]
enum Outcome {
    Idle,
    Solved(Rc<Estimate>),
}

#[derive(Debug, Clone)]
enum Msg {
    Solve,
    Sample(usize),
    /// Open a TPT Solutions URL — in a new browser tab for the in-browser
    /// free edition, or via the desktop host's system-browser IPC action for
    /// the Pro desktop shell (whose webview would otherwise navigate itself
    /// away from the local `app://` bundle with no way back). See
    /// [`visit_site`].
    VisitSite(String),
}

const BASE_CSS: &str = r#"
.drn-app{--drn-bg:#f7f8fa;--drn-surface:#ffffff;--drn-panel:#f2f4f7;--drn-fg:#181d27;--drn-muted:#667085;--drn-border:#e3e7ee;--drn-accent:#0e7c6b;--drn-accent-2:#0a5f52;--drn-accent-soft:#e6f5f2;--drn-accent-fg:#ffffff;--drn-err:#b3261e;--drn-err-bg:#fbeeec;--drn-warn:#8a6116;--drn-warn-bg:#fdf6e3;--drn-ok:#0e7c6b;--drn-ok-bg:#e6f5f2;--drn-shadow:0 1px 2px rgba(16,24,40,.04),0 1px 3px rgba(16,24,40,.06);max-width:960px;margin:0 auto;background:var(--drn-bg);color:var(--drn-fg);font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;font-size:14.5px;line-height:1.55;padding:0 4px 32px}
@media(prefers-color-scheme:dark){.drn-app{--drn-bg:#0b0f16;--drn-surface:#141a24;--drn-panel:#161d2a;--drn-fg:#e8ebf0;--drn-muted:#98a2b3;--drn-border:#262f3d;--drn-accent:#34b39a;--drn-accent-2:#4fd6ba;--drn-accent-soft:#122824;--drn-accent-fg:#04140f;--drn-err:#f2b8b5;--drn-err-bg:#2c1a1c;--drn-warn:#e3c069;--drn-warn-bg:#2c2413;--drn-ok:#34b39a;--drn-ok-bg:#122824;--drn-shadow:0 1px 2px rgba(0,0,0,.3)}}
.drn-app *{box-sizing:border-box}
.drn-app h1{font-size:24px;margin:20px 0 2px;letter-spacing:-.02em}
.drn-app h2{font-size:17px;margin:0 0 10px}
.drn-app h3{font-size:14px;margin:14px 0 8px;color:var(--drn-muted);text-transform:uppercase;letter-spacing:.05em}
.drn-sub{color:var(--drn-muted);margin:0 0 16px;font-size:14px}
.drn-card{background:var(--drn-surface);border:1px solid var(--drn-border);border-radius:10px;box-shadow:var(--drn-shadow);padding:16px;margin-bottom:14px}
.drn-samples{display:flex;flex-wrap:wrap;gap:8px;margin-bottom:14px}
.drn-chip{font:inherit;font-size:13px;background:var(--drn-panel);color:var(--drn-fg);border:1px solid var(--drn-border);border-radius:999px;padding:5px 12px;cursor:pointer}
.drn-chip:hover{border-color:var(--drn-accent);color:var(--drn-accent)}
.drn-row{display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:10px}
.drn-field{display:flex;flex-direction:column;gap:3px;min-width:0}
.drn-field .drn-label{font-size:12px;color:var(--drn-muted)}
.drn-input{font:inherit;width:100%;padding:7px 9px;border:1px solid var(--drn-border);border-radius:7px;background:var(--drn-surface);color:var(--drn-fg)}
.drn-input:focus{outline:2px solid var(--drn-accent);outline-offset:-1px;border-color:transparent}
.drn-actions{display:flex;gap:10px;margin-top:14px;flex-wrap:wrap;align-items:center}
.drn-btn{font:inherit;font-weight:600;background:var(--drn-accent);color:var(--drn-accent-fg);border:none;border-radius:8px;padding:9px 20px;cursor:pointer}
.drn-btn:hover{background:var(--drn-accent-2)}
.drn-ghost{font:inherit;background:transparent;color:var(--drn-muted);border:1px solid var(--drn-border);border-radius:8px;padding:8px 14px;cursor:pointer}
.drn-hint{color:var(--drn-muted);font-size:13px;margin:6px 0 0}
.drn-verdict{border-radius:8px;padding:10px 14px;font-weight:600;margin-bottom:12px}
.drn-verdict.ok{background:var(--drn-ok-bg);color:var(--drn-ok)}
.drn-verdict.bad{background:var(--drn-err-bg);color:var(--drn-err)}
.drn-metrics{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:10px;margin:10px 0}
.drn-metric{background:var(--drn-panel);border-radius:8px;padding:10px 12px}
.drn-metric .v{font-size:20px;font-weight:700;letter-spacing:-.01em}
.drn-metric .u{font-size:12.5px;font-weight:400;color:var(--drn-muted)}
.drn-metric .k{font-size:12px;color:var(--drn-muted);margin-top:2px}
.drn-issue{border-radius:8px;padding:8px 12px;margin:6px 0;font-size:13.5px}
.drn-issue.err{background:var(--drn-err-bg);color:var(--drn-err)}
.drn-issue.warn{background:var(--drn-warn-bg);color:var(--drn-warn)}
.drn-chart{width:100%;height:auto;margin-top:8px}
.drn-upsell{border:1px solid var(--drn-accent);background:var(--drn-accent-soft);border-radius:10px;padding:14px 16px;margin-bottom:14px}
.drn-upsell h2{color:var(--drn-accent)}
.drn-upsell ul{margin:8px 0 12px;padding-left:20px}
.drn-upsell li{margin:3px 0}
.drn-foot{color:var(--drn-muted);font-size:12.5px;text-align:center;margin-top:18px}
"#;

/// Entry from [`mount_app`]: builds the static panel and wires the
/// reactive results section. Same split for both editions; Pro blocks are
/// `#[cfg(feature = "pro")]`-gated additions.
pub fn mount_app(container: &web_sys::Element) -> Result<(), JsValue> {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    upsert_style(&document, "drone-range-styles", BASE_CSS);
    // `.drn-app` on the mount root itself: every color custom property lives
    // here, so the theme depends on this class being present.
    let existing_class = container.get_attribute("class").unwrap_or_default();
    if !existing_class.split_whitespace().any(|c| c == "drn-app") {
        let combined = if existing_class.is_empty() {
            "drn-app".to_string()
        } else {
            format!("{existing_class} drn-app")
        };
        let _ = container.set_attribute("class", &combined);
    }

    let outcome: Signal<Outcome> = Signal::new(Outcome::Idle);

    let dispatch: Rc<dyn Fn(Msg)> = {
        let container = container.clone();
        let outcome = outcome.clone();
        Rc::new(move |msg| match msg {
            Msg::Solve => run_solve(&container, &outcome),
            Msg::Sample(index) => {
                apply_sample(&container, index);
                run_solve(&container, &outcome);
            }
            Msg::VisitSite(url) => visit_site(&url),
        })
    };

    // 1. Static input panel + samples row, mounted once.
    let ui = input_view();
    let _root = tpt_appfront_dom::mount(container, &ui, dispatch.clone())?;
    // The mount root is the first child; the panel's placeholders live under
    // the container itself via classes, so query from `container`.

    // Fixed-wing fields only apply when the kind select says so — toggle
    // their fieldset with a raw change listener (the input panel is static,
    // so this never re-renders).
    toggle_fixed_wing_group(container);
    if let Some(select) = container
        .query_selector(".drn-kind")?
        .and_then(|el| el.dyn_into::<web_sys::HtmlSelectElement>().ok())
    {
        let container_for_listener = container.clone();
        let closure = wasm_bindgen::closure::ScopedClosure::<dyn FnMut()>::new(move || {
            toggle_fixed_wing_group(&container_for_listener)
        });
        select.set_onchange(Some(closure.as_ref().unchecked_ref()));
        closure.forget();
    }

    // 2. Reactive results section into its placeholder.
    let results_el = container
        .query_selector(".drn-results")?
        .ok_or_else(|| JsValue::from_str("results placeholder missing"))?;
    let outcome_for_render = outcome.clone();
    let handle = tpt_appfront_dom::render(
        &results_el,
        Rc::new(move || results_view(outcome_for_render.clone())),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);

    // 3. Envelope chart: raw canvas appended into its placeholder, redrawn
    // by an effect that watches the outcome signal.
    let chart_holder = container
        .query_selector(".drn-chart-holder")?
        .ok_or_else(|| JsValue::from_str("chart placeholder missing"))?;
    let canvas: web_sys::HtmlCanvasElement = document
        .create_element("canvas")?
        .dyn_into()
        .map_err(|_| JsValue::from_str("canvas cast failed"))?;
    canvas.set_width(920);
    canvas.set_height(280);
    let _ = canvas.set_attribute("class", "drn-chart");
    chart_holder.append_child(&canvas)?;

    let chart_effect_outcome = outcome.clone();
    let handle = create_effect(move || {
        if let Outcome::Solved(est) = chart_effect_outcome.get() {
            let _ = draw_envelope_chart(&canvas, &est);
        }
    });
    std::mem::forget(handle);

    Ok(())
}

/// Loads a built-in sample's numbers into the fields.
fn apply_sample(container: &web_sys::Element, index: usize) {
    let Some(spec) = SAMPLE_BUILDS.get(index) else {
        return;
    };
    write_field(container, ".drn-name", &spec.name);
    let kind = match spec.kind {
        AirframeKind::Multirotor => "multirotor",
        AirframeKind::FixedWing => "fixedwing",
    };
    write_field(container, ".drn-kind", kind);
    write_field(container, ".drn-kv", &fmt_num(spec.motor.kv_rpm_v));
    write_field(container, ".drn-rm", &fmt_num(spec.motor.resistance_ohm * 1000.0));
    write_field(container, ".drn-io", &fmt_num(spec.motor.no_load_current_a));
    write_field(container, ".drn-imax", &fmt_num(spec.motor.max_current_a));
    write_field(container, ".drn-motor-g", &fmt_num(spec.motor.weight_g));
    write_field(container, ".drn-prop-d", &fmt_num(spec.prop.diameter_in));
    write_field(container, ".drn-prop-p", &fmt_num(spec.prop.pitch_in));
    write_field(container, ".drn-blades", &spec.prop.blades.to_string());
    write_field(container, ".drn-prop-g", &fmt_num(spec.prop.weight_g));
    write_field(container, ".drn-mah", &fmt_num(spec.battery.capacity_mah));
    write_field(container, ".drn-cells", &spec.battery.cells_s.to_string());
    write_field(container, ".drn-pack-rm", &fmt_num(spec.battery.pack_resistance_mohm));
    write_field(container, ".drn-batt-g", &fmt_num(spec.battery.weight_g));
    write_field(container, ".drn-crate", &fmt_num(spec.battery.c_rating));
    write_field(
        container,
        ".drn-usable",
        &fmt_num(spec.battery.usable_fraction * 100.0),
    );
    write_field(container, ".drn-count", &spec.motor_count.to_string());
    write_field(container, ".drn-airframe-g", &fmt_num(spec.airframe_g));
    write_field(container, ".drn-payload-g", &fmt_num(spec.payload_g));
    write_field(container, ".drn-cda", &fmt_num(spec.drag_area_m2));
    write_field(container, ".drn-cruise", &fmt_num(spec.cruise_speed_ms * 3.6));
    write_field(container, ".drn-wing-area", &fmt_num(spec.wing_area_m2));
    write_field(container, ".drn-wing-span", &fmt_num(spec.wing_span_m));
    write_field(container, ".drn-cd0", &fmt_num(spec.cd0));
    write_field(container, ".drn-oswald", &fmt_num(spec.oswald_e));
    write_field(container, ".drn-clmax", &fmt_num(spec.cl_max));
    toggle_fixed_wing_group(container);
}

/// Reads one numeric field, recording a parse problem instead of failing.
fn num_of(container: &web_sys::Element, selector: &str, label: &str, errors: &mut Vec<String>) -> f64 {
    let raw = read_input(container, selector);
    match raw.trim().parse::<f64>() {
        Ok(v) => v,
        Err(_) => {
            errors.push(format!("{label}: \u{201c}{raw}\u{201d} is not a number"));
            f64::NAN
        }
    }
}

/// Reads one whole-number field, recording a parse problem instead of failing.
fn int_of(container: &web_sys::Element, selector: &str, label: &str, errors: &mut Vec<String>) -> u32 {
    let raw = read_input(container, selector);
    match raw.trim().parse::<u32>() {
        Ok(v) => v,
        Err(_) => {
            errors.push(format!("{label}: \u{201c}{raw}\u{201d} is not a whole number"));
            0
        }
    }
}

/// Reads every field into a [`BuildSpec`], or the list of parse problems.
fn read_spec(container: &web_sys::Element) -> Result<BuildSpec, Vec<String>> {
    let mut errors = Vec::new();

    let name = {
        let raw = read_input(container, ".drn-name");
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            "My build".to_string()
        } else {
            trimmed.to_string()
        }
    };
    let kind = match read_input(container, ".drn-kind").as_str() {
        "fixedwing" => AirframeKind::FixedWing,
        _ => AirframeKind::Multirotor,
    };

    let kv = num_of(container, ".drn-kv", "Motor Kv", &mut errors);
    let rm_mohm = num_of(container, ".drn-rm", "Motor resistance", &mut errors);
    let io = num_of(container, ".drn-io", "Motor no-load current", &mut errors);
    let imax = num_of(container, ".drn-imax", "Motor max current", &mut errors);
    let motor_g = num_of(container, ".drn-motor-g", "Motor weight", &mut errors);
    let prop_d = num_of(container, ".drn-prop-d", "Prop diameter", &mut errors);
    let prop_p = num_of(container, ".drn-prop-p", "Prop pitch", &mut errors);
    let blades = int_of(container, ".drn-blades", "Prop blades", &mut errors);
    let prop_g = num_of(container, ".drn-prop-g", "Prop weight", &mut errors);
    let mah = num_of(container, ".drn-mah", "Battery capacity", &mut errors);
    let cells = int_of(container, ".drn-cells", "Battery cells", &mut errors);
    let pack_rm = num_of(container, ".drn-pack-rm", "Pack resistance", &mut errors);
    let batt_g = num_of(container, ".drn-batt-g", "Battery weight", &mut errors);
    let crate_c = num_of(container, ".drn-crate", "Battery C rating", &mut errors);
    let usable_pct = num_of(container, ".drn-usable", "Battery usable", &mut errors);
    let count = int_of(container, ".drn-count", "Motor count", &mut errors);
    let airframe_g = num_of(container, ".drn-airframe-g", "Airframe weight", &mut errors);
    let payload_g = num_of(container, ".drn-payload-g", "Payload weight", &mut errors);
    let cda = num_of(container, ".drn-cda", "Drag area", &mut errors);
    let cruise_kmh = num_of(container, ".drn-cruise", "Cruise speed", &mut errors);
    let wing_area = num_of(container, ".drn-wing-area", "Wing area", &mut errors);
    let wing_span = num_of(container, ".drn-wing-span", "Wing span", &mut errors);
    let cd0 = num_of(container, ".drn-cd0", "CD0", &mut errors);
    let oswald = num_of(container, ".drn-oswald", "Oswald e", &mut errors);
    let cl_max = num_of(container, ".drn-clmax", "CL max", &mut errors);

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(BuildSpec {
        name,
        kind,
        motor: tpt_drone_engine::MotorSpec {
            kv_rpm_v: kv,
            resistance_ohm: rm_mohm / 1000.0,
            no_load_current_a: io,
            max_current_a: imax,
            weight_g: motor_g,
        },
        motor_count: count.max(1),
        prop: tpt_drone_engine::PropSpec {
            diameter_in: prop_d,
            pitch_in: prop_p,
            blades: blades.clamp(2, 6),
            weight_g: prop_g,
        },
        battery: tpt_drone_engine::BatterySpec {
            capacity_mah: mah,
            cells_s: cells.clamp(1, 24),
            pack_resistance_mohm: pack_rm,
            weight_g: batt_g,
            c_rating: crate_c,
            usable_fraction: (usable_pct / 100.0).clamp(0.05, 1.0),
        },
        airframe_g,
        payload_g,
        drag_area_m2: cda,
        wing_area_m2: wing_area,
        wing_span_m: wing_span,
        cd0,
        oswald_e: oswald,
        cl_max,
        cruise_speed_ms: cruise_kmh / 3.6,
    })
}

/// Runs the solve and publishes the outcome (or the field-parse issues).
fn run_solve(container: &web_sys::Element, outcome: &Signal<Outcome>) {
    match read_spec(container) {
        Ok(spec) => outcome.set(Outcome::Solved(Rc::new(solve(&spec)))),
        Err(errors) => {
            let est = tpt_drone_engine::Estimate {
                feasible: false,
                issues: errors.into_iter().map(SolveIssue::Error).collect(),
                ..blank_estimate()
            };
            outcome.set(Outcome::Solved(Rc::new(est)));
        }
    }
}

/// An all-zero estimate to carry parse-issue text through the results view.
fn blank_estimate() -> Estimate {
    tpt_drone_engine::Estimate {
        feasible: false,
        issues: Vec::new(),
        all_up_weight_g: 0.0,
        battery_energy_wh: 0.0,
        usable_energy_wh: 0.0,
        pack_max_current_a: 0.0,
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
    }
}

/// Opens a URL — new tab in the browser, system browser from the desktop
/// shell (the host's `open_external` IPC action; without it the webview
/// would navigate itself away from the local bundle).
fn visit_site(url: &str) {
    let window = web_sys::window().expect("no window");
    let _ = window.open_with_url_and_target(url, "_blank");
}

/// Shows/hides the fixed-wing fieldset to match the kind select.
fn toggle_fixed_wing_group(container: &web_sys::Element) {
    let kind = read_input(container, ".drn-kind");
    let display = if kind == "fixedwing" { "" } else { "none" };
    if let Some(group) = container.query_selector(".drn-fw-group").ok().flatten() {
        let _ = group.set_attribute("style", &format!("display:{display}"));
    }
}

// ---------- views ----------

/// The static input panel (mounted once, never re-rendered).
fn input_view() -> UITree<Msg> {
    let samples: Vec<UITree<Msg>> = SAMPLE_BUILDS
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            UITree::container(|b| {
                b.button(spec.name.clone()).class("drn-chip").on_click(Msg::Sample(i));
            })
        })
        .collect();

    UITree::container(|b| {
        b.heading(1, "TPT Drone Range");
        styled_text(b, "drn-sub", "Estimate range, endurance, and payload trade-offs for a custom drone build — enter the specs from your motor, prop, and battery bench sheets.");

        b.container(|row| {
            for chip in samples {
                row.with(chip);
            }
        }).class("drn-samples");

        b.container(|card| {
            card.heading(2, "Build configuration");

            card.container(|row| {
                row.container(|field| {
                    styled_text(field, "drn-label", "Build name");
                    field.input("My build").class("drn-input drn-name").attr("aria-label", "Build name");
                }).class("drn-field");
                row.container(|field| {
                    styled_text(field, "drn-label", "Type");
                    field.select(
                        [("multirotor", "Multirotor"), ("fixedwing", "Fixed wing")],
                        "multirotor",
                    ).class("drn-input drn-kind").attr("aria-label", "Build type");
                }).class("drn-field");
                row.container(|field| {
                    styled_text(field, "drn-label", "Motors");
                    field.select(
                        [("1", "1"), ("2", "2"), ("3", "3"), ("4", "4"), ("6", "6"), ("8", "8")],
                        "4",
                    ).class("drn-input drn-count").attr("aria-label", "Motor count");
                }).class("drn-field");
            }).class("drn-row");

            card.heading(3, "Motor & prop (per motor)");
            card.container(|row| {
                num_field(row, ".drn-kv", "Kv (rpm/V)", "1750");
                num_field(row, ".drn-rm", "Resistance (mΩ)", "80");
                num_field(row, ".drn-io", "No-load current (A)", "0.6");
            }).class("drn-row");
            card.container(|row| {
                num_field(row, ".drn-imax", "Max continuous current (A)", "42");
                num_field(row, ".drn-motor-g", "Motor weight (g)", "32");
            }).class("drn-row");

            card.container(|row| {
                num_field(row, ".drn-prop-d", "Prop diameter (in)", "5.1");
                num_field(row, ".drn-prop-p", "Prop pitch (in)", "4.1");
                num_field(row, ".drn-prop-g", "Prop weight (g)", "3.2");
                row.container(|field| {
                    styled_text(field, "drn-label", "Blades");
                    field.select(
                        [("2", "2"), ("3", "3"), ("4", "4"), ("5", "5"), ("6", "6")],
                        "3",
                    ).class("drn-input drn-blades").attr("aria-label", "Prop blades");
                }).class("drn-field");
            }).class("drn-row");

            card.heading(3, "Battery");
            card.container(|row| {
                num_field(row, ".drn-mah", "Capacity (mAh)", "1300");
                num_field(row, ".drn-cells", "Cells (S)", "6");
                num_field(row, ".drn-pack-rm", "Pack resistance (mΩ)", "9");
            }).class("drn-row");
            card.container(|row| {
                num_field(row, ".drn-batt-g", "Weight (g)", "250");
                num_field(row, ".drn-crate", "C rating (0 = skip check)", "100");
                num_field(row, ".drn-usable", "Usable (%)", "80");
            }).class("drn-row");

            card.heading(3, "Airframe & mission");
            card.container(|row| {
                num_field(row, ".drn-airframe-g", "Airframe weight (g)", "290");
                num_field(row, ".drn-payload-g", "Payload weight (g)", "0");
                num_field(row, ".drn-cda", "Drag area Cd·A (m²)", "0.010");
            }).class("drn-row");
            card.container(|row| {
                num_field(row, ".drn-cruise", "Cruise speed (km/h)", "54");
            }).class("drn-row");

            card.container(|group| {
                group.heading(3, "Wing (fixed wing only)");
                group.container(|row| {
                    num_field(row, ".drn-wing-area", "Wing area (m²)", "0.36");
                    num_field(row, ".drn-wing-span", "Wing span (m)", "1.5");
                    num_field(row, ".drn-cd0", "CD0 (zero-lift drag)", "0.028");
                }).class("drn-row");
                group.container(|row| {
                    num_field(row, ".drn-oswald", "Oswald e", "0.85");
                    num_field(row, ".drn-clmax", "CL max", "1.1");
                }).class("drn-row");
            }).class("drn-fw-group");

            card.container(|actions| {
                actions.button("Estimate range & endurance").class("drn-btn").on_click(Msg::Solve);
            }).class("drn-actions");
            styled_text(card, "drn-hint", "Tip: motor resistance is often quoted in mΩ — multiply Ω by 1000. Pack resistance is the whole pack's, not per cell.");
        }).class("drn-card");

        // Placeholders the reactive sections mount into (never re-created).
        card_placeholder(b, "drn-results");
        chart_placeholder(b, "drn-chart-holder");

        upsell_view(b);
        styled_text(b, "drn-foot", "Model class: momentum theory + DC-motor electrical chain, calibrated against published bench data — expect ±25-30% vs a real flight log, not lab precision. No data leaves the page.");
    })
}

/// Styled text: the DOM backend renders bare `text` nodes without their
/// classes, so anything that carries styling wraps in a container here.
fn styled_text(b: &mut tpt_appfront_core::ContainerBuilder<Msg>, class: &str, text: impl Into<String>) {
    let text = text.into();
    b.container(|w| {
        w.text(text);
    })
    .class(class);
}

/// Small helper: one labelled numeric input inside a row builder. The
/// identifying class (selector-style, with a leading dot) goes on the
/// **input** — `read_spec`/`apply_sample` query `.drn-*` selectors.
fn num_field(b: &mut tpt_appfront_core::ContainerBuilder<Msg>, class: &str, label: &str, value: &str) {
    let cls = class.trim_start_matches('.');
    b.container(|field| {
        styled_text(field, "drn-label", label);
        // `class` is a single slot in NodeMeta — pass combined, not chained.
        field.input(value.to_string()).class(format!("drn-input {cls}")).attr("aria-label", label);
    })
    .class("drn-field");
}

/// Placeholder container for the reactive results card.
fn card_placeholder(b: &mut tpt_appfront_core::ContainerBuilder<Msg>, class: &str) {
    b.container(|_| {}).class(format!("drn-card {class}"));
}

/// Placeholder for the chart canvas.
fn chart_placeholder(b: &mut tpt_appfront_core::ContainerBuilder<Msg>, class: &str) {
    b.container(|c| {
        c.heading(2, "Flight envelope");
        styled_text(c, "drn-hint", "Endurance and range at each constant cruise speed. The marks show the best-endurance, best-range and your cruise speed.");
    }).class(class);
}

/// The free-tier upsell block (the Pro desktop build replaces this whole
/// card with the real features).
fn upsell_view(b: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    b.container(|up| {
        up.heading(2, "Pro edition — $29 one-time");
        styled_text(up, "drn-hint", "The free estimate above is one build. The offline Pro edition adds what serious builders compare builds with:");
        up.list(|list| {
            list.text("Multi-configuration comparison — race up to four builds side by side");
            list.text("Payload trade-off analysis — the range-vs-payload curve for your build");
            list.text("Saved build profiles — keep every iteration, reload and compare later");
        });
        up.container(|actions| {
            actions.button("See the Pro edition")
                .class("drn-btn")
                .on_click(Msg::VisitSite("https://tptsolutions.co.nz/tools/drone-range".to_string()));
        }).class("drn-actions");
    }).class("drn-upsell");
}

/// The reactive results section — a pure function of the outcome signal.
fn results_view(outcome: Signal<Outcome>) -> UITree<Msg> {
    let outcome = outcome.get();
    let est = match &outcome {
        Outcome::Idle => {
            return UITree::container(|c| {
                styled_text(c, "drn-hint", "Press “Estimate range & endurance”, or load a sample build above.");
            });
        }
        Outcome::Solved(est) => Rc::clone(est),
    };

    UITree::container(|c| {
        let (banner, ok) = if est.feasible {
            (
                format!(
                    "Flyable — {} g AUW · {:.1} Wh pack · {:.1} Wh usable",
                    fmt_num(est.all_up_weight_g),
                    est.battery_energy_wh,
                    est.usable_energy_wh
                ),
                true,
            )
        } else {
            ("Not flyable as specified — fix the errors below".to_string(), false)
        };
        c.container(|w| {
            w.text(banner);
        })
        .class(if ok { "drn-verdict ok" } else { "drn-verdict bad" });

        c.heading(2, "Results");

        c.container(|grid| {
            metric(grid, "Max range", &format!("{} km", fmt_num(est.max_range_km)), Some(format!("at {} km/h", fmt_num(est.best_range_speed_kmh))));
            metric(grid, "Best endurance", &format!("{} min", fmt_num(est.best_endurance_min)), Some(format!("at {} km/h", fmt_num(est.best_endurance_speed_kmh))));
            metric(grid, "Cruise", &format!("{} km", fmt_num(est.cruise_range_km)), Some(format!("{} km/h · {} min", fmt_num(est.cruise_speed_kmh), fmt_num(est.cruise_endurance_min))));
            metric(grid, "All-up weight", &format!("{} g", fmt_num(est.all_up_weight_g)), Some(format!("{} Wh usable", fmt_num(est.usable_energy_wh))));
            if let Some(hover) = est.hover_endurance_min {
                metric(grid, "Hover", &format!("{} min", fmt_num(hover)), Some(format!("{} A · {}% throttle", fmt_num(est.hover_current_a.unwrap_or(0.0)), fmt_num(est.hover_throttle_pct.unwrap_or(0.0)))));
            }
            if let Some(twr) = est.thrust_to_weight {
                metric(grid, "Thrust / weight", &format!("{}:1", fmt_num(twr)), None);
            }
            if let Some(stall) = est.stall_speed_kmh {
                metric(grid, "Stall speed", &format!("{} km/h", fmt_num(stall)), est.wing_loading_g_dm2.map(|wl| format!("{} g/dm² wing loading", fmt_num(wl))));
            }
            if let Some(dl) = est.disc_loading_g_dm2 {
                metric(grid, "Disc loading", &format!("{} g/dm²", fmt_num(dl)), None);
            }
        }).class("drn-metrics");

        for issue in &est.issues {
            let (class, text) = match issue {
                SolveIssue::Error(t) => ("err", t.clone()),
                SolveIssue::Warning(t) => ("warn", t.clone()),
            };
            c.container(|w| {
                w.text(text);
            })
            .class(format!("drn-issue {class}"));
        }
    })
}

/// One metric card inside the results grid.
fn metric(b: &mut tpt_appfront_core::ContainerBuilder<Msg>, k: &str, v: &str, unit: Option<String>) {
    let value = v.to_string();
    let label = k.to_string();
    b.container(|m| {
        m.container(|vv| {
            vv.container(|velem| {
                velem.text(value.clone());
            }).class("v");
            if let Some(u) = unit {
                vv.container(|uelem| {
                    uelem.text(format!(" {u}"));
                }).class("u");
            }
        });
        m.container(|kelem| {
            kelem.text(label);
        }).class("k");
    }).class("drn-metric");
}

/// Draws the endurance/range envelope onto the chart canvas.
fn draw_envelope_chart(canvas: &web_sys::HtmlCanvasElement, est: &Estimate) -> Result<(), JsValue> {
    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .map_err(|_| JsValue::from_str("context cast failed"))?;
    let w = canvas.width() as f64;
    let h = canvas.height() as f64;
    ctx.clear_rect(0.0, 0.0, w, h);

    let flyable: Vec<&tpt_drone_engine::SpeedPoint> =
        est.speed_slice.iter().filter(|p| p.feasible).collect();
    if flyable.len() < 2 {
        return Ok(());
    }
    let x_max = flyable.last().unwrap().speed_kmh;
    let t_max = flyable.iter().map(|p| p.endurance_min).fold(0.0, f64::max);
    let r_max = flyable.iter().map(|p| p.range_km).fold(0.0, f64::max);
    if t_max <= 0.0 || r_max <= 0.0 {
        return Ok(());
    }
    let pad_l = 46.0;
    let pad_r = 46.0;
    let pad_t = 14.0;
    let pad_b = 28.0;

    let px = |kmh: f64| pad_l + (w - pad_l - pad_r) * kmh / x_max;
    let py_t = |min: f64| h - pad_b - (h - pad_t - pad_b) * min / t_max;
    let py_r = |km: f64| h - pad_b - (h - pad_t - pad_b) * km / r_max;

    // Axes (stroke in the inherited text colour via a fixed neutral).
    ctx.set_stroke_style_str("#888");
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.move_to(pad_l, pad_t);
    ctx.line_to(pad_l, h - pad_b);
    ctx.line_to(w - pad_r, h - pad_b);
    ctx.stroke();

    // Gridlines + tick labels.
    ctx.set_font("11px system-ui, sans-serif");
    ctx.set_fill_style_str("#888");
    for frac in [0.25, 0.5, 0.75, 1.0] {
        let y = h - pad_b - (h - pad_t - pad_b) * frac;
        ctx.begin_path();
        ctx.move_to(pad_l, y);
        ctx.line_to(w - pad_r, y);
        ctx.set_stroke_style_str("rgba(136,136,136,.25)");
        ctx.stroke();
        let _ = ctx.fill_text(&format!("{:.0} min", t_max * frac), 2.0, y + 4.0);
        let _ = ctx.fill_text(&format!("{:.0} km", r_max * frac), w - pad_r + 4.0, y + 4.0);
    }
    for frac in [0.0, 0.5, 1.0] {
        let _ = ctx.fill_text(
            &format!("{:.0}", x_max * frac),
            px(x_max * frac) - 8.0,
            h - 8.0,
        );
    }
    let _ = ctx.fill_text("km/h", w / 2.0 - 10.0, h - 8.0);

    // Endurance curve (accent) and range curve (muted).
    let curve = |select: fn(&tpt_drone_engine::SpeedPoint) -> f64,
                 map_y: &dyn Fn(f64) -> f64,
                 color: &str,
                 width: f64| {
        ctx.begin_path();
        let mut started = false;
        for p in &flyable {
            let x = px(p.speed_kmh);
            let y = map_y(select(p));
            if started {
                ctx.line_to(x, y);
            } else {
                ctx.move_to(x, y);
                started = true;
            }
        }
        ctx.set_stroke_style_str(color);
        ctx.set_line_width(width);
        ctx.stroke();
    };
    curve(|p| p.endurance_min, &py_t, "#0e7c6b", 2.4);
    curve(|p| p.range_km, &py_r, "rgba(120,130,150,.9)", 2.0);

    // Markers: best range, best endurance, cruise.
    let marker = |kmh: f64, color: &str| {
        ctx.begin_path();
        ctx.set_stroke_style_str(color);
        ctx.move_to(px(kmh), pad_t);
        ctx.line_to(px(kmh), h - pad_b);
        ctx.stroke();
    };
    marker(est.best_range_speed_kmh, "rgba(120,130,150,.5)");
    marker(est.best_endurance_speed_kmh, "rgba(14,124,107,.5)");
    marker(est.cruise_speed_kmh, "rgba(200,120,40,.7)");

    // Legend.
    let legend = [
        ("endurance (left)", "#0e7c6b"),
        ("range (right)", "rgba(120,130,150,.9)"),
        ("cruise mark", "rgba(200,120,40,.9)"),
    ];
    let mut lx = pad_l + 8.0;
    for (label, color) in legend {
        ctx.set_fill_style_str(color);
        ctx.fill_rect(lx, 4.0, 12.0, 3.0);
        ctx.set_fill_style_str("#888");
        let _ = ctx.fill_text(label, lx + 16.0, 10.0);
        lx += 16.0 + label.len() as f64 * 5.6 + 14.0;
    }
    Ok(())
}

// ---------- DOM helpers (same idioms as the reference apps) ----------

fn read_input(container: &web_sys::Element, selector: &str) -> String {
    container
        .query_selector(selector)
        .ok()
        .flatten()
        .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .or_else(|| {
            container
                .query_selector(selector)
                .ok()
                .flatten()
                .and_then(|el| el.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .map(|select| select.value())
        })
        .unwrap_or_default()
}

fn write_field(container: &web_sys::Element, selector: &str, value: &str) {
    if let Some(el) = container.query_selector(selector).ok().flatten() {
        if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
            input.set_value(value);
        } else if let Some(select) = el.dyn_ref::<web_sys::HtmlSelectElement>() {
            select.set_value(value);
        }
    }
}

/// Creates or updates the `<style id>` in the document head.
fn upsert_style(document: &web_sys::Document, id: &str, css: &str) {
    if let Ok(Some(existing)) = document.query_selector(&format!("style#{id}")) {
        existing.set_text_content(Some(css));
        return;
    }
    let Some(head) = document.head() else {
        return;
    };
    if let Ok(style_el) = document.create_element("style") {
        let _ = style_el.set_attribute("id", id);
        style_el.set_text_content(Some(css));
        let _ = head.append_child(&style_el);
    }
}

/// Compact number formatting for results text.
fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "—".to_string();
    }
    let a = v.abs();
    if a >= 100.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else if a >= 1.0 {
        format!("{v:.2}")
    } else if a >= 0.01 {
        format!("{v:.3}")
    } else if a == 0.0 {
        "0".to_string()
    } else {
        format!("{v:.4}")
    }
}
