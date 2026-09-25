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
    Solved(Rc<SolvedRun>),
}

/// One solved run: the estimate plus the spec it came from (the Pro payload
/// sweep re-solves the spec at increasing payload masses).
struct SolvedRun {
    spec: Rc<BuildSpec>,
    est: Rc<Estimate>,
}

/// (pro) One pinned column in the configuration comparison.
#[cfg(feature = "pro")]
#[derive(Clone)]
struct CompareRow {
    name: String,
    est: Rc<Estimate>,
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
    /// (pro) pin the current solve into the comparison table.
    #[cfg(feature = "pro")]
    CompareSnapshot,
    /// (pro) empty the comparison table.
    #[cfg(feature = "pro")]
    CompareClear,
    /// (pro) save the current spec under the profile-name field.
    #[cfg(feature = "pro")]
    SaveProfile,
    /// (pro) load a saved profile into the form and solve it.
    #[cfg(feature = "pro")]
    LoadProfile(String),
    /// (pro) delete a saved profile.
    #[cfg(feature = "pro")]
    DeleteProfile(String),
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
.drn-compare-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:10px;margin-top:10px}
.drn-compare-col{background:var(--drn-panel);border-radius:8px;padding:10px 12px}
.drn-compare-col .n{font-weight:700;margin-bottom:6px}
.drn-compare-col .r{font-size:12.5px;margin:2px 0;color:var(--drn-fg)}
.drn-compare-col .r b{color:var(--drn-muted);font-weight:400}
.drn-profile-row{display:flex;align-items:center;gap:8px;padding:6px 0;border-bottom:1px solid var(--drn-border)}
.drn-profile-row .pn{flex:1;font-weight:600}
.drn-mini{font:inherit;font-size:12.5px;background:var(--drn-panel);color:var(--drn-fg);border:1px solid var(--drn-border);border-radius:6px;padding:4px 10px;cursor:pointer}
.drn-mini:hover{border-color:var(--drn-accent);color:var(--drn-accent)}
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
    // (pro) pinned comparison columns and the saved-profile name list.
    #[cfg(feature = "pro")]
    let compare: Signal<Vec<CompareRow>> = Signal::new(Vec::new());
    #[cfg(feature = "pro")]
    let profiles: Signal<Vec<String>> = Signal::new(list_profiles());

    let dispatch: Rc<dyn Fn(Msg)> = {
        let container = container.clone();
        let outcome = outcome.clone();
        #[cfg(feature = "pro")]
        let compare = compare.clone();
        #[cfg(feature = "pro")]
        let profiles = profiles.clone();
        Rc::new(move |msg| match msg {
            Msg::Solve => run_solve(&container, &outcome),
            Msg::Sample(index) => {
                apply_sample(&container, index);
                run_solve(&container, &outcome);
            }
            Msg::VisitSite(url) => visit_site(&url),
            #[cfg(feature = "pro")]
            Msg::CompareSnapshot => {
                if let Outcome::Solved(run) = outcome.get() {
                    let mut rows = compare.get();
                    if rows.len() < 4 {
                        rows.push(CompareRow {
                            name: run.spec.name.clone(),
                            est: Rc::clone(&run.est),
                        });
                        compare.set(rows);
                    }
                }
            }
            #[cfg(feature = "pro")]
            Msg::CompareClear => compare.set(Vec::new()),
            #[cfg(feature = "pro")]
            Msg::SaveProfile => {
                let name = read_input(&container, ".drn-profile-name");
                let name = name.trim();
                if !name.is_empty() {
                    if let Ok(spec) = read_spec(&container) {
                        let name = name.replace('|', "/");
                        if let Some(store) = storage() {
                            let _ = store.set_item(
                                &format!("{PROFILE_PREFIX}{name}"),
                                &encode_spec(&spec),
                            );
                        }
                        profiles.set(list_profiles());
                    }
                }
            }
            #[cfg(feature = "pro")]
            Msg::LoadProfile(name) => {
                if let Some(encoded) = storage().and_then(|store| {
                    store
                        .get_item(&format!("{PROFILE_PREFIX}{name}"))
                        .ok()
                        .flatten()
                }) {
                    if let Some(spec) = decode_spec(&encoded) {
                        apply_spec(&container, &spec);
                        run_solve(&container, &outcome);
                    }
                }
            }
            #[cfg(feature = "pro")]
            Msg::DeleteProfile(name) => {
                if let Some(store) = storage() {
                    let _ = store.remove_item(&format!("{PROFILE_PREFIX}{name}"));
                }
                profiles.set(list_profiles());
            }
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
        if let Outcome::Solved(run) = chart_effect_outcome.get() {
            let _ = draw_envelope_chart(&canvas, &run.est);
        }
    });
    std::mem::forget(handle);

    // 4. (pro) comparison table, payload trade-off chart, saved profiles.
    #[cfg(feature = "pro")]
    {
        // Comparison: re-renders when the pinned rows change; the highlight
        // of the best value per row needs the rows only.
        let compare_holder = container
            .query_selector(".drn-compare")?
            .ok_or_else(|| JsValue::from_str("compare placeholder missing"))?;
        let compare_for_render = compare.clone();
        let handle = tpt_appfront_dom::render(
            &compare_holder,
            Rc::new(move || compare_view(compare_for_render.clone())),
            dispatch.clone(),
        )?;
        std::mem::forget(handle);

        // Payload trade-off: canvas + effect re-solving the current spec at
        // increasing payload masses on every outcome change.
        let payload_holder = container
            .query_selector(".drn-payload-chart-holder")?
            .ok_or_else(|| JsValue::from_str("payload chart placeholder missing"))?;
        let payload_canvas: web_sys::HtmlCanvasElement = document
            .create_element("canvas")?
            .dyn_into()
            .map_err(|_| JsValue::from_str("payload canvas cast failed"))?;
        payload_canvas.set_width(920);
        payload_canvas.set_height(300);
        let _ = payload_canvas.set_attribute("class", "drn-chart");
        payload_holder.append_child(&payload_canvas)?;

        let payload_effect_outcome = outcome.clone();
        let handle = create_effect(move || {
            if let Outcome::Solved(run) = payload_effect_outcome.get() {
                if run.est.feasible {
                    let cap_g = run.est.all_up_weight_g * 2.0;
                    let points = tpt_drone_engine::payload_sweep(&run.spec, cap_g, 48);
                    let _ = draw_payload_chart(&payload_canvas, &points);
                }
            }
        });
        std::mem::forget(handle);

        // Saved profiles: the list re-renders from the profiles signal.
        let profiles_holder = container
            .query_selector(".drn-profiles-list")?
            .ok_or_else(|| JsValue::from_str("profiles placeholder missing"))?;
        let profiles_for_render = profiles.clone();
        let handle = tpt_appfront_dom::render(
            &profiles_holder,
            Rc::new(move || profiles_view(profiles_for_render.clone())),
            dispatch.clone(),
        )?;
        std::mem::forget(handle);
    }

    Ok(())
}

/// Loads a built-in sample's numbers into the fields.
fn apply_sample(container: &web_sys::Element, index: usize) {
    let Some(spec) = SAMPLE_BUILDS.get(index) else {
        return;
    };
    apply_spec(container, spec);
}

/// Writes a [`BuildSpec`] into the form fields.
fn apply_spec(container: &web_sys::Element, spec: &BuildSpec) {
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
        Ok(spec) => {
            let est = solve(&spec);
            outcome.set(Outcome::Solved(Rc::new(SolvedRun {
                spec: Rc::new(spec),
                est: Rc::new(est),
            })));
        }
        Err(errors) => {
            let est = tpt_drone_engine::Estimate {
                feasible: false,
                issues: errors.into_iter().map(SolveIssue::Error).collect(),
                ..blank_estimate()
            };
            // The spec is unparsable; carry a placeholder so the payload
            // sweep and comparison have nothing to chew on.
            outcome.set(Outcome::Solved(Rc::new(SolvedRun {
                spec: Rc::new(blank_spec()),
                est: Rc::new(est),
            })));
        }
    }
}

/// A placeholder spec for runs whose fields did not parse.
fn blank_spec() -> BuildSpec {
    BuildSpec {
        name: "(invalid)".to_string(),
        kind: AirframeKind::Multirotor,
        motor: tpt_drone_engine::MotorSpec {
            kv_rpm_v: 1.0,
            resistance_ohm: 1.0,
            no_load_current_a: 0.0,
            max_current_a: 0.0,
            weight_g: 0.0,
        },
        motor_count: 1,
        prop: tpt_drone_engine::PropSpec {
            diameter_in: 1.0,
            pitch_in: 1.0,
            blades: 2,
            weight_g: 0.0,
        },
        battery: tpt_drone_engine::BatterySpec {
            capacity_mah: 1.0,
            cells_s: 1,
            pack_resistance_mohm: 0.0,
            weight_g: 0.0,
            c_rating: 0.0,
            usable_fraction: 0.8,
        },
        airframe_g: 0.0,
        payload_g: 0.0,
        drag_area_m2: 0.0,
        wing_area_m2: 0.0,
        wing_span_m: 0.0,
        cd0: 0.03,
        oswald_e: 0.8,
        cl_max: 1.1,
        cruise_speed_ms: 0.0,
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

/// Opens a URL. The free edition's wasm runs in a browser tab, where a
/// normal new-tab open is correct. The Pro edition's *identical* wasm bundle
/// is instead served over the desktop shell's custom protocol (see
/// `desktop/src/main.rs`) — there, `window.open` would replace the app
/// inside the native window with no way back, so the open goes through the
/// host's allowlisted IPC bridge and the OS default browser. Runtime
/// protocol sniffing (rather than `#[cfg(feature = "pro")]`) is what
/// distinguishes the two, since it's the hosting context that matters.
fn visit_site(url: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let in_desktop_shell = window
        .location()
        .protocol()
        .map(|p| p != "http:" && p != "https:")
        .unwrap_or(false);

    if in_desktop_shell {
        if let Ok(appfront) = js_sys::Reflect::get(&window, &JsValue::from_str("__appfront")) {
            if let Ok(post) = js_sys::Reflect::get(&appfront, &JsValue::from_str("post")) {
                if let Ok(post_fn) = post.dyn_into::<js_sys::Function>() {
                    let params = js_sys::Object::new();
                    let _ = js_sys::Reflect::set(
                        &params,
                        &JsValue::from_str("url"),
                        &JsValue::from_str(url),
                    );
                    let _ = post_fn.call2(&appfront, &JsValue::from_str("open_external"), &params);
                }
            }
        }
    } else {
        let _ = window.open_with_url_and_target(url, "_blank");
    }
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

        #[cfg(not(feature = "pro"))]
        upsell_view(b);
        #[cfg(feature = "pro")]
        pro_views(b);
        styled_text(b, "drn-foot", "Model class: momentum theory + DC-motor electrical chain, calibrated against published bench data — expect ±25-30% vs a real flight log, not lab precision. No data leaves the page.");
    })
}

/// (pro) Static shells of the three paid features; the reactive lists and
/// charts mount into the placeholders from `mount_app`.
#[cfg(feature = "pro")]
fn pro_views(b: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    b.container(|card| {
        card.heading(2, "Configuration comparison");
        styled_text(card, "drn-hint", "Solve a build, then pin it here. Pin up to four builds and race them side by side.");
        card.container(|actions| {
            actions.button("Pin current build").class("drn-btn").on_click(Msg::CompareSnapshot);
            actions.button("Clear").class("drn-ghost").on_click(Msg::CompareClear);
        }).class("drn-actions");
        card.container(|_| {}).class("drn-compare");
    }).class("drn-card");

    b.container(|card| {
        card.heading(2, "Payload trade-off");
        styled_text(card, "drn-hint", "Range and endurance as payload increases — the curves end where this build can no longer fly. Updates on every solve.");
        card.container(|_| {}).class("drn-payload-chart-holder");
    }).class("drn-card");

    b.container(|card| {
        card.heading(2, "Saved build profiles");
        styled_text(card, "drn-hint", "Save the current form under a name, reload and compare builds any time. Stored offline on this machine.");
        card.container(|row| {
            row.container(|field| {
                styled_text(field, "drn-label", "Profile name");
                field.input("").class("drn-input drn-profile-name").attr("aria-label", "Profile name");
            }).class("drn-field");
            row.container(|actions| {
                actions.button("Save current build").class("drn-btn").on_click(Msg::SaveProfile);
            }).class("drn-actions");
        }).class("drn-row");
        card.container(|_| {}).class("drn-profiles-list");
    }).class("drn-card");
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
        Outcome::Solved(run) => Rc::clone(&run.est),
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

// ---------- (pro) comparison, profiles, payload chart ----------

/// localStorage key prefix for saved profiles.
#[cfg(feature = "pro")]
const PROFILE_PREFIX: &str = "drone-range-profile:";

#[cfg(feature = "pro")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// All saved profile names, sorted.
#[cfg(feature = "pro")]
fn list_profiles() -> Vec<String> {
    let Some(store) = storage() else {
        return Vec::new();
    };
    let Ok(len) = store.length() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for i in 0..len {
        if let Ok(Some(key)) = store.key(i) {
            if let Some(name) = key.strip_prefix(PROFILE_PREFIX) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names
}

/// Compact one-line spec encoding: `v1|name|kind|n|n|...` in a fixed field
/// order. Names cannot contain `|` (sanitized at save time).
#[cfg(feature = "pro")]
fn encode_spec(spec: &BuildSpec) -> String {
    let kind = match spec.kind {
        AirframeKind::Multirotor => "m",
        AirframeKind::FixedWing => "f",
    };
    let g = |v: f64| format!("{v}");
    [
        "v1".to_string(),
        spec.name.clone(),
        kind.to_string(),
        g(spec.motor.kv_rpm_v),
        g(spec.motor.resistance_ohm),
        g(spec.motor.no_load_current_a),
        g(spec.motor.max_current_a),
        g(spec.motor.weight_g),
        g(spec.prop.diameter_in),
        g(spec.prop.pitch_in),
        g(spec.prop.blades as f64),
        g(spec.prop.weight_g),
        g(spec.battery.capacity_mah),
        g(spec.battery.cells_s as f64),
        g(spec.battery.pack_resistance_mohm),
        g(spec.battery.weight_g),
        g(spec.battery.c_rating),
        g(spec.battery.usable_fraction),
        g(spec.motor_count as f64),
        g(spec.airframe_g),
        g(spec.payload_g),
        g(spec.drag_area_m2),
        g(spec.cruise_speed_ms),
        g(spec.wing_area_m2),
        g(spec.wing_span_m),
        g(spec.cd0),
        g(spec.oswald_e),
        g(spec.cl_max),
    ]
    .join("|")
}

/// Inverse of [`encode_spec`]; `None` on any malformed field.
#[cfg(feature = "pro")]
fn decode_spec(encoded: &str) -> Option<BuildSpec> {
    let f: Vec<&str> = encoded.split('|').collect();
    if f.len() != 28 || f[0] != "v1" {
        return None;
    }
    let n = |i: usize| f[i].parse::<f64>().ok();
    let kind = match f[2] {
        "m" => AirframeKind::Multirotor,
        "f" => AirframeKind::FixedWing,
        _ => return None,
    };
    Some(BuildSpec {
        name: f[1].to_string(),
        kind,
        motor: tpt_drone_engine::MotorSpec {
            kv_rpm_v: n(3)?,
            resistance_ohm: n(4)?,
            no_load_current_a: n(5)?,
            max_current_a: n(6)?,
            weight_g: n(7)?,
        },
        motor_count: n(18)? as u32,
        prop: tpt_drone_engine::PropSpec {
            diameter_in: n(8)?,
            pitch_in: n(9)?,
            blades: n(10)? as u32,
            weight_g: n(11)?,
        },
        battery: tpt_drone_engine::BatterySpec {
            capacity_mah: n(12)?,
            cells_s: n(13)? as u32,
            pack_resistance_mohm: n(14)?,
            weight_g: n(15)?,
            c_rating: n(16)?,
            usable_fraction: n(17)?,
        },
        airframe_g: n(19)?,
        payload_g: n(20)?,
        drag_area_m2: n(21)?,
        cruise_speed_ms: n(22)?,
        wing_area_m2: n(23)?,
        wing_span_m: n(24)?,
        cd0: n(25)?,
        oswald_e: n(26)?,
        cl_max: n(27)?,
    })
}

/// The reactive comparison table: one column per pinned build.
#[cfg(feature = "pro")]
fn compare_view(compare: Signal<Vec<CompareRow>>) -> UITree<Msg> {
    let rows = compare.get();
    UITree::container(|c| {
        if rows.is_empty() {
            styled_text(c, "drn-hint", "Nothing pinned yet — solve a build, then press “Pin current build”.");
            return;
        }
        c.container(|grid| {
            for row in &rows {
                grid.container(|col| {
                    col.container(|n| {
                        n.text(row.name.clone());
                    }).class("n");
                    let metric_row = |col: &mut tpt_appfront_core::ContainerBuilder<Msg>, label: &str, value: String| {
                        col.container(|r| {
                            r.text(value);
                            r.text(format!(" {}", label));
                        }).class("r");
                    };
                    metric_row(col, "km max range", fmt_num(row.est.max_range_km));
                    metric_row(col, "min best endurance", fmt_num(row.est.best_endurance_min));
                    metric_row(col, "km cruise range", fmt_num(row.est.cruise_range_km));
                    metric_row(col, "g AUW", fmt_num(row.est.all_up_weight_g));
                    if let Some(hover) = row.est.hover_endurance_min {
                        metric_row(col, "min hover", fmt_num(hover));
                    }
                }).class("drn-compare-col");
            }
        }).class("drn-compare-grid");
    })
}

/// The reactive saved-profiles list.
#[cfg(feature = "pro")]
fn profiles_view(profiles: Signal<Vec<String>>) -> UITree<Msg> {
    let names = profiles.get();
    UITree::container(|c| {
        if names.is_empty() {
            styled_text(c, "drn-hint", "No saved profiles yet.");
            return;
        }
        for name in names {
            c.container(|row| {
                row.container(|pn| {
                    pn.text(name.clone());
                }).class("pn");
                row.button("Load").class("drn-mini").on_click(Msg::LoadProfile(name.clone()));
                row.button("Delete").class("drn-mini").on_click(Msg::DeleteProfile(name));
            }).class("drn-profile-row");
        }
    })
}

/// Draws the payload trade-off curves: range and endurance vs payload mass,
/// ending where the build stops being flyable.
#[cfg(feature = "pro")]
fn draw_payload_chart(
    canvas: &web_sys::HtmlCanvasElement,
    points: &[tpt_drone_engine::PayloadPoint],
) -> Result<(), JsValue> {
    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .map_err(|_| JsValue::from_str("context cast failed"))?;
    let w = canvas.width() as f64;
    let h = canvas.height() as f64;
    ctx.clear_rect(0.0, 0.0, w, h);

    let flyable: Vec<&tpt_drone_engine::PayloadPoint> =
        points.iter().filter(|p| p.flyable).collect();
    if flyable.len() < 2 {
        return Ok(());
    }
    let p_max = flyable.last().unwrap().payload_g;
    let r_max = flyable.iter().map(|p| p.range_km).fold(0.0, f64::max);
    let t_max = flyable.iter().map(|p| p.endurance_min).fold(0.0, f64::max);
    if r_max <= 0.0 || t_max <= 0.0 {
        return Ok(());
    }
    let pad_l = 52.0;
    let pad_r = 52.0;
    let pad_t = 14.0;
    let pad_b = 30.0;
    let px = |g: f64| pad_l + (w - pad_l - pad_r) * g / p_max;
    let py_t = |min: f64| h - pad_b - (h - pad_t - pad_b) * min / t_max;
    let py_r = |km: f64| h - pad_b - (h - pad_t - pad_b) * km / r_max;

    ctx.set_stroke_style_str("#888");
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.move_to(pad_l, pad_t);
    ctx.line_to(pad_l, h - pad_b);
    ctx.line_to(w - pad_r, h - pad_b);
    ctx.stroke();

    ctx.set_font("11px system-ui, sans-serif");
    ctx.set_fill_style_str("#888");
    for frac in [0.25, 0.5, 0.75, 1.0] {
        let y = h - pad_b - (h - pad_t - pad_b) * frac;
        ctx.set_stroke_style_str("rgba(136,136,136,.25)");
        ctx.begin_path();
        ctx.move_to(pad_l, y);
        ctx.line_to(w - pad_r, y);
        ctx.stroke();
        let _ = ctx.fill_text(&format!("{:.0} min", t_max * frac), 2.0, y + 4.0);
        let _ = ctx.fill_text(&format!("{:.0} km", r_max * frac), w - pad_r + 4.0, y + 4.0);
    }
    for frac in [0.0, 0.5, 1.0] {
        let _ = ctx.fill_text(
            &format!("{:.0} g", p_max * frac),
            px(p_max * frac) - 10.0,
            h - 8.0,
        );
    }

    let curve = |select: fn(&tpt_drone_engine::PayloadPoint) -> f64,
                 map_y: &dyn Fn(f64) -> f64,
                 color: &str,
                 width: f64| {
        ctx.begin_path();
        let mut started = false;
        for p in &flyable {
            let x = px(p.payload_g);
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

    let legend = [
        ("endurance (left)", "#0e7c6b"),
        ("range (right)", "rgba(120,130,150,.9)"),
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
