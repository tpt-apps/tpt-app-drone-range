# TPT Drone Range — Build Todo

> Hub listing: `tptsolutions.co.nz/tools/drone-range` (registry slug
> `drone-range`, category `engineering`). Spec: `apps.md` §T1-15 in the
> tpt-electrician-nz-2 repo. Free WASM edition: single-configuration
> range/endurance estimate. Paid offline edition ($29 USD, Gumroad):
> multi-configuration comparison, payload trade-off analysis, and saved build
> profiles. Solver wraps `tpt-transportation`. UI on `tpt-appfront`.
>
> Hub contract: `wasm-bindgen --target web` glue exposes `mount(container)`
> (called by `WasmAppRunner` after `default()` init). Standalone `trunk serve`
> mounts into the `#tpt-appfront-root` marker div instead — never
> `document.body` — so the two hosts never double-mount.

## Phase 0 — Scaffold & pipeline proof
- [ ] `tpt-appfront init tpt-app-drone-range --target dom` (path deps on the tpt-appfront checkout)
- [ ] Restructure scaffold to hub contract: `#[wasm_bindgen] pub fn mount(el)` + `#[wasm_bindgen(start)]` no-op unless `#tpt-appfront-root` present
- [ ] `index.html` carries the `#tpt-appfront-root` marker div
- [ ] Build pipeline script mirroring fea-lite/cutlist-optimizer's (`--target web` glue → web repo `public/apps/drone-range/`)
- [ ] Hello-world build loads through the standalone dev page (`trunk serve`)
- [ ] git init + first commit
- [ ] Note: base repo is `tpt-transportation` — confirm exact repo name/location
      on this machine (not found under `Open Source/` at planning time; may
      need to be located or created first)

## Phase 1 — Solver integration (wrap tpt-transportation)
- [ ] Survey the transportation base crate for any multirotor/fixed-wing
      flight-performance modeling (thrust/current draw from motor/prop specs,
      battery discharge modeling, drag from airframe)
- [ ] Adapter crate `engine/`: motor/prop/battery/airframe/payload spec in →
      range, endurance, payload trade-off out as plain structs — no UI deps,
      host-unit-testable
- [ ] Validation against a real published drone build's measured
      range/endurance as a sanity check

## Phase 2 — Free WASM UI
- [ ] Single build-configuration input (motor, prop, battery, airframe,
      payload) + range/endurance estimate
- [ ] Free-tier upsell copy pointing at multi-configuration comparison,
      payload trade-off analysis, saved profiles
- [ ] Browser-verified: mount, calculate, results render correctly

## Phase 3 — Paid desktop edition (cargo feature `pro`)
- [ ] `pro` gates: multi-configuration side-by-side comparison, payload
      trade-off analysis (range vs. payload curve), saved build profiles
- [ ] Pro UI: multi-config comparison table/chart, payload trade-off chart,
      saved profiles list
- [ ] Desktop shell via `tpt-appfront-webview`; Windows exe build + smoke test
- [ ] Gumroad product from apps.md listing copy ($29), exe upload, URL into
      registry — manual step, listing copy at apps.md §T1-15

## Phase 4 — Ship & measure
- [ ] Registry entry (`engineering` category, `wasm: { entry: '/apps/drone-range/drone-range.js' }`)
      per the "Adding a finished app to the hub" runbook at the bottom of apps.md
- [ ] Verify hub card/detail/sitemap/runner in dev, then prod after deploy
- [ ] WASM size budget check on the shipped build
