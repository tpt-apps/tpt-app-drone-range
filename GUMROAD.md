# Gumroad listing

Everything needed to create/update the Gumroad product once this app is
built. Source copy is `apps.md` §T1-15 in the `tpt-electrician-nz-2` repo — if
that changes, update here too (and vice versa).

## Step-by-step: publish this listing

1. Build the Pro edition (`./build.ps1 -Pro`, once the build pipeline exists
   — see `todo.md` and mirror `tpt-app-fea-lite`/`tpt-app-cutlist-optimizer`'s
   pattern) — produces the packaged exe/zip in `release/`.
2. Log in to Gumroad → **New product** → **Digital product**.
3. Set the product name, price, and upload the packaged file (all below).
4. Paste the short description, bullets, and full description (below) into
   their respective fields.
5. Set category/tags (below) and add a cover image (see note below).
6. Publish, then copy the resulting product URL (`https://gum.co/...`).
7. Paste that URL into the `drone-range` entry's `gumroadUrl` field in
   `tpt-electrician-nz-2\src\lib\tpt-apps-registry.ts`, and set `price: '$29'`
   on the same entry.
8. Flip that entry's `status` to `'live'` once both the free demo and this
   Gumroad listing exist.
9. Commit and push the registry repo.

## File to upload

```
target\release\tpt-drone-range-pro.exe
```

Produced by `./build.ps1 -Pro` (trunk pro bundle → dist\, then the native
webview exe). The exe embeds its whole web bundle at compile time and
extracts it to the user's temp dir on first launch — ship the exe **zipped**
(Gumroad wants an archive; some browsers also flag bare exes):

```powershell
Compress-Archive -Path target\release\tpt-drone-range-pro.exe `
  -DestinationPath release\TPT-Drone-Range-Pro.zip -Force
```

Smoke test before upload: double-click the exe — the app window opens with
the comparison, payload trade-off, and saved-profiles panels present and no
"Pro edition" upsell card (that card only exists in the free build).

## Product name

```
TPT Drone Range — Flight Performance Calculator for Custom Builds
```

## Price

**$29 USD, one-time purchase** (no subscription). Kept accessible — this is
the one Tier 1 app with a genuine hobbyist audience alongside professionals.

## Short description / summary field

```
Estimate range, endurance, and payload trade-offs for custom drone builds — replace spreadsheet guesswork with a real performance model.
```

## Bullets / feature list

```
Range/endurance estimation
Multi-configuration comparison
Payload trade-off analysis
Saved build profiles
One-time purchase
```

## Full description (long-form field)

```
TPT Drone Range estimates range, endurance, and payload trade-offs from
motor/prop/battery/airframe specs — the calculation serious builders
currently do with spreadsheets and rough rules of thumb.

Who it's for: drone hobbyists and small commercial drone operators
designing or modifying custom multirotor/fixed-wing builds who need real
flight-performance estimates rather than guesswork.

Pro edition (this download):
- Multi-configuration comparison
- Payload trade-off analysis
- Saved build profiles

Try the free single-configuration estimate first, right in your browser, no
install: tptsolutions.co.nz/tools/drone-range

Requirements: Windows 10 (20H2+) or Windows 11. Uses the WebView2 runtime,
preinstalled on both — nothing extra to download.
```

## Category / tags

- Category: **Software > Utility** (or Gumroad's closest equivalent)
- Tags: `drone-engineering`, `uav`, `multirotor`, `flight-performance`,
  `hobbyist-tools`, `windows`

## Cover image / thumbnail

Not created yet. A screenshot comparing two build configurations
side-by-side is the most convincing cover once built. No stock/AI art.
Source for the shot: run the Pro exe, load the freestyle sample, pin it,
then load the long-range wing sample and pin that — the comparison table
plus the payload chart fills a clean cover crop. (The free build's
flight-envelope chart in light mode also works as a backdrop.)

## After publishing

1. Copy the resulting product URL (`https://gum.co/...`).
2. Paste it into the `gumroadUrl` field of the `drone-range` entry in
   `tpt-electrician-nz-2\src\lib\tpt-apps-registry.ts`, and set `price: '$29'`.
3. Set `status` to `'live'`.
4. Commit and push the registry repo.
