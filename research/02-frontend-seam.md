# The Frontend Seam — and the constraint Cena does not inherit

Written 2026-09-17. Resolves open question 1 from `01-architecture.md`, and follows the
observation that **Lich is constrained by frontends it does not own; Cena owns both sides
of the wire.**

---

## 1. The question, answered

`06-vellum-frontends-and-platforms.md` §2 calls Vellum's frontend abstraction "a
near-fiction." Verified against the source — it is right, and the conclusion is
**reassuring rather than alarming**: the trait is bad, but the seam underneath it is good.

### What the trait actually is

`src/frontend/mod.rs:26-73` defines a five-method `Frontend` trait. `grep -rn 'impl
Frontend for' src/` returns **exactly one hit**: `tui/frontend_impl.rs:12`.

The GUI declines it, and says why (`gui/mod.rs:16-19`):

> The GUI is a native `eframe::App` driven by the egui event loop; it deliberately does not
> implement the `Frontend` trait (that trait models a frontend polled/rendered by an
> app-owned loop, which eframe inverts). The shared contract with the TUI is `AppCore` +
> `UiState` + the config layer.

The web frontend is not a frontend at all — it is a **sidecar** that attaches a
`RemoteSink` to `AppCore` and receives flushed deltas. `FrontendType` has three variants
dispatched by a plain `match` into three unrelated `run()` functions. There is no
`Box<dyn Frontend>` in the shipped path.

The trait's failure is legible in its own signature: `render(&mut self, app: &mut dyn
std::any::Any)` forces a downcast at `frontend_impl.rs:76-78`, because the trait could not
name `AppCore`.

### What actually unifies the four UIs

Three things, none of them the trait:

1. **`AppCore` + `UiState` + config** — the stated shared contract.
2. **`FrontendEvent`** (`frontend/events.rs:11-25`) — five variants: `Key`, `Mouse`,
   `Resize`, `Paste`, `Quit`. Crucially, key/mouse types come from `crate::data::input`,
   **not** from crossterm or egui. That is what lets the GUI reuse TUI keybind resolution.
3. **`WindowContent`** (`data/widget.rs`) — the GUI matches on it, the TUI dispatches
   per-type, the web serializes deltas from it.

### Verdict

**The real seam is: a frontend-agnostic state snapshot + a frontend-agnostic input event,
with each UI owning its own loop.** That seam is proven three times over in Vellum (TUI
polls, GUI inverts, web pushes over a socket).

**Do not port the `Frontend` trait.** Port the seam it failed to express. This is exactly
what `01-architecture.md` §1 already specifies (L6 consumes frames, emits intents), so
**the plan does not change** — Phase 4 and Phase 9 costs stand.

One concrete lesson to carry: input types must live in Cena's own vocabulary
(`cena_data::input`), never re-exported from ratatui/egui/web. That single discipline is
what made three loop models share one keybinding system.

---

## 2. The constraint Cena does not inherit

This is the more valuable finding, and it comes from the reframe.

**Lich must serve frontends it did not write and cannot change.** StormFront, Wrayth,
Wizard, Genie, Profanity, Saga, Suks — each speaks its own dialect, and Lich sits in the
middle translating. That obligation is visible everywhere:

| Cost | Evidence |
|---|---|
| Per-frontend adapters | `lib/common/frontend/`: `genie.rb`, `profanity.rb`, `saga.rb`, `suks.rb`, `wizard.rb`, `wrayth.rb` |
| A markup translation layer | `lib/common/markup.rb` (264 lines): `fb_to_sf`, `sf_to_wiz`, `strip_xml`, `monsterbold_*` |
| Frontend branching in core | 33 `$frontend` references in `lib/` |
| Launch machinery | `frontend_launcher.rb`, `frontend_locator.rb`, `frontend_settings.rb`, hosts-file/SAL redirection |
| Capability probing | `Frontend.supports_mono?` gates how `respond` wraps output |

`markup.rb`'s header states the job plainly: *"rewrites a StormFront/XML line into the GSL
escape vocabulary an old Wizard frontend understands."* Lich is a **protocol translator**
as much as a scripting engine — and it carries stateful cross-call buffers
(`$sftowiz_multiline`, `$strip_xml_multiline`) because a `pushStream` element can split
across two socket reads, with `games.rb:435` reaching in to clear them on reconnect.

**Vellum already demonstrates the alternative.** Owning both sides, it pays none of this:
`grep -rn "FrontendType" src/core/` returns **0**. "Monsterbold" survives only as a
*styling* concept (`core/harmony.rs:272`), never as a markup dialect to emit.

### What Cena gains

- **No dialect emission.** Frames go to the UI as typed values. No GSL escapes, no
  XML-to-GSL rewriting, no `supports_mono?` capability probing.
- **No split-tag buffers.** The protocol layer owns reassembly once (§0.1 of the plan);
  no UI-facing translator needs its own cross-call state.
- **No frontend launching.** Cena *is* the frontend. `frontend_launcher`, `frontend_locator`,
  hosts-file redirection and SAL handling all disappear.
- **The wire format is ours.** Core→UI can be a typed snapshot + delta, versioned on our
  schedule.

**Rough saving: 400–600 lines of translation and adapter code, plus the launch subsystem —
and, more importantly, an entire category of stateful bug.**

### The one place this is *not* free

Cena still must parse the game's XML/GSL **inbound** — that is Simutronics' protocol, not
ours, and it is the unmovable external contract alongside EAccess auth. The freedom is
strictly on the **outbound/UI side**. Do not let "we own the protocol" leak into thinking
the parser can be simplified; §0.1 and the `Unknown` frame variant still stand, and
`markup.rb`'s split-element problem is a warning about the *inbound* reassembly Cena must
still get right.

---

## 3. The trap: `$frontend` in the script corpus

`07-script-corpus-api-census.md` finds `$frontend` read **92 times** across the corpus to
branch on client type — `'stormfront'` 41, `'profanity'` 20, `'genie'` 2, `'suks'` 1,
`'saga'` 1 — and recommends: *"Cena must expose a `$frontend`..."*

**Reject that recommendation.** It is right for a Lich-compatible engine and wrong for
Cena. Exposing `$frontend` would import the exact constraint Cena is free of, and worse, it
would be a *lie* — scripts would branch on a value that no longer varies, then silently
mis-handle whichever branch we picked.

Scripts branch on `$frontend` for one real reason: **"what markup may I emit?"** That is a
capability question wearing an identity costume.

**Cena's answer: scripts do not emit markup at all.** They emit *intent* —
`echo{text=..., style="creature"}` — and the UI layer decides rendering. If a script truly
needs to know whether something is available, it asks a **capability**
(`ui.supports("inline_image")`), never an identity. Capabilities are honest under a single
frontend; identities are not.

This is the same discipline as §4 of the plan (game capabilities must be honest, never a
fabricated default), applied to the UI boundary.

---

## 4. What this adds to the plan

1. **Confirmed:** do not port the `Frontend` trait; port snapshot + input-event with
   per-UI loops. Phases 4 and 9 unchanged.
2. **Discipline:** input types live in Cena's own vocabulary, never re-exported from a UI
   crate. This is what made three loop models share one keybind system.
3. **Deleted scope:** frontend adapters, markup translation, frontend launching/locating,
   capability probing, hosts-file/SAL redirection — none of it is Cena's problem.
4. **New rule:** the Lua API exposes **UI capabilities, never a frontend identity**. No
   `$frontend`.
5. **Unchanged:** inbound protocol parsing is still an external contract. The freedom is
   outbound only.
