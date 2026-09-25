//! Free WASM edition UI (tpt-appfront-dom), single build configuration.
//!
//! Phase 0 placeholder: proves the mount contract end-to-end. The real UI
//! lands in Phase 2 — see `todo.md` in the repo root.

use tpt_appfront_core::UITree;
use wasm_bindgen::prelude::*;

pub fn mount_app(container: &web_sys::Element) -> Result<(), JsValue> {
    let ui: UITree<&'static str> = UITree::container(|b| {
        b.heading(1, "TPT Drone Range");
        b.text("Phase 0 scaffold proof — solver and UI land next.");
    });
    tpt_appfront_dom::mount(
        container,
        &ui,
        std::rc::Rc::new(|_: &'static str| {}),
    )?;
    Ok(())
}
