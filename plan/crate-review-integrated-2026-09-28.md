# Cena integrated crate review — 2026-09-28

## Assessment

The integrated workspace is substantially better than the earlier review suggests. Most earlier defects have fixes, settings have a real user interface, and command help now supplies useful configuration examples. The highest-priority remaining problems are at the new agent boundary: stopping queued work, preserving connection identity, selecting the correct character, and enforcing the documented drop restriction.

This review identifies **six current findings: four P1 and two P2**. Four have offline reproductions of the relevant failure; two are source-traced. P1 means fix before relying on this automation with a real character; P2 means a correctness problem under the stated conditions. These are review priorities, not claims that every feature is unusable.

**This report supersedes the current-status conclusions in `crate-review-2026-09-28.md`.** That report describes the separate GUI worktree before integration. Do not treat its fourteen findings as fourteen still-open defects in this workspace.

## Scope and limitations

Reviewed all **13 workspace crates**, with deeper inspection of changed code, state recovery, command routing, persistence, and the new agent/Ruby integration. This is a risk-directed review, not a claim of exhaustive line-by-line verification or a security certification.

The repository changed during review: the GUI and agent worktrees were merged and removed. Integrated inspection began at `1468b1a`; the final inspection also includes the GUI changes through `75adedb` (settings first-open, grid-on-drag, and link selection). The full test run began before those last GUI merges. Check results therefore describe a moving working tree, not certification of one frozen commit. The six finding paths below did not change in that final GUI-only delta.

No production source or existing tests were edited. Temporary probes and logs live under `target/`. No live game login, live command, or interactive desktop session was used. Ruby bridge state recovery was inspected and its Rust event-log failure reproduced; the complete lag scenario was not driven through a Ruby subprocess. Cross-platform desktop behavior was not exercised.

## Current findings

### I1 — P1: Stop and permission revocation do not cancel an unsent agent command

**Reproduced with a fake transport and the production session API.** See `crates/cena-session/src/operation.rs:457`, `crates/cena-session/src/agent/takeover.rs:240`, and `:273`.

The operation starts an asynchronous send, but its steering function always says the command is already the game's. A command admitted at the Commands level has no takeover token to cancel. Consequently, even before its task is polled, `stop_agent()` cannot stop it and lowering the level to Off does not invalidate it.

The probe admitted `look`, confirmed it was absent from the fake transport, called Stop and set Off without yielding, then allowed the runtime to proceed. The transport received `look` afterward:

```text
Unsent agent command: stop=(0, false), level=Off, look sent after stop=true
```

**User impact:** Stop can appear to have no active work to stop while an agent action is still about to be sent. Off does not establish the boundary a user reasonably expects for unsent agent commands.

**Recommended fix:** give unsent operations cancellation state and enforce revocation at the actual write boundary. Distinguish admitted, queued, sent, and answered. Report honestly when a command has already been written and cannot be undone.

**Regression tests:** Stop before the operation task is polled; Stop while another command holds the queue; lower Commands to Off in both cases; separately verify the already-written outcome.

### I2 — P1: An agent action's expected connection generation is discarded before asynchronous sending

**Source-traced; reconnect interleaving not reproduced.** `crates/cena-session/src/agent/door.rs:191` checks the caller's expected generation, but the immediate-admission path at `:245` passes only the action into `handle.perform`. `crates/cena-session/src/operation.rs:457` later calls the ordinary asynchronous `send_and_await`. `crates/cena-session/src/command/round_trip.rs:137` stamps the generation current when that future runs.

If the connection advances between admission and execution of the send future, an action decided for the old connection can acquire the replacement connection's generation. The actor's generation check then cannot identify its original stale context. A generation check at admission does not cover this asynchronous gap.

**User impact:** a command based on pre-reconnect observations can be delivered on the replacement connection. This is the same class of problem that the earlier trigger fix addressed, through a different entry point.

**Recommended fix:** carry the admitted generation through the operation and envelope, checking it again when writing. Cover approved requests as well as immediate admission. Keep this separate from cancellation: both connection identity and current authority must remain valid.

**Regression test:** admit a command on generation N, delay its task, reconnect to N+1, then release it; assert refusal and no game write.

### I3 — P1: The agent selects ambiguous character names by first match

**Reproduced against the production character registry; binary wiring inspected.** `crates/cena/src/play.rs:362` seats the agent with the bare character name. `crates/cena-agent/src/characters.rs:97` implements `named` with `.find(...)`, without detecting duplicate names. The agent's character-oriented tools use this lookup.

Two sessions called `Shared` can both be registered, but asking for `Shared` silently selects one. The second session cannot be distinguished by a game-qualified name because that qualification was not stored as its name. Equal connection generations across the two sessions do not disambiguate them.

```text
Two agent seats named Shared: count=2, ambiguous lookup picked=Some(Off)
```

The level printed is simply evidence of which seat was selected; the defect is ambiguous selection, not the level itself.

**User impact:** the agent can inspect or act on a different session than intended, or fail to reach the intended session. The GUI/relay/party identity fixes do not cover this newly integrated registry.

**Recommended fix:** expose a stable session identifier in discovery and accept it in tools. Preserve game-qualified display identities. Reject ambiguous bare-name requests and return the choices.

**Regression test:** two same-named characters on different instances, with equal generations; verify independent observation/control and explicit ambiguity errors.

### I4 — P1: The agent drop restriction permits the application's own wire-level drop command

**Reproduced against the production denylist.** `crates/cena-session/src/agent/denylist.rs:75` refuses `drop #123` but permits `_drag #123 drop`. The latter is a supported ground-drop command in Cena itself; see `crates/cena-gui/src/play/tests/drag.rs:100` and `:257`. The agent door uses this denylist when admitting game commands.

```text
Denylist: drop refused=true, _drag drop refused=false
```

**User impact:** an agent allowed to send commands can perform an operation that its documented always-on restriction says is forbidden. This is a concrete equivalent command already used in the repository, not a claim that the denylist should sandbox every possible game action.

**Recommended fix:** recognize this drop form alongside the other documented aliases. Decide explicitly whether hand/container `_drag` forms remain permitted.

**Regression tests:** normal drop, `_drag` to ground, casing/spacing variations, and any intentionally permitted container move.

### I5 — P2: A lagging Ruby runner can permanently miss fields in its local state

**Event-log loss reproduced; downstream recovery failure source-traced.** `crates/cena-agent/src/scripts/listening.rs:164` evicts old events after 4,096 retained entries, including State events. State events contain changed fields, rather than an authoritative full snapshot each time. `crates/cena-agent/src/scripts/watch.rs:113` compares with the producer's previous state and advances that previous state whether or not the runner receives the delta.

`bridges/ruby/hydra/listener.rb:30` processes the retained events, advances its position, and only prints a warning when the response says `lagged`. `bridges/ruby/hydra/copy.rb:26` merges received fields; neither path retrieves a full replacement state after this loss.

The probe pushed a State event and enough ordinary lines to evict it. The next listen reported lag, returned 500 lines, and supplied no State event:

```text
Script overflow: lagged=true, state entries=0, returned=500
```

If the missed field subsequently remains unchanged, the producer never sends it again. This also affects a runner that loses its initial full-state event during a sufficiently large startup burst.

**User impact:** scripts can continue with obsolete room, hands, or other local state even after catching up with output. A warning alone does not restore correctness.

**Recommended fix:** provide a fenced full-state recovery after log loss, before dependent script decisions resume. Preserve ordering relative to retained lines. Also audit the `while let Ok` drain in `Watcher::copy`, which exits silently on a receiver lag error.

**Regression test:** drop a material state delta, keep that field unchanged afterward, and verify the runner receives a full recovery and reads the correct field before continuing.

### I6 — P2: Web output can remain hidden after losing the end of a quiet command

**Source-traced; residual portion of earlier R8.** `crates/cena-web/src/presentation/pending.rs:65` records quiet-window transitions, and `:77` suppresses main-stream output while quiet. `missing()` at `:100` only records a gap. Snapshot fencing does not restore authoritative quiet state.

If quiet-on was observed but quiet-off is lost during subscriber lag, subsequent ordinary main output remains suppressed until another event happens to reset the flag. The GUI now clears quiet on a missed-event recovery, but the web presentation still has the old behavior.

**User impact:** the browser can look frozen or omit game output after falling behind, while the underlying session remains active.

**Recommended fix:** apply an explicit recovery policy to the web presenter, consistent with the GUI, or include authoritative quiet state in snapshot recovery.

**Regression test:** observe quiet-on, lose quiet-off, recover across a fence, and assert that the next ordinary main line is visible.

## User experience and configuration review

The original complaint—being told configuration is required without knowing how to supply it—is partly addressed in the integrated implementation. These are useful existing entry points, not proposed commands:

| Task | Existing route | Remaining usability work |
|---|---|---|
| Find settings | Launcher Settings; play-window Settings | Exercise the first-open experience with an empty data directory and one newly seated character. Recent first-open fixes are included in the inspected merge. |
| Discover commands | `;help` | Make this visible without requiring a player to guess the command. |
| Configure healing | `;heal help`, `;heal show`, `;heal set container herb pouch` | The help now contains an actual example. A missing-container failure should lead directly to the relevant settings page and current character. |
| Inspect/change a hunt | `;hunt help`, `;hunt show <profile>`, `;hunt set <profile> <setting> <value>`; settings pages already carry value origins | Verify that users understand inherited versus overridden values and can reset an override without consulting files. |
| Create a hunt with map picking | `;hunt setup` explains the web Configure hunt page, `--web --hunt-setup`, and `CENA_MAP` | This still requires knowledge of launch flags and map setup. Prefer a visible setup action that explains missing prerequisites and leads to each one. |
| Stop work | Play-window Stop and `;stop`; agent Stop | Treat I1 as a blocker to trustworthy stop semantics. Show what was stopped and what was already sent. |

Sources: `crates/cena/src/commands.rs`, `crates/cena/src/hunt/settings.rs:24`, `crates/cena-behavior/src/hunt/command.rs:226`, `crates/cena-gui/src/app.rs:184`, and `crates/cena-gui/src/play.rs:237`.

**Recommended next QoL pass:** walk a fresh user from launch to one configured heal and one valid hunt without editing a file or consulting the developer plans. Every failure should name the missing value, its scope, its current effective value where applicable, and the exact action to fix it. Separate an example configuration from a verified usable configuration; do not silently start automation as part of setup.

Add small focused usability checks for: missing map, no herb container, invalid profile, inherited setting, save failure, duplicate character names, reconnect, and Stop while queued. Assess whether a person can recover using only the screen they are looking at. These are recommended acceptance scenarios, not claims that each currently fails.

The IDE's `plan/50-settings-inventory.md` is useful development history, but its opening still says PROPOSED and “Nothing in this document is built,” and the opening inventory describes several values as hand-edit-only. Later implementation has overtaken that introductory snapshot. Give users a short current configuration guide and clearly label the inventory's historical sections; otherwise the documentation itself makes existing controls appear absent.

## Earlier findings after integration

“Addressed” below means the inspected fix covers the previously reported mechanism, not that every adjacent path is guaranteed correct.

| Earlier finding | Integrated assessment |
|---|---|
| R1: behavior continues after event loss | Addressed: lag tracking and snapshot recovery are called from the behavior loops. Do not mistake the remaining empty drain arm for the old defect without following recovery. |
| R2: trigger generation lost | Addressed: trigger actions carry generation and dispatch checks it. I2 is a separate agent path. |
| R3: trigger bypasses Ready | Addressed: trigger origins now require readiness. |
| R4: failed trigger reload claims none active | Addressed: failure messaging describes the retained matcher. |
| R5: concurrent settings edits lose updates | Addressed in the reviewed callers: whole read/change/commit transactions use the per-path lock; hunt validation precedes commit. |
| R6: bare names route the wrong session | Addressed for the reviewed GUI, relay, and party paths; agent registry remains affected (I3). |
| R7: quit notification/closing wait | Addressed by the waiter fix. |
| R8: quiet state survives lost end event | Addressed in GUI; still present in web (I6). |
| R9: invalid timer interpreted as zero | Addressed: malformed values are not converted into a valid zero timer. |
| R10: GUI command validation gaps | Addressed: both ordinary and quiet sends validate before dispatch. |
| R11: account reservation released too soon | Addressed: stopping sessions remain accounted for until cleanup. |
| R12: settings Map/Text appear absent | Addressed: rendering uses the displayed value. |
| R13: stale object menu answers | Addressed: request generation and answer sequencing fence the menu; generation changes clear it. |
| R14: static links ignore learned commands | Addressed: link resolution consults learned state. |

## Crate coverage

| Crate | Review focus / conclusion |
|---|---|
| `cena-platform` | Transport, replay/fake sources, persistence/logging boundaries; used the real fake-transport support for probes. No additional confirmed finding. |
| `cena-protocol` | Parser/error boundaries and malformed timer handling; R9 fix checked. |
| `cena-model` | Folded state, snapshots, and data consumed by automation/UI; no additional confirmed finding. |
| `cena-map` | Route/map integration and prerequisite exposure; no additional confirmed finding. |
| `cena-session` | Actor admission, generations, operations, Stop, agent policy, stores, and snapshot boundaries; I1, I2, I4. |
| `cena-behavior` | Hunt/travel recovery, profiles/help, trigger state and readiness integration; earlier lag issue addressed. |
| `cena-ui` | Settings representation and shared command/menu contracts; earlier value-display issue addressed. |
| `cena-web` | Presentation recovery, quiet windows, setup discoverability, and JavaScript checks; I6. |
| `cena-agent` | Character registry/tools, authorization flow, record queries, bounded script log, Ruby copy/listener and process lifecycle; I3, I5. This new crate merits continued focused review. |
| `cena-host` | Session ownership, close/wait, reservation lifetime, and qualified identity; earlier fixes checked. |
| `cena-gui` | Settings access/rendering, command validation, learned links, object-menu lifetime, quiet recovery, layout/drag integration; earlier defects addressed. No interactive desktop validation. |
| `cena` | Actual wiring of settings/help, profiles, stores, agent seating, triggers and session lifecycle; I3 depends on this wiring. |
| `cena-arch-tests` | Workspace structural enforcement and regression coverage; architecture checks complement, but do not prove, runtime cross-component correctness. |

## Validation

All standard integrated checks completed successfully:

| Check | Result |
|---|---|
| `cargo test --workspace` | Exit 0; **3,738 passed, 0 failed, 15 ignored**, summed across test binaries and doctests. |
| `cargo test -p cena-gui` after the final GUI merges | Exit 0; **268 passed, 0 ignored**. This is an additional rerun, not added to the workspace total above. |
| `cargo fmt --check` | Passed, including a rerun after the final GUI merges. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `cargo doc --workspace --no-deps` | Passed; no warning/error diagnostics in its log. |
| JavaScript tests | **71 passed, 0 failed**. |
| Offline review probes | Four failure mechanisms reproduced as documented above. |
| `git diff --check` | Passed. |

The ignored tests require external Lich installs/scripts, map/route fixtures, real-traffic corpora, or explicitly opted-in measurements; one is an ignored documentation example. They were not enabled. No interactive browser smoke or desktop walkthrough was performed. Passing checks do not contradict the findings: the probes exercise conditions not covered by the existing regression suite.

Final inspected HEAD: `75adedb`. The workspace run spans the moving-tree interval described above; the separate GUI rerun covers the final GUI delta. Logs and probes are under `target/crate-review-integrated-*` and `target/integrated-review/`.

The offline probes use production APIs and an `AnsweringSource`, not a live game. Their source is `target/integrated-review/probe.rs` and `lag.rs`; outputs are `output.log` and `lag-output.log`. They are scratch artifacts, not committed regression tests.

## Suggested implementation order

1. Repair the agent command lifecycle together: cancellation before write and generation preservation (I1–I2).
2. Make agent character selection unambiguous and close the known drop alias (I3–I4).
3. Add full script-state recovery and web quiet recovery (I5–I6).
4. Perform the fresh-user heal/hunt walkthrough, turn prerequisite messages into direct recovery actions, and publish a current short configuration guide.

Retain the architecture and regression checks, but add tests at these cross-component boundaries. The present defects survive because each component can appear correct in isolation while authority, identity, or state is lost between them.
