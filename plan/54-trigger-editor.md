# 54 — The trigger editor

**Status: APPROVED 2026-09-29, the author's answers in §1. Branch `trigger-editor`. Steps 1-3 BUILT 2026-09-29.**

M8 built triggers whole (`plan/45`) and left one door: the file, `;trigger`, and a Wrayth
import. The author, choosing one file (`plan/45` §1 row 3): *"players won't be accessing the
file most of the time, we will have a gui editor which can give a category"*. This is that
editor: a window over the one triggers file, in the GUI (`plan/47`, `plan/49`).

---

## 1. Decisions — AUTHOR, 2026-09-29

| # | Question | Answer |
|---|---|---|
| 1 | An import button in the editor? | *"don't need an import wrayth button. we can have a hydra command to import but no need to make it a permanent thing in the editor."* `;trigger import` stays the way in. |
| 2 | Does a send need approval? | As `plan/45` §1 row 1: only a trigger from elsewhere. And: *"if you import it, and it has a send command it should do like a popup indicating it contains the commands, list the commands, offer accept all, accept one, cancel. editing it counts as approving sure."* |
| 3 | Where does it open? | *"sure we can put it next to settings for now."* A *Triggers* button beside *Settings*. |
| 4 | Per-character overrides in the form | **(a)**: a per-character on/off. Changing any field for one character (`for.<name>`, `plan/45` §5b) stays in the file and `;trigger`, and is shown, not edited. |
| 5 | Sounds | *"we can have a default folder, and accept a path"*: already so (`plan/45` Stage 3), a file in the sounds folder or a path. The form offers the folder's files and takes a path. |
| 6 | Presets and send rules | *"sure stay as plan/45"*. |
| 7 | The form's shape | *"yeah I like those form answers"*: *Match case* off and *Whole words* on by default (`plan/45` §5d); a condition greys out the line's responses (look, squelch, substitute, redirect: §6b, a condition has no line); the event list is §6b's. |

**CLAUDE'S READING, TO CONFIRM (row 2):** the import popup's three answers.

- **Accept all**: the import goes ahead, every send approved.
- **Accept one**: the commands are listed with a box each. The import goes ahead, the ticked
  ones approved; the rest are imported with their sends held, as `;trigger approve` can
  release later. The triggers' other responses (colour, sound) work either way.
- **Cancel**: nothing is imported.

Without a window (`--headless`, `--web`) the import goes on as it does now: sends held, each
named, `;trigger approve` to release.

---

## 2. What exists — measured 2026-09-29

| Piece | Where | What it gives the editor |
|---|---|---|
| The rule | `crates/cena-model/src/trigger.rs:136` | every field the form edits, checked by `check::Raw` |
| The file | `crates/cena-behavior/src/triggers.rs` | categories, `characters`, `for.<name>`, `origin`, `held`, `approved`, the master switches, refused by name |
| The writer | `crates/cena-behavior/src/triggers/edit.rs` | `add`, `set`, `unset`, `approve`, `remove`, `switch`, `list`, `show`, `import`: each reads the change back as Hydra will and refuses it with the reason. **The editor writes through these**, so the form and `;trigger` cannot disagree (`plan/50` §7's rule: one writer) |
| Every character told | `crates/cena/src/triggers.rs` (`Changes`, `plan/45` Stage 6) | a change reaches every running character |
| The matcher | `cena_session::trigger::Matcher` (`crates/cena-session/src/lib.rs:90`) | the GUI can run the real matcher on a typed line: the live test needs no round trip |
| `;trigger test` | `crates/cena/src/triggers/explain.rs` | what fires and why, in words |
| The seam | `HubRequest::Settings` / `Change` (`crates/cena-ui/src/hub.rs:51`), `Sessions::settings` (`crates/cena-gui/src/sessions.rs`) | the pattern: the window asks, the binary answers by handing it a view, a change goes back as a request |

The GUI cannot see `cena-behavior` (`crates/cena-arch-tests/tests/layering.rs`): the file is
read and written by the binary, and crosses to the window as view types in `cena-ui`, as the
settings menu's pages do.

---

## 3. The window

```
┌ Triggers ─────────────────────────────────────────────────────────────┐
│ [Search…]  [+ New]        Squelch [on] Sound [on] Send [on] …         │
├──────────────────────────┬────────────────────────────────────────────┤
│ ▾ Combat (12)   [on]     │ Name  [stunned        ]  Category [Combat▾]│
│   ☑ stunned              │ ── When ─────────────────────────────────  │
│   ☑ webbed      ⚑ send   │  (•) Text  ( ) Event  ( ) Condition        │
│ ▸ Ignores (40)  [off]    │  [You are stunned ] ☐regex ☐case ☑whole    │
│ ⚠ Refused (1)            │  Stream [any ▾]   Only if [+ guard word]    │
│   bad-regex: unclosed (  │ ── Do ───────────────────────────────────  │
│                          │  ☑ Look ■#ff4040 □bg ☑bold [line ▾]         │
│                          │  ☐ Squelch ☐ Substitute ☐ Redirect ☐ Flag   │
│                          │  ☑ Sound [alarm.wav ▾] ▶  ☐ Notify ☐ Alert  │
│                          │  ☐ Send [          ]   Cooldown [3] Prio [0]│
│                          │ ── For whom ─────────────────────────────  │
│                          │  (•) Everyone ( ) Only [Nisugi, Dicate]     │
│                          │  Off for: ☐ Nisugi ☑ Dicate                 │
├──────────────────────────┴────────────────────────────────────────────┤
│ Test [You are stunned!                    ] stream [main ▾]           │
│  → You are stunned!  (as it would show)   fires: stunned              │
└───────────────────────────────────────────────────────────────────────┘
```

---

## 4. Steps

1. **The seam and the list.** `cena_ui::triggers`: the view the binary builds from the file
   (each trigger's name, category, on/off, a line of what it does, its form, a held send, its
   origin, who it is off for; the refused, with reasons; the switches), and the changes the
   window asks. `HubRequest::Triggers` and `HubRequest::Trigger(change)`, answered through
   `Sessions` as pages are. The window: *Triggers* beside *Settings* on the play window and
   the hub; the list by category with each category's switch and count, search, a trigger's
   own switch, the kinds' switches, the refused with their reasons, a held send with
   *Approve*.

   > **BUILT 2026-09-29.** `crates/cena-ui/src/triggers.rs` (`Book`, `Entry`, `Change`);
   > `HubRequest::Triggers` and `HubRequest::Trigger`; the binary's
   > `crates/cena/src/triggers/book.rs` reads the file into the book and makes a change
   > through `edit.rs`, then tells every running character, as "the Triggers window",
   > so each reads the file again (`Changes`, `plan/45` Stage 6). The writer's list gained
   > each trigger's line of what it does, its held send and its origin, and
   > `edit::switches` reads the master switches. The window: `crates/cena-gui/src/triggers.rs`
   > and `app/triggers.rs`, placed as every window is (`placement.rs`); *Triggers* beside
   > *Settings* on the hub and the play window. Removing asks again first. Ten GUI images
   > changed, each only by the new button.
2. **The form.** When (text or regex, match case, whole words, stream; an event, with its
   names; a condition's guard words, with the vocabulary offered; only-if), Do (every response
   of `Rule`, the line's greyed for a condition), For whom (everyone or named characters, and
   off for each). New, duplicate, rename, delete (asked first). Saved through `edit.rs`; a
   send typed or changed here is approved (§1 row 2).
   > **BUILT 2026-09-29.** The form is `cena_ui::triggers::Form`, every field of a trigger's
   > table the form edits (`edit::FORM_KEYS`); the rest (`for.<name>`, `origin`, `held`,
   > `approved`) the writer keeps. The binary turns a table into a form and back
   > (`crates/cena/src/triggers/form.rs`, a round trip pinned by a test) and saves through
   > `edit::save`, which renames, keeps what the form does not edit, and approves the send
   > only when the form changed it; one left alone keeps its hold. *Off for* is a change of
   > its own, `for.<name>.enabled`. The window (`crates/cena-gui/src/triggers/form.rs`):
   > name and category; *A line* (words, regex, match case, whole words, an event from
   > `LineEvent::every`, a stream) or *A condition* (guard words from
   > `guard::vocabulary`, less the five a trigger cannot use, and its rearm); *only if*;
   > each response behind a box, the line's greyed for a condition; cooldown and priority;
   > everyone or the characters ticked; off for each roster character. *+ New*,
   > *Duplicate*, *Save*, *Revert*. A draft is saved only when the file reads it back as
   > it is, so a refused save stays unsaved, and a click elsewhere does not throw it away.
   > Not built: ▶ to hear a sound (the sound plays in the binary, `crate::attention`).
3. **The live test.** A line and its stream run through the real matcher in the window: the
   line as it would show (painted, squelched, substituted, moved) and what fires; the form's
   unsaved trigger with the saved ones.
   > **BUILT 2026-09-29.** Along the bottom of the editor (`crates/cena-gui/src/triggers/test.rs`):
   > a line and its stream, and below, the line as it would be shown (painted, or *not shown:
   > squelched*, or on another stream), what fires, and what would be sent (a test sends
   > nothing). The triggers are the file's that are on, not refused and not in a category
   > switched off, with the form in place of its saved self, so a change is seen before it is
   > saved; a form the file would refuse says why. The form becomes a `Rule` in the window
   > (`Form::rule`): the table conversion moved down from the binary to
   > `crates/cena-ui/src/triggers/table.rs`, so the window and the binary share one. As
   > `;trigger test`, an *only if* is taken to hold and a condition has no line to test; the
   > kinds' master switches are not applied.
4. **From the story, and against the log.** A story line's right-click: *Make a trigger from
   this line*. *What would this have caught?*: the form's trigger over today's player log
   (`plan/25`'s reader), the lines it matches.
5. **The import popup** (§1 row 2): `;trigger import` with a window open asks first.

Each step leaves the tree green and is committed alone.
