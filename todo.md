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
>
> **Resolution on the base crate:** `tpt-transportation` does not exist on
> this machine (the `2 WIP/TPT Transport` repo is an unrelated PHP web app).
> The engine crate is therefore self-contained pure Rust (momentum theory +
> DC-motor electrical chain), following the cutlist-optimizer precedent where
> the solver crate carries the domain physics with zero UI deps. Validated
> against published flight-time classes in `engine/tests/validation.rs`.

## Phase 0 — Scaffold & pipeline proof
- [x] Scaffold the crate to the hub contract (hand-structured like
      fea-lite/cutlist-optimizer rather than via `tpt-appfront init` — the
      reference apps define the layout the build scripts and hub expect)
- [x] `#[wasm_bindgen] pub fn mount(el)` + `#[wasm_bindgen(start)]` no-op
      unless `#tpt-appfront-root` present
- [x] `index.html` carries the `#tpt-appfront-root` marker div
- [x] Build pipeline scripts mirroring fea-lite's (`build.ps1` / `build.sh`:
      `--target web` glue → web repo `public/apps/drone-range/`)
- [x] Hello-world build loads through the standalone dev page (`trunk serve`)
- [x] git init + first commit (b9655c8)
- [x] Note: base repo `tpt-transportation` — confirmed absent; engine written
      self-contained (see resolution note above)

## Phase 1 — Solver integration
- [x] Flight-performance model: prop Ct/Cp coefficients (UIUC-anchored),
      DC-motor back-EMF/phase-current equations, pack sag, momentum-theory
      induced power (multirotor) and lift/drag polar with Oswald efficiency
      (fixed wing), usable-fraction energy budget
- [x] `engine/` adapter crate: motor/prop/battery/airframe/payload specs in →
      range, endurance, hover, feasibility issues out as plain structs — no
      UI deps, host-unit-testable
- [x] Validation against published builds (DJI Mini 2 class, 1.5 m
      long-range wing class, 5" freestyle bench-current band) +
      physics-invariant tests — 12/12 pass, clippy clean

## Phase 2 — Free WASM UI
- [x] Single build-configuration input (motor, prop, battery, airframe,
      payload) + range/endurance estimate + flight-envelope chart
- [x] Free-tier upsell copy pointing at multi-configuration comparison,
      payload trade-off analysis, saved profiles
- [x] Browser-verified: mount, samples (both kinds), solve, results, issue
      rendering, kind toggle, parse-error path — via trunk serve

## Phase 3 — Paid desktop edition (cargo feature `pro`)
- [x] `pro` gates: multi-configuration comparison (pin up to 4),
      payload trade-off sweep (engine `payload_sweep`, pro-gated), saved
      build profiles (localStorage, offline)
- [x] Pro UI: comparison columns, payload trade-off chart, saved profiles
      list (save/load/delete) — browser-verified on the pro bundle
- [x] Desktop shell via `tpt-appfront-webview`; Windows exe build + smoke
      test (launches; embedded-dist extraction verified from a neutral cwd)
- [ ] Gumroad product from apps.md listing copy ($29), exe upload, URL into
      registry — **manual step** (needs a browser login; see `GUMROAD.md`;
      registry entry already carries a `proFeatures` list and a gumroad
      comment ready to fill)

## Phase 4 — Ship & measure
- [x] Registry entry (`engineering` category, `wasm: { entry:
      '/apps/drone-range/drone-range.js' }`, `status: 'live'`, proFeatures,
      gumroad comment) in `tpt-electrician-nz-2`
- [x] Verified hub card/detail/runner in dev (card on /tools with
      Engineering filter; detail page mounts the app and solves embedded).
      Sitemap is generated from the registry on the next build. Prod check
      happens after the next deploy of the web repo.
- [x] WASM size budget check on the shipped build: 89.4 KB gzipped
      (~223 KB raw) — far under the ~1 MB budget
- [x] Pro exe packaged at `target/release/tpt-drone-range-pro.exe` (zip it
      for Gumroad — see `GUMROAD.md`)
