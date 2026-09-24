# Despana map explorer — first integration

This brings the independently tested atlas presentation into Despana's existing
loopback server. Open **Map explorer** from the character hub or a character page.
It opens in a separate tab so the live story and command entry stay available.

This document records the first, offline integration. The separate read-only
character minimap follow-up is documented in [plan 30](30-despana-live-map.md).

## What is included

- World connections and canonical room search; region overview with Map and
  Places tabs; individual area/focus views.
- Prominent, named transitions, including every recorded catacomb boundary.
  Destination clicks can cross region bundles. Direction is preserved: an exit
  never creates a return connection.
- Service/landmark icons, configurable colors, optional labels, obstacle-aware
  label placement and enlarged skeleton hit targets.
- Creature reference panels and exact habitat-UID overlays. Unknown means
  unknown, not creature-free. Highlight patches are not approved hunt boundaries.
- Animated, read-only route previews from an explicitly selected preview origin.
  Catacomb shortcuts show separate visits; they do not draw a false surface edge.
  Right-clicking the destination or using Clear route removes the preview.
- Native layout by default, with an explicit artwork-coordinate comparison.
  Artwork positions do not rewrite exits, ownership, floor levels or path costs.

The browser never opens a game WebSocket and has no command API. This is **not**
the live minimap, map editor, or M6 hunting setup. The snapshot is independent of
the running character's `CENA_MAP`; the UI does not claim they match. No live
character position, reachability, membership restriction or safety is inferred.

## Sources and limits

The initial snapshot was built from Fable's 2026-09-24 area-fill `gs.map`:

`d5e8ac60659b09dac49f0bf75e9cf2e700b22d8f716cc1b12c0ef407ba22ceff`

The build includes 33,779 live/closed rooms across 16 region groups. Gone and
virtual rooms are excluded; closed rooms remain visible but cannot enter a
preview route. Complete areas can appear as context in another region, but
canonical search and world links retain the native owning region.

Fable described the new area fills as machine placements using title/location
matches and single-area dead-end pockets. They are not individually approved
boundaries. Missing assignments remain explicit provisional display groups.
No earlier Jev classification or human approval is silently promoted into this
new map's ownership. Our added data is the provenance-bearing habitat/reference
layer and presentation rules, not a second competing area authority.

Native geometry comes from `cena-map-layout` at
`e98f669e4783b2613ed4f0bf0f685d4743108cba`; map decoding uses Cena
`671bc326453fb8ef4b74c0a1be62bd9563164695`. Both are locked in the offline export
tool. This does not import the standalone mapper UI or copy its solver.

The creature catalogue is a frozen subset of installed Lich5 data, with source
filename/content hashes. Only literal data is read; Ruby is never executed.
The BSD 3-Clause notice is shipped and linked from the world page. Game room
descriptions remain GemStone IV / Simutronics content. Wiki links are references,
not a claim of fresh verification. Service highlights use native metadata and
conservative title hints, not a complete verified business registry.

## The boundary between native and browser code

`cena-web::atlas` serves an explicit compile-time allowlist under `/atlas/`.
The existing Host/Origin guard, no-query rule, CSP, no-store and framing policy
apply unchanged. No arbitrary filesystem paths, external asset hosts, browser
tokens, credentials or additional native runtime dependencies are introduced.

`mountAtlas(root, {source, storage, initialHash, onNavigate, onOutside})` owns one
view's state. `offlineSource` loads a region and verifies the habitat sidecar's
SHA-256 against the actual map response. A stale reference layer is withheld
with a warning; the map itself remains usable. The shell handles URLs and exact
cross-region navigation. The renderer does not know how to control a character.

JSON snapshots are embedded as inspectable text, not opaque binary payloads.
The architecture guard still bans `include!` and `include_bytes!`; `.mjs` browser
modules and `.txt` license notices are documented inert text extensions. This
costs about 80 MB uncompressed data in the first version (about 11 MB compressed).
Each regional bundle is fetched on demand, not the entire corpus at startup.

This intentionally establishes the reusable presentation/data boundary before
adding live state. A later native provider can supply scenes from the running
map and native located-room identity. It must not treat an observed game UID
as a Lich room ID, nor run a parallel travel engine. M6 hunting selection and
field/town recovery profiles belong to the behavior layer, not this renderer.

## Build and review

No game credentials are needed to review the integration:

```sh
cargo run -p cena-web --example atlas_preview
```

Open the printed local URL. This example creates no session and has no game
connection. Stop it with Ctrl-C. Normal Despana users use the header link.

The offline generation tool is a separate workspace, so rebuilding normal
Hydra does not fetch a layout-engine git dependency or rerun map generation.
See `tools/atlas/README.md` for reproducible snapshot generation and checks.

Checks cover native membership/cross-region copies, all world directed records,
sidecar hashes, closed/conditional route exclusions, label/hit-target regressions,
and a real browser against the native HTTP server and its unchanged CSP.
These checks are not a claim that all layouts or hunting boundaries have been
visually reviewed. No live travel or combat testing is part of this PR.
