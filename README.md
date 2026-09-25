# TPT Drone Range

Flight-performance estimator for custom drone builds: motor/prop/battery/
airframe/payload specs in, range, endurance, and payload trade-offs out.
Spec: `apps.md` §T1-15 in the `tpt-electrician-nz-2` repo. Hub listing:
`tptsolutions.co.nz/tools/drone-range` (slug `drone-range`, category
`engineering`).

Two editions from one codebase:

- **Free WASM edition** (default) — single-configuration range/endurance
  estimate, served on the tools hub. `wasm-bindgen --target web` glue
  exposing `mount(container)` for the hub's `WasmAppRunner`; standalone
  `trunk serve` mounts into the `#tpt-appfront-root` marker div.
- **Pro desktop edition** (`--features pro`) — multi-configuration
  comparison (pin up to four builds), payload trade-off analysis, and saved
  build profiles, hosted by the `desktop/` WebView2 shell whose exe embeds
  the whole bundle. $29 USD one-time on Gumroad (listing copy in
  `GUMROAD.md`).

## Layout

- `engine/` — `tpt-drone-engine`: the model, in pure Rust with no UI deps.
  Actuator-disk momentum theory + DC-motor electrical chain (multirotor),
  lift/drag polar with Oswald efficiency (fixed wing). Validated against
  published flight-time classes in `engine/tests/validation.rs`. Accuracy
  class ±25-30% — a rule-of-thumb replacement, not a wind-tunnel.
- `src/` — the appfront DOM UI. The input panel mounts once (fields are read
  from the DOM at solve time); results re-render fine-grained from the
  `outcome` signal; charts are raw web-sys canvases.
- `desktop/` — `tpt-drone-desktop`: the `tpt-appfront-webview` shell.
  `build.ps1 -Pro` runs the trunk pro build first so `include_dir!` can
  embed `dist/` into the exe — the shipped file is the exe alone.

## Build

```powershell
./build.ps1          # free wasm-bindgen bundle -> web repo public/apps/drone-range/
./build.ps1 -Pro     # trunk pro bundle -> dist\, then target\release\tpt-drone-range-pro.exe
./build.sh           # same on macOS/Linux (--pro)
```

`TPT_WEB_REPO` overrides the web repo destination. Engine tests:
`cargo test -p tpt-drone-engine` (add `--features pro` for the sweep).
