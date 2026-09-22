# 26 — The mapper: a standalone map explorer, embeddable in Hydra

**Author's request, 2026-09-22:** *"I think what I want is to build a mapper crate.
Something that will work standalone but also can be embedded into hydra as it's 'map
explorer/editor'. I think we should take Vellum's layout engine and improve it with what
we've learned."*

Scoped through a short round of decisions (recorded in §1) rather than assumed.

**MOVED OUT, 2026-09-22, after being built here first.** `cena-map-layout` and
`cena-mapper` were built in this workspace, verified (12 tests: unit coverage plus a
hand-built fixture town exercising the whole pipeline), restructured to Cena's own
clippy bar, then moved to their own repo, **[`Nisugi/hydra-mapper`](https://github.com/
Nisugi/hydra-mapper)** — the author's correction: *"I actually wanted the standalone
mapper to be standalone ... you know like it's own github, and functions without
cena."* Mirrors `Nisugi/hydra-mapdb`'s shape exactly: both crates depend on `cena-map`
as a git dependency on `Nisugi/cena`, not a vendored copy, and neither crate exists in
this workspace anymore. This document's design record stands — the crate shape, the
port decisions in §3, the verification in §4 — as the reasoning for what lives in that
repo now, cited from there rather than duplicated.

The relationship runs both ways: `hydra-mapper` pulls `cena-map` from here to read a
`.map` file, and this workspace is expected to pull `cena-map-layout` from
`hydra-mapper` once Hydra's own GUI (M4+) needs to draw a map inside the client itself
— not done yet, and not stubbed in ahead of that use (`plan/05` Rule -1).

## 0. What this is not

- **Not an editor yet.** V1 renders a layout; it does not write corrections back. Vellum's
  override system (position pins, edge overrides, classification flips, §5) is read as a
  reference for later, not ported now.
- **Not new floor/elevation work.** The Jev trial (`research/jev-trial/`,
  `plan/22-jev-trial.md`) spent a long session on world elevation and concluded, in its own
  §11p, that **it is irrelevant to this crate's job.** Render placement (which plate a room
  draws on) is a room-graph packing problem, decided by the same three constraints
  Simutronics' own baker enforces — cell collision, edge direction, no crossings — with no
  floor number as input. Vellum's engine already treats up/down this way (borrowed as N/S
  offsets in the plane, §9 of its spec, "out of scope: up/down layering"). This crate does
  the same, keeps doing it, and the elevation apparatus stays in `research/jev-trial/`
  where it is available if a later feature (not this one) needs real height.
- **Not a rewrite.** Vellum's `src/core/layout_engine/` (4,572 lines, its own
  `docs/layout-engine-spec.md`, tested against five real zones up to 3,227 rooms) is a
  working, spec'd, validated implementation of exactly this problem. It is ported, not
  redesigned from a blank page.

## 1. Decisions made (2026-09-22)

| question | decision | why |
|---|---|---|
| Input model | Consumes `cena_map::Room`/`Exit`, not a second parallel Room type | One room model for the whole app. `Crossing::Command` still carries the plain-move text Vellum's direction analyzer reads; scripted/routine/hub exits are non-directional edges, same treatment Vellum already gives a `;e` stringproc it cannot resolve. |
| Standalone form | A real window app, not just a library | The author wants something to actually open and look at now, not only an embeddable module with no way to see it run. |
| Editor vs explorer | Explorer first — render only, no write-back | Matches how Travel was built: the walker before targets, targets before the save format. Get a correct, browsable map before designing a correction format. |
| Rendering stack | `egui`/`eframe` (the author's own fork, already used by Vellum) | Proven at exactly this job (panels, custom canvas drawing) and the fork is already a known quantity. If Hydra's own GUI (M4+) also picks egui, this slots in as a panel with no rendering-stack change. |
| Crate split | Two crates: a pure layout engine + a thin window binary | Matches the workspace's existing pattern (`cena-map` / `cena-mapdb-convert`, a pure crate and its tool kept apart). The pure crate has zero egui dependency, so a future Hydra GUI panel can depend on it directly without pulling in a second app's `main.rs`. |

## 2. Crate shape

```
crates/
  cena-map-layout/     pure: direction analysis, BFS placement, hill-climb + compaction,
                        interior classification, cluster packing, scene model.
                        depends on: cena-map. No egui, no file I/O, no window.
  cena-mapper/          the standalone window.
                        depends on: cena-map, cena-map-layout, egui, eframe.
                        src/main.rs -- eframe::run_native, loads a .map file (CENA_MAP,
                        same env var travel.rs already uses), lets a person pick a
                        location and see it laid out.
```

`cena-map-layout` is the crate a later Hydra GUI panel depends on directly. `cena-mapper`
exists so there is something to run today, and stays thin: window setup, file loading, and
drawing `MapScene` — no algorithm of its own.

## 3. What ports from Vellum, and what changes

Source: `reference/VellumFE/src/core/layout_engine/` and its spec,
`reference/VellumFE/docs/layout-engine-spec.md` (read in full before writing any of this;
cited by section below).

| Vellum module | Ports to | Changes for `cena_map::Room` |
|---|---|---|
| `direction.rs` (spec §3) | `cena-map-layout::direction` | `ExitKind::Cardinal` is a bare tag (VERIFIED: `crates/cena-map/src/exit.rs:219` carries no direction value), so the word-boundary scan Vellum always runs is still needed to learn *which* of the 8 it is. But an exit already typed `ExitKind::Vertical` or `ExitKind::Out` skips the ambiguity Vellum's string-sniffing has to resolve by hand (spec §3 step 2) — Hydra's converter already did that classification once. `dirto` has no equivalent in `cena_map::Exit` yet (VERIFIED: not a field on `Exit` or `Room`); until the override system exists (out of scope, §0), direction comes from the command text and `ExitKind` alone. |
| `positioner.rs` (spec §4–5) | `cena-map-layout::positioner` | Direct port. BFS placement, grid rips, per-component hill-climb, compaction. Room ids are `RoomId` not `u32`; otherwise unchanged. |
| `classifier.rs` (spec §6) | `cena-map-layout::classifier` | The indoor/outdoor vote reads `Room::paths` (VERIFIED: `cena_map::Room` keeps the raw "Obvious exits/paths" lines, `crates/cena-map/src/room.rs:61-63`) exactly as Vellum's `paths` field does. `climate`/`terrain` weatherless fallback needs checking against whatever `cena_map::Room` currently keeps of those fields (TODO before coding: confirm they survived conversion, not assumed). |
| `packer.rs` (spec §7) | `cena-map-layout::packer` | Direct port of the three-pass packing (image anchors, connector BFS, strip fallback) and the interior shelf. `image`/`image_coords` need the same check as climate/terrain: confirm what `cena_map::Room` kept from upstream's hand-drawn overlay anchors before assuming pass 1 has anything to anchor on. |
| `scene.rs` | `cena-map-layout::scene` | Direct port: the presentation model (`MapScene`, `Sheet`, stub/dashed-edge rules) egui draws from. No egui types inside this module — it stays pure geometry and enums, `cena-mapper` is what turns it into `egui::Shape`s. |
| `overrides.rs` (spec §8) | Not ported yet | Read as the reference for the future editor (§0). |
| `cache.rs` | Not ported yet | Revisit once there's a reason to cache (a live GUI regenerating on area entry); the standalone explorer loads a whole map file once and does not need it for v1. |

## 4. Verification before coding

Three checks the plan above depends on and marks TODO rather than assumed:

1. Does `cena_map::Room` still carry `climate`/`terrain`, and in what shape, after
   conversion? Needed for classifier.rs's weatherless fallback (spec §6 step 3).
2. Does `cena_map::Room` keep `image`/`image_coords`? Needed for packer.rs pass 1 (spec §7).
   If neither survived conversion, pass 1 never fires and every zone falls through to the
   connector-BFS/strip passes — correct behaviour, but worth knowing going in rather than
   discovering as a silent always-miss.
3. `Room::paths` is `Vec<String>` in `cena_map` (plural) versus Vellum's single joined
   `String`. Confirm the join behaviour (comma-joined, Vellum's own `String(array)`
   semantics, spec §2) is either reproduced or deliberately dropped, not accidentally lost.

## 5. Rough plan of work

Not milestoned yet — this is the shape, not a schedule the author has approved section by
section.

1. Answer §4's three checks against a real converted map (`cena-map`'s own fixtures or a
   full `hydra.map`).
2. Port `direction.rs`, with unit tests carried over from Vellum's (`tests/layout_engine.rs`,
   `tests/fixtures/layout`) adapted to `cena_map` types.
3. Port `positioner.rs`, `classifier.rs`, `packer.rs`, `scene.rs` in that order — the
   pipeline order `mod.rs` already documents.
4. Reproduce the statistical targets table (spec §9) against the same five real zones
   (Moonsedge, the Atoll, Mist Harbor, Icemule Trace, Wehnimer's Landing) converted through
   `cena_map`, not just against Vellum's own fixtures — the numbers are expected to drift
   slightly (iteration order, `plan/21`'s own converted shapes) and large drift is a logic
   bug, per the spec's own words.
5. `cena-mapper`: an `eframe` window, `CENA_MAP` env var, a location picker, and a canvas
   that draws `MapScene`.
