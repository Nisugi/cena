# Cena crate review — 2026-09-28

## Assessment and scope

The new settings and widget work makes Hydra substantially easier to configure. The remaining concern is whether the controls and automation consistently act on the state and character they present. This review identifies **14 current findings: six P1 and eight P2**, including **three newly identified defects** in the GUI/UI integration. The eleven findings from the September 27 review remain open in the inspected implementation, with changed locations and expanded exposure noted below.

The active checkout, `G:/dev/Cena`, is at `a4b9ba4`; its crate sources have not changed since the earlier review at `e028508`. The IDE tab points to the newer `G:/dev/Cena-gui` worktree. After presenting that scope choice and receiving no answer during the review, I used **the full 12-crate Cena-gui workspace** as the updated review target. Its inspected HEAD is `560d18cbcee3aeca05c1cd46801804a428a7ff8d` on `gui-widgets`, plus the working-tree edits present during inspection.

The other active worktrees, `Cena-m7` and `Cena-m7b`, are **not included**. In particular, this is not a review of the new agent or Ruby bridge implementations on those branches.

Uncommitted drag-and-drop work was changing during the review. Initially modified files were `carry.rs` and `widget/{draw,lines,lists,tests}.rs` in `cena-gui`; `play/tests/drag.rs` also changed during the run. This limits any claim that the checks certify one frozen revision. No production source or existing test was edited by this review. The report is saved in the active Cena checkout; build logs and temporary probes are in Cena-gui's `target/`.

All crate source paths below are relative to **`G:/dev/Cena-gui/crates`**, unless explicitly stated otherwise. This is a risk-directed review of every crate, with closer inspection of changed code and cross-crate contracts, not a claim to have audited every gameplay rule or every line.

**Evidence labels:** VERIFIED—probe means an offline experiment exercised the relevant implementation; VERIFIED—source means the stated behavior follows from inspected code, without reproducing the full runtime scenario. An inferred consequence is identified as such. P1 means fix before relying on affected automation/concurrent use; P2 means a narrower correctness or usability defect. These are engineering priorities, not security ratings.

## Prioritized findings

### R1 — P1: Hunt and travel continue after losing state updates

**VERIFIED—source; carried forward.** `cena-behavior/src/hunt/drive.rs:374` and `:393`, and `cena-behavior/src/travel/drive.rs:592` and `:671`, ignore broadcast lag while maintaining a locally folded state.

A lost room, inventory, or lifecycle event need not be repeated by subsequent output. Continuing can produce a state-dependent command based on obsolete information. The session gate does not validate every movement or inventory decision.

**Fix:** stop or recover an authoritative snapshot and fenced subscription before making another decision. **Test:** force lag across a material state change; assert no dependent command is sent before recovery, covering both drain and awaited receive paths.

### R2 — P1: Queued trigger actions lose their originating connection generation

**VERIFIED—source; carried forward.** `cena/src/triggers/follow.rs:67` consumes plain events; `:79` dispatches an action without its generation. `cena/src/triggers/act.rs:15` uses the current command table or `send_now`; `cena-session/src/command/handle.rs:494` stamps the latter with the current generation.

An action queued before reconnect can consequently be treated as belonging to the replacement connection. Local semicolon commands also need protection, because they can start a behavior before any game-send gate is consulted.

**Fix:** preserve observation metadata through both local and game dispatch. **Test:** queue an action, reconnect, then consume it; neither a command nor a behavior should start on the new connection.

### R3 — P1: Trigger sends bypass synchronization readiness

**VERIFIED—source today; session-boundary probe reproduced September 27.** Trigger dispatch uses `Origin::Trigger` and `Gate::None` at `cena/src/triggers/act.rs:22`. The Ready check in `cena-session/src/actor/io.rs:393` only covers behavior origins.

The earlier offline probe sent `stand` successfully while the session was `Syncing`; these relevant paths are unchanged. Line-trigger publication in `cena-session/src/actor/line.rs` also has no Ready check. Manual input policy should not implicitly determine trigger automation policy.

**Fix:** define and enforce readiness for trigger actions, without replaying obsolete setup actions later. **Test:** the same sending trigger during setup and after Ready; only the latter should send under that policy.

### R4 — P1: Failed trigger reloads falsely report that none are active

**VERIFIED—source; carried forward.** `cena/src/triggers/load.rs:31` returns on loading failure before replacing the matcher at `:55`, but the error at `:70` says “None are on.” The follower repeats that claim at `cena/src/triggers/follow.rs:110`.

After a valid active configuration, a malformed reload leaves the old matcher active, including sending triggers, while claiming it is disabled.

**Fix:** either retain the old matcher and accurately say it remains active, or explicitly disable it. **Test:** install a sending trigger, damage its file, reload, and verify the actual active state agrees with the notice.

### R5 — P1: Settings and roster updates are not serialized transactions

**VERIFIED—source; carried forward and expanded.** Trigger changes still read/edit/write without transaction locking (`cena/src/triggers.rs:146`). Hunt edits still save before final load validation and may restore an old copy (`cena/src/hunt/settings.rs:113`). The shared TOML writer now delegates to `cena-session/src/store.rs:136`, but it still uses one fixed temporary path per destination. Atomic replacement does not prevent two writers from reading the same starting value and losing one update.

The new UI adds concurrent entry points: `cena/src/general.rs:216` reads and later saves a whole settings file; roster record/forget/favourite at `cena/src/roster.rs:139`, `:156`, and `:173` have the same transaction gap. A roster update on successful login can overlap a launcher edit. A hunt rollback can overwrite someone else's intervening successful change.

**Fix:** serialize the complete read/validate/change/commit transaction per file, including all entry points. Validate before committing where possible. Unique temporary names alone are insufficient. The travel store's locked update is an existing useful pattern. **Test:** barrier-controlled concurrent edits preserve both changes; a failing edit cannot roll back a successful writer.

### R6 — P1: Bare character names still substitute for session identity

**VERIFIED—source; carried forward and expanded.** Relay destinations lose instance identity in `cena/src/play.rs:514`; `cena/src/relay.rs:85` returns the first exact match. Party seats are inserted by bare name at `cena/src/hunt.rs:199` and removed by name at `:181`.

Same-named characters on different accounts/instances can therefore be misrouted or overwrite one another's party seat. The new launcher repeats the assumption: `cena-gui/src/launch.rs:106` considers any matching bare name already on the table, and `:115` hides its roster entry. Advanced widget following also looks up a bare name in `cena-gui/src/play/draw.rs:246`.

**Fix:** use `SessionId` for live control and qualified game/character identity for persistent selection. Reject ambiguous exact names. **Test:** two same-named characters on distinct accounts/instances through relay, party cleanup, launcher availability, and widget following. Layout filenames have already improved to include instance; that does not fix these other paths.

### R7 — P2: Concurrent quit requests disagree about server acknowledgment

**VERIFIED—source today; reproduced September 27.** `cena-session/src/actor/io.rs:277` immediately returns `Farewell::Acknowledged` for a second quit while the first is pending. In the earlier no-EOF probe, the second returned acknowledgment and the first timed out. This code is unchanged.

**Fix:** share the eventual logout outcome among waiters or return a distinct pending result. **Test:** two requests before EOF, both eventual closure and timeout; acknowledgment must mean actual server closure.

### R8 — P2: Missing quiet-off can leave game text hidden

**VERIFIED—probe today; carried forward.** `cena-gui/src/story.rs:147` adds a gap marker without recovering quiet state. In the current implementation, quiet-on, a missed-event gap, and an ordinary main-stream line still leave the ordinary line suppressed. The web equivalent remains at `cena-web/src/presentation/pending.rs:100`.

The session snapshot does not carry authoritative quiet state, so simply resubscribing does not repair this presentation state. Newly attaching during quiet mode has the opposite initialization problem.

**Fix:** recover suppression state with the snapshot/fence contract. **Test:** lose quiet-off, recover, and verify subsequent game text appears; also attach while quiet is active.

### R9 — P2: Malformed numeric timers clear known active timers

**VERIFIED—source today; reproduced September 27.** `cena-protocol/src/parser/thin.rs:60` defaults malformed numeric values to zero. The earlier probe changed active roundtime to inactive with `<roundTime value='broken'/>`; the parser/model path is unchanged.

This is malformed-input resilience, not evidence that the live server routinely emits such data. **Fix:** distinguish invalid/missing data from legitimate zero, preserving prior trustworthy state or marking it unknown. **Test:** invalid, absent, negative, and overflowing values after an active timer; valid zero remains a separate case.

### R10 — P2: Native keybindings bypass single-command validation

**VERIFIED—source; carried forward.** `cena-gui/src/keys.rs:281` validates chords but accepts arbitrary command strings. The new editor writer at `cena-gui/src/keys/write.rs:39` likewise validates the chord and resulting binding, not single-line command content. Submission through `cena-gui/src/sessions.rs:354` does not apply `cena-ui`'s command validator.

A binding containing an escaped newline can reach the transport as multiple game commands under one submission. **Fix:** enforce shared command constraints at submission, with early binding diagnostics; intentional batches need explicit dispatch. **Test:** LF, CR, NUL, and excessive length through both file loading and the editor.

### R11 — P2: Account reservations end before session shutdown does

**VERIFIED—source; carried forward.** `cena-host/src/table.rs:180` removes the session immediately; `cena/src/play.rs:442` releases the host lock before awaiting shutdown. During that wait, another request can start a new character on the same account because the old reservation is gone.

This breaks the host's stated one-character-per-account invariant during removal. **Fix:** retain a stopping reservation through shutdown and dependent cleanup, without blocking unrelated accounts. **Test:** delay EOF, request remove and same-account add concurrently, and ensure only an unrelated account may proceed immediately.

### R12 — P2, NEW: Effective hunt settings are displayed as “none”

**VERIFIED—probe and source.** `cena/src/hunt_pages.rs:89` represents a character-file override as `RowKind::Map` with `Value::Text`, making it read-only. Complex values such as arrays of target tables take the same representation at `:145`. But `cena-gui/src/menu.rs:427` renders only a nonempty `Value::Map`; every other value becomes “none.”

The offline GUI harness supplied the actual producer's representation for an inherited resting room of `29877`. The menu displayed “none” and no `29877`. This undermines the effective-settings view precisely when the user is trying to understand why a profile behaves differently for one character.

**Fix:** separate read-only presentation from value type, or render text/list values correctly in the read-only branch. Display the real value and origin even when editing is unavailable. **Test:** render real hunt-page output end to end for a character override, structured targets, an empty map, and a nonempty map. Testing the page producer and renderer separately missed this mismatch.

### R13 — P2, NEW: Object menus remain actionable across reconnects

**VERIFIED—probe for retained state; source for dispatch consequence.** At `cena-gui/src/story.rs:88`, a new generation resets quiet but not `menu`. The probe installed a menu response, delivered an event from the next generation, and confirmed the old menu was retained.

The pending request in `cena-gui/src/play/links.rs:21` contains its request number, object ID, noun, and position, but no generation. The renderer at `:62` only matches the numeric request ID. Selecting an entry at `:95` produces a string; `cena-gui/src/sessions.rs:354` then obtains the latest snapshot generation for submission. Thus the send gate cannot identify that the menu and object ID came from an earlier connection.

**Fix:** clear pending/displayed menus on generation changes and retain the originating generation through action dispatch. Also prevent an old response from being reused after a play window is reopened and its request counter restarts. **Test:** open a menu, reconnect, and attempt selection; no command should send. Exercise reopened windows and delayed responses too.

### R14 — P2, NEW: Direct object links ignore server-learned command overrides

**VERIFIED—probe.** `cena-ui/src/object_menu.rs:43` resolves popup entries with the state's learned dictionary. In contrast, `link_command` at `:81` calls the static dictionary's `command_for`, without any state argument. The GUI sends that direct result at `cena-gui/src/play/links.rs:43`.

An offline probe taught the model that coordinate `2524,1543` means `inspect #`. The popup correctly resolved `inspect #123`, while a direct link to that same coordinate still resolved `attack #123`. The override is synthetic, but the discrepancy is real: server updates are explicitly authoritative in the model's command-list contract.

**Fix:** resolve direct coordinates through the same learned-first dictionary policy as popup entries. Preserve dialog and secondary-argument restrictions. **Test:** a changed built-in coordinate and a newly learned coordinate through both paths must agree with the server's current command.

## Verification and observed test instability

Results are being finalized; pending entries below are not passes.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed on the inspected working tree. |
| Browser/atlas Node tests | 71 passed, 0 failed. |
| `cargo test --workspace` | **Failed**, exit 101, in the GUI library tests: 207 passed, 1 failed in that test binary. Cargo stopped before completing the workspace. |
| Exact container-drag rerun | Passed: 1 passed, 207 filtered. This does not erase the original failure. |
| Remaining workspace tests | In progress at drafting. |
| GUI package rerun, Clippy, rustdoc | Pending at drafting. |

The failing test was `play::tests::drag::from_a_container_it_goes_elsewhere`, at `cena-gui/src/play/tests/drag.rs:311`: expected `[Quietly("_drag #11 #200")]`, observed `[]`. It was an existing uncommitted test being added by the ongoing development work, not a test added by this review. Its isolated rerun passed without a review fix. This establishes an unstable test result; it does **not** establish the cause or prove that container dragging always fails. Investigate scheduling/frame assumptions and shared test state before treating the full GUI suite as reliably green.

Today's standalone probes produced:

```text
Server override: menu=Some("inspect #123"), direct link=Some("attack #123")
Inherited setting: shows none=true, shows actual 29877=false
Old object menu retained after generation change=true
Ordinary line still hidden after lost quiet-off=true
```

Probe source and output: `G:/dev/Cena-gui/target/review28/`. The GUI settings probe calls the production menu through `egui_kittest`; the story probe compiles an unchanged copy of the production story module and its children in scratch space. These are diagnostic probes, not committed regression tests. The old roundtime, quit, and Syncing probes are identified as September 27 evidence, not counted as reruns today.

## Coverage by crate

| Crate | Review focus and assessment |
| --- | --- |
| `cena-platform` | Revisited new character-list authentication flow, response parsing, and game/instance mapping; existing transport/TLS review remains applicable. No additional actionable defect established. No real credentials or live login used. |
| `cena-protocol` | Crate source unchanged from the prior review; numeric timer defect R9 remains. Existing incremental parsing/bounds review carries forward. |
| `cena-model` | Reviewed new menu resolution, learned-command behavior, world-state addition, and reconnect changes. Model honors learned overrides; R14 is its caller bypassing that contract. Timer consequence remains R9. |
| `cena-map` | Reviewed setting-name discovery through serialized exits and its use by the settings menu. No new correctness defect established; prior loader/routing review remains applicable. |
| `cena-session` | Rechecked actor gates, observation and generation APIs, shutdown, and the consolidated file writer. R2, R3, R5, R7, R8, R10, R11 span these contracts. |
| `cena-behavior` | Reviewed settings metadata/defaults, profile chain/origins, persistence changes, and travel setting constants; rechecked hunt/travel lag handling. R1 and R5 remain. |
| `cena-ui` | Reviewed settings/launcher DTOs, object-menu projection and direct links. R14 is new; command validation remains available but inconsistently used by native callers. |
| `cena-web` | Production crate unchanged; presentation recovery R8 remains. Browser/atlas tests rerun. No new live browser walkthrough performed. |
| `cena-host` | Crate source unchanged; account reservation/shutdown contract R11 remains. |
| `cena-gui` | Settings, key editor, launcher, widgets/following, layout persistence, menu/link actions, drag payloads, and story lifecycle. R6, R8, R10, R12, R13, R14; container-drag test instability recorded separately. |
| `cena` | Settings producers and writers, command wiring, launcher/roster, trigger dispatch, relay, session lifecycle. Several root causes are lost contracts at this composition layer. |
| `cena-arch-tests` | Reviewed updated cap baselines and prior structural checks; these do not substitute for cross-crate runtime assertions. No separate actionable architecture defect established. |

“No new defect established” means no demonstrated issue in the inspected paths, not a guarantee of correctness. Neither the entire game vocabulary nor every combat/healing strategy was exhaustively revalidated.

## QoL and maintainability assessment

**Progress worth keeping:** one settings entry point; pages for characters even when offline; visible effective values and origin metadata; behavior-owned parsers reused by the GUI; explicit notices for changes that apply on the next login/trip; editable keybindings; per-instance layout filenames; and a shared atomic writer. These address the original difficulty of finding configuration controls.

**Remaining access gaps:** maps/structured settings are largely read-only, character-level hunt overrides direct the user back to their file, and a fresh installation with no hunt profiles has no hunt page to edit (`hunt_pages::pages` enumerates existing files). Treat these as onboarding work, rather than claiming a completed GUI editor for every setting. Show the exact command/path needed, or provide the missing operation. The false “none” display in R12 must be fixed regardless of whether editing those values is in scope.

**Tests need to cross producer/consumer boundaries.** R12 survives tests of the page data and of generic map rows; R14 survives tests of the model dictionary and static links in isolation. Add tests that create the actual producer output and drive the consumer, including stale generations, lag and concurrent writes.

**Additional validation, not counted as findings:** drag payloads currently carry object ID/name without source session/generation. Verify cross-window and reconnect behavior before extending item movement; another-character widget checks alone do not establish that a shared drag payload is correctly scoped. Native focus, accessibility, DPI, sound, and 3–25-session responsiveness still need hands-on or measured testing. No performance regression is asserted without measurement.

## Recommended order

1. Fix R1–R4: stop trusting incomplete observations, keep trigger actions attached to their connection, and make automation status truthful.
2. Fix R5–R6 and R11: serialize shared data updates and preserve complete session/account identity through lifecycle changes.
3. Fix R12 and R14 promptly: both are small integration defects with directly reproduced user-visible consequences. Then fix R13's menu lifetime and R8's quiet recovery.
4. Finish R7, R9 and R10, stabilize the drag test, and perform a fresh-character walkthrough: configure heal, create/import hunt settings, run/check/stop a behavior, reconnect, and verify which settings are actually in effect.

No live game connection, credential operation, merge, or production fix was performed. A successful offline suite would not replace the author's eventual interactive/live validation.
