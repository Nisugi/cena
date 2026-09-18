# Cena Testing Strategy

> **STATUS: BINDING except where it prescribes Lua.** [`12-implementation-spec.md`](12-implementation-spec.md) is authoritative. The nine test categories, the layer table and CI rules hold. **VOID:** guest-language sandbox tests, conformance-to-Lua-API tests, and "Phase 5 scheduler stress" in §6 — see `12` §1.


Written 2026-09-17. What we test, at which layer, with what kind of test — and which of
these are load-bearing rather than nice-to-have.

Companion to [`05-engineering-rules.md`](05-engineering-rules.md). The layer order in
[`01-architecture.md`](../research/01-architecture.md) was chosen *specifically* to make this possible:
**each layer is testable without the layer above it.** That property is the whole strategy;
everything below is consequence.

---

## 0. What the reference codebases prove

Before inventing anything, look at what these projects actually built. Both are
test-heavy, and their test *shapes* are more instructive than their counts.

| | VellumFE (Rust) | Lich-5 (Ruby) |
|---|---|---|
| Inline unit tests | **3,488** `#[test]` across 240 `#[cfg(test)]` modules | — |
| Integration tests | **10,435 lines** across 16 files in `tests/` | — |
| Spec suite | — | **88,921 lines** under `spec/` |
| Fixtures | 55 files in `tests/fixtures/` | `spec/fixtures/` |
| Structure | mirrors `src/` | *"Spec files mirror the `lib/` directory structure"* (`spec/README.md`) |
| CI | per-target builds | 16 workflows incl. `rspec_tests`, `rubocop`, `ruby_syntax`, `windows_active_sessions` |

Two conclusions:

1. **Test layout mirrors source layout, in both.** Independently arrived at. Rule below.
2. **The valuable tests are not unit tests.** Vellum's most interesting files —
   `parser_characterization.rs`, `chokepoints.rs`, `wheel_parity.cjs` — are none of the
   textbook categories. They are the ones worth stealing.

---

## 1. The test categories

Nine kinds. Not all are equally important; the ones marked **load-bearing** are the ones
that will actually save this project.

### 1.1 Unit tests — *supporting*

Pure functions and single types: parsing one tag, one ladder state transition, one skill
curve. Rust convention: inline `#[cfg(test)] mod tests` in the same file.

**Rule:** any function with branching logic gets unit tests. Any *pure* function gets them
before it gets an integration test.

*Why only "supporting":* unit tests confirm you built what you meant. They do not confirm
you meant the right thing. Every serious bug in a game client lives between components.

### 1.2 Golden / characterization tests — **LOAD-BEARING**

**The single most valuable category for Cena.** Vellum's `parser_characterization.rs` says
it plainly:

> Pins the parser's exact output over a corpus of real captured wire lines plus
> known-nasty edge cases, so behavior changes … show up as reviewable snapshot diffs
> instead of silent regressions.

And it ships a regeneration protocol:

```
UPDATE_PARSER_GOLDEN=1 cargo test --test parser_characterization
```

> then review the diff of `tests/data/parser_golden.snap` in the commit.

**Why this matters more here than in most projects:** Cena's parser faces an *external
protocol we do not control*. Simutronics changes it without notice. A golden corpus turns
"did my refactor change parsing?" from an unanswerable question into a reviewable diff.

**Rules:**
- The parser has a golden corpus of **real captured transcripts** from day one (Phase 1).
- Regenerating a golden file is an explicit, flagged act, and the diff is reviewed in the
  commit. Never regenerate to make a test pass.
- Fixtures are real wire bytes, not hand-written approximations. Vellum has 55.

### 1.3 Integration tests — *important*

Several components across one layer boundary: transport→parser, parser→model,
model→events. Rust convention: `tests/` directory, one file per concern.

**Rule:** every layer boundary in §1 of the engineering rules has at least one integration
test that exercises it with realistic data.

### 1.4 Replay tests — **LOAD-BEARING**

Feed a **recorded game session** end-to-end through transport→parser→model→events and
assert the resulting state. This is the closest thing to testing against the real game
without the real game.

Lich already does this: PR #1559 is *"combat module — event defs, observer emissions,
**replay-verified parsing**, SQLite recorder."* They built a recorder to capture sessions
and verify the parser against them. **Cena should build the recorder in Phase 1**, not
later — every session any developer plays becomes a test fixture.

**Rule:** a bug found in live play is reproduced as a replay fixture before it is fixed.

### 1.5 Property tests — *important where it fits*

Generate inputs, assert invariants. (`proptest`/`quickcheck`.)

High-value targets here:
- **The parser never panics on arbitrary bytes.** Non-negotiable — hostile or corrupt input
  must degrade to `Frame::Unknown`, never crash a session.
- Round-trips: snapshot→serialize→deserialize is identity.
- The send ladder terminates: no input sequence causes unbounded resends.
- Pathfinding: a returned route is always walkable.

### 1.6 Architecture tests — **LOAD-BEARING**

Enforce the structural rules. Vellum's `tests/architecture.rs` is 359 lines of
string-scanning: `core_and_data_do_not_reference_frontend`, `gui_does_not_reference_tui`,
`core_is_android_safe`, `config_root_stays_a_facade`, `split_parents_stay_facades`,
`server_time_offset_has_a_single_owning_field`.

**The empirical finding from the decision interrogation is the important part: Vellum's
line caps were never once raised.** Mechanical enforcement *held*. But the caps arrived at
roughly 250K lines, *after* the author had hand-split a 9,227-line `impl` block
(commit `d9b875bb`, 2026-08-09), and the follow-up commit `f022411b` says the new facade
test exists *"so the monoliths can't silently re-form."*

**So: enforcement ratchets a refactor you already did; it does not do the refactor for
you.** Start with the ratchet in place and you never need the refactor.

**Rules:**
- Cena is a **workspace of crates**, so most layering rules are compiler-enforced. The
  architecture test covers only what the compiler cannot express (banned crate paths in a
  layer, mobile-safety, file-size caps, facade discipline).
- Every rule in `05-engineering-rules.md` tagged *"Enforced by: architecture test"* has
  that test **written when the rule is adopted**.
- Per-file line caps from day one, with an allowlist requiring a justifying comment.

### 1.7 Parity tests — **LOAD-BEARING for us specifically**

Two implementations of the same logic, driven by **one shared golden truth table**, so they
cannot drift. Vellum's `wheel_parity.cjs`:

> Drives `src/frontend/web/assets/wheel-core.js` — **the EXACT bytes the phone runs** —
> through `tests/data/wheel_golden.json`, the same truth table the Rust machine is tested
> against … A change on one side only turns the other side's run red instead of the phone
> firing a different slice than the desktop.

**Cena needs this in at least three places:**
- Rust core vs. browser-side JS for anything the web frontend computes locally.
- **GemStone vs. DragonRealms adapters** — shared behavior must agree where it should.
- Any Lua-side helper that mirrors Rust logic.

### 1.8 Contract / conformance tests — **LOAD-BEARING**

Does the Lua API actually let someone build a real script? The conformance target is
**eohunter** (`01-architecture.md` §9 Q2): ~18,000 lines written against a modern core with
an explicit dependency list.

**Rule:** maintain a **capability checklist** derived from eohunter's
`core-consumption-audit.md` plus the corpus census tiers, as an executable test that fails
when a Tier-0/Tier-1 capability regresses. This is how "can Cena's Lua API express a real
hunting engine?" becomes a build-time question instead of an opinion.

Also here: **the script sandbox holds.** A script cannot reach the filesystem, the network,
or another session, except through sanctioned APIs. Test the negative cases explicitly.

### 1.9 Performance tests — *important, with a trap*

Vellum ships `tests/bench_parse.rs`. Cena's hot path is measured in
[`../inventory/09-hot-path-measurements.md`](../inventory/09-hot-path-measurements.md):
~2,395 crit patterns + 1,082 combat-def regexes, per line, per session, on a phone.

**Rules:**
- Benchmark the multi-pattern matcher against a **real transcript**, not synthetic lines.
- Benchmark on a **mobile-class target**, not just a desktop. Desktop numbers will lie.
- Track regressions in CI as a trend; fail only on a large regression, to avoid flakiness.

> **The trap, restated because it is the archetype:** Lich measured that many small literal
> unions beat one big union (58µs vs 35µs/line, `combat/parser.rb:34-40`). True in Ruby,
> because Ruby scans alternations linearly. **Rust's `RegexSet`/`aho-corasick` invert it.**
> Never port a performance conclusion; port the discipline of measuring, then measure
> *here*.

---

## 2. What to test at each layer

| Layer | Primary | Also | Needs no… |
|---|---|---|---|
| `cena-platform` | unit | property (paths) | network, UI |
| `cena-protocol` | **golden**, property (never panics) | unit, bench | network, UI, model |
| `cena-model` | **replay**, unit | property, parity (GS vs DR) | network, UI |
| `cena-session` | integration, unit | property (queue fairness) | UI |
| `cena-script` | **conformance**, integration | sandbox, unit | network, UI |
| `cena-ui` | unit (snapshot shape) | — | a real frontend |
| `cena-tui/gui/web` | integration, **parity** | manual smoke | — |

**Rule 2.1 — If a test for game logic needs a terminal or a socket, the layering is
wrong.** The test is telling you about an architecture violation. Fix the architecture, not
the test.

---

## 3. Organization

**Rule 3.1 — Test layout mirrors source layout.** Both reference projects do this
independently; Lich's `spec/README.md` states it as policy.

**Rule 3.2 — Unit tests inline (`#[cfg(test)]`), everything else in `tests/`.** Standard
Rust; matches Vellum.

**Rule 3.3 — Fixtures are shared, versioned, and real.** One fixture corpus per crate that
needs one. Real captured data wherever possible.

**Rule 3.4 — Test doubles are explicit and documented.** Lich's `mock_database_adapter.rb`
is a good model: it says what it doubles, and notably that it is *"deliberately NOT
auto-required"* so certain specs can test in strict isolation. Know what you are faking and
why.

**Rule 3.5 — A test's name says what breaks if it fails**, not what it calls.
`unknown_tag_survives_to_frontend`, not `test_parser_4`.

---

## 4. CI

Lich runs 16 workflows; the ones that matter structurally are `rspec_tests`, `rubocop`,
`ruby_syntax`, and — notably — `windows_active_sessions`, a **platform-specific** job for
the subsystem where platform differences actually bite.

**Cena's required checks, from the first commit:**

1. `cargo test --workspace` — everything.
2. `cargo clippy -- -D warnings` — lint is not optional.
3. `cargo fmt --check`.
4. **Builds for Android and iOS targets** (engineering rule 9.4). A dependency that breaks
   mobile fails the build, not a future port.
5. Architecture tests (part of 1, called out because they gate merges).
6. Golden/characterization diffs reviewed, never auto-regenerated.

**Rule 4.1 — Platform-specific behavior gets a platform-specific CI job.** Lich learned
this for active-sessions on Windows; Cena's equivalents are mobile and the web frontend.

---

## 5. What we deliberately do not do

- **No coverage percentage target.** Vellum's `chokepoints.rs` makes the argument better
  than I can — it exists precisely because *"coverage percentages say how many scripted
  edges we can walk. They do not say whether a user can still get where they are going."*
  Measure user-facing impact, not line coverage.
- **No mocking the game.** We replay real transcripts. A mock encodes our belief about the
  protocol; a transcript encodes the protocol.
- **No UI snapshot tests of rendered pixels.** Test the snapshot *data* that frontends
  render, not the rendering.
- **No flaky tests tolerated.** A flaky test is deleted or fixed the day it flakes. Nothing
  erodes a suite faster.

---

## 6. Phase alignment

| Phase | Tests that must exist by the end of it |
|---|---|
| 0 Skeleton | architecture tests, CI incl. mobile builds, fixture corpus started |
| 1 Frames | **golden parser suite**, property (no panic), bench baseline |
| 2 Connection | integration (auth, reconnect), **the session recorder** |
| 3 Game model | **replay suite**, GS/DR parity where shared |
| 4 Frontend | UI integration, web/Rust parity |
| 5 Script runtime | **conformance checklist**, sandbox tests, scheduler stress |
| 6 API completion | conformance expands per census tier |
| 7 Multi-session | isolation tests (no cross-session reach), fairness under load |
| 8 DragonRealms | DR golden + replay corpus; adapter parity |
| 9 Desktop frontends | per-frontend integration; parity with web |

**Rule 6.1 — The recorder is Phase 2, not "later."** Every session anyone plays after that
point is a potential fixture. Building it late means throwing away months of free test data.
