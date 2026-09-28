# Cena workspace crate review — 2026-09-27

## Assessment

Cena has substantial implementation and test coverage, with useful architectural protections already in place. The highest-risk gaps found in this review are at the boundaries: automation consuming incomplete observations, trigger actions crossing connection lifetimes, concurrent settings edits, and session identity being reduced to a character name. These can make a feature behave incorrectly even when its individual parser, model, or command tests pass.

I recommend addressing findings 1–6 before expanding unattended automation. Findings 7–11 should accompany the next reliability and frontend pass. This is a review, not an implementation change: production sources were left unchanged.

The review covers all 12 workspace crates at HEAD `e028508db153a8bc76f99b83ba0585afba0a9934`, including the working tree as inspected. Existing changes to `plan/28-gui-inventory.md` and the earlier `plan/44-user-qol-review.md` were preserved. The earlier QoL report is not treated as a list of current defects: settings commands, the native GUI, triggers, and relaying have developed since then.

## Scope and evidence

This was a risk-directed source review across every crate, with deeper tracing of commands, observations, reconnects, persistence, session lifecycle, and frontend presentation. It is not a claim that every source line or every gameplay rule was audited.

- **Reproduced:** an offline probe exercised the relevant production implementation and observed the failure. These are small diagnostic probes, not permanent regression tests.
- **Source verified:** the failure follows from the inspected implementation and a stated sequence of events. The complete concurrent or live scenario was not reproduced.
- **Follow-up:** an area needing measurement or interactive testing; not counted as a confirmed defect.

P1 means address before relying on the affected automation or concurrent operation. P2 means a concrete correctness or usability defect with a narrower trigger. These are engineering priorities, not security severity ratings. No P0 issue was established. The report contains **6 P1 and 5 P2 findings**, four with offline reproductions and seven established through source tracing.

## Findings

### 1. P1 — Automation silently continues after losing observations

**Evidence: source verified.**

The hunt and travel drivers fold events into their own state. Both ignore broadcast lag and continue acting: `crates/cena-behavior/src/hunt/drive.rs:374` and `:393`, and `crates/cena-behavior/src/travel/drive.rs:592` and `:671`.

If a driver falls behind and misses a room, inventory, flag, or lifecycle update, its local state can remain wrong. A later game event does not necessarily repeat the missing information. The session's command gate cannot repair a stale movement or inventory decision merely by checking roundtime and connection readiness.

**Change:** treat lag as loss of trustworthy state. Stop safely or obtain a fresh authoritative snapshot with an observation fence before resuming. Preserve the metadata needed to identify the snapshot's connection lifetime.

**Regression check:** force receiver lag using a small channel, omit a material room/inventory update, and verify that no state-dependent command is emitted until recovery completes. Cover both the nonblocking drain and awaited receive paths.

### 2. P1 — Queued trigger actions can execute on a newer connection

**Evidence: source verified.**

`crates/cena/src/triggers/follow.rs:67` consumes a plain `Receiver<Event>`; at `:79` it dispatches `Event::Act` without the originating generation. `crates/cena/src/triggers/act.rs:15` sends through the current command table or `send_now`. `crates/cena-session/src/command/handle.rs:494` stamps that send with the handle's current generation.

A trigger action queued on connection A can be consumed after reconnection to B and be accepted as a command belonging to B. This also matters for a trigger invoking a semicolon command that starts a behavior. Generation protection at the actor does not help if the caller has replaced the original generation with the new one.

**Change:** carry the originating observation generation through trigger dispatch and use a generation-pinned send. Reject stale actions before local command dispatch as well as before the socket write.

**Regression check:** queue a trigger action, reconnect before consuming it, then verify neither a game command nor a local behavior starts on the replacement connection.

### 3. P1 — Trigger game commands bypass the synchronization readiness gate

**Evidence: reproduced at the session dispatch boundary; application trigger path source verified.**

`crates/cena/src/triggers/act.rs:22` uses `Origin::Trigger` with `Gate::None`. The readiness check in `crates/cena-session/src/actor/io.rs:393` applies only when `origin.is_behavior()` is true; triggers are a distinct origin.

An offline session still in `Syncing` accepted a trigger-origin `stand` command and wrote it to the fake transport. Therefore a matching trigger during setup can send automation before the session has established the state that behavior commands require. Manual input being allowed during setup does not require automated triggers to receive the same exemption.

**Change:** make the readiness policy explicit for automated trigger sends. Avoid queuing them indefinitely and then replaying obsolete setup actions when Ready is reached. Combine this with the generation check in finding 2.

**Regression check:** fire a sending trigger during setup and after Ready, and verify only the latter sends. Retain separately tested manual-command behavior.

### 4. P1 — A failed trigger reload says none are active while old triggers remain active

**Evidence: source verified.**

`crates/cena/src/triggers/load.rs:31` can return on a file read or parse error before replacing the matcher at `:55`. The error message at `:70` says **“None are on.”** The shared-file follower says the same at `crates/cena/src/triggers/follow.rs:110`.

Starting with active sending triggers, damage the TOML and reload it. The old matcher remains installed, so old triggers can continue sending while the UI tells the player none are on. The misleading state report is especially consequential for automation.

**Change:** choose one explicit policy. Either retain the last working matcher and state that it remains active, or disable it and accurately report that result. Report startup failure separately from failure to replace an existing matcher.

**Regression check:** install a working sending trigger, make the file unreadable or malformed, reload, and assert both the actual matcher state and the exact user-facing status.

### 5. P1 — Shared configuration edits can lose updates and interfere through temporary files

**Evidence: source verified; concurrent interleaving not stress-tested.**

Trigger changes perform an unlocked read/edit/write transaction in `crates/cena/src/triggers.rs:146`; the writer at `:229` uses a shared `.toml.new` temporary filename. Hunt profile edits at `crates/cena/src/hunt/settings.rs:69` read, save, reload for validation, and sometimes restore the old file. Their writer, `crates/cena-behavior/src/settings.rs:78`, uses a fixed `.toml.saving` path.

Commands from multiple sessions, or concurrent command tasks, can read the same original document and overwrite each other's changes. Shared temporary names add write/rename interference. A hunt edit that rolls back can restore an old document over another task's successful edit. Atomic rename alone does not make the read/modify/write transaction atomic.

**Change:** serialize the full transaction per destination file. Validate a candidate before committing where possible; avoid restoring a stale pre-edit copy after another writer can have committed. Unique temporary names help file replacement but do not solve lost updates. `crates/cena-session/src/travel_store.rs` already provides a useful example of locking a read/edit/write transaction.

**Regression check:** coordinate two edits with barriers so they read the same starting state; verify both changes persist. Also interleave an invalid edit with a successful edit and verify rollback cannot erase the successful result.

### 6. P1 — Character names are used as control identities across instances

**Evidence: source verified; requires duplicate character names on distinct sessions/accounts.**

The roster supports qualified `GAME:Name` selections, but downstream paths discard that distinction:

- `crates/cena/src/play.rs:458` constructs relay destinations from bare character names.
- `crates/cena/src/relay.rs:85` returns the first exact name match instead of rejecting duplicate exact matches.
- `crates/cena/src/hunt.rs:201` stores party seats by name; inserting a duplicate replaces the prior seat. Removal at `:183` is also by name.
- `crates/cena/src/play.rs:417` filters available roster entries by bare running name, hiding a different instance's same-named character.

A relay can target the wrong session; party membership can be overwritten or removed by the other session's cleanup. The same name-only assumption also affects the trigger reload follower's identification of its own change.

**Change:** use `SessionId` for running-session control and a qualified identity for persisted character selection. Qualify display names when necessary. An ambiguous bare name must produce a useful selection error, including when multiple entries are exact matches.

**Regression check:** run two offline sessions with identical character names on different accounts/instances. Exercise relay selection, party join/removal, available-character listings, and shared trigger reload notifications.

### 7. P2 — A second quit reports server acknowledgment before the server closes

**Evidence: reproduced.**

In `crates/cena-session/src/actor/io.rs:277`, `begin_quit` immediately returns `Farewell::Acknowledged` to a second caller when an earlier quit is still pending and has its reply sender. That is not evidence of server EOF.

With a fake server that never closes, the second quit returned `Acknowledged` while the first eventually returned `TimedOut`. Callers receive contradictory information about the same logout.

**Change:** coalesce quit requests and deliver the actual eventual outcome to every waiter, or explicitly return a distinct “already pending” result. Do not use the acknowledged result to mean merely that a quit request exists.

**Regression check:** issue two quit requests before EOF and test both eventual EOF and timeout. No caller should report server acknowledgment before EOF.

### 8. P2 — Losing the end of quiet mode can leave ordinary game text hidden

**Evidence: reproduced in the GUI story implementation; equivalent web recovery gap source verified.**

`crates/cena-gui/src/story.rs:104` records a missed-event gap but does not recover its `quiet` state. If `Quiet(true)` was seen and `Quiet(false)` was lost, later main-stream lines remain suppressed until another event happens to correct the state or the generation changes.

The web presentation's `missing` and `fence` methods, `crates/cena-web/src/presentation/pending.rs:100`, similarly do not recover quiet state. A newly attached presentation also has no authoritative quiet value in the session snapshot from which to initialize.

The GUI probe delivered quiet-on, simulated missed events, and then delivered an ordinary game line. The visible result contained only a gap marker; the ordinary line was discarded.

**Change:** make presentation suppression recoverable from an authoritative snapshot/fence. Merely clearing quiet on every gap prevents indefinite hiding but can expose output that is still intentionally suppressed; define the recovery behavior deliberately for both frontends.

**Regression check:** drop quiet-off, then deliver ordinary lines and a recovery snapshot. Also attach a frontend during an active quiet interval.

### 9. P2 — Malformed timer values become a known zero and clear active roundtime

**Evidence: reproduced.**

`crates/cena-protocol/src/parser/thin.rs:60` parses roundtime, cast time, and timer values with `unwrap_or_default()`. Invalid numeric data therefore becomes a valid zero-valued frame. The model accepts the roundtime frame as a new known timer value.

The probe established active roundtime with a valid frame, then fed a roundtime tag whose value was `broken`. The parser emitted `RoundTime { value: 0 }` and the model changed from active roundtime to inactive roundtime. This is a malformed-input resilience defect; the review does not establish that the live game routinely sends such data.

**Change:** distinguish malformed/missing numeric values from a legitimate zero. Preserve the previous trustworthy value or explicitly mark it unknown according to the model's readiness policy. Apply the policy consistently across the timer tags.

**Regression check:** cover missing, nonnumeric, negative, and overflowing values after a valid active timer, and separately verify a legitimate zero.

### 10. P2 — GUI keybindings can submit multiple wire commands as one command

**Evidence: source verified.**

`crates/cena-gui/src/keys.rs:268` validates key chords but accepts their command strings without single-line validation. `crates/cena-gui/src/sessions.rs:290` passes those strings to `send_manual_at`. This path does not apply the bounded single-line validation available in `cena-ui`.

A TOML binding such as `F1 = "look\nquit"` is accepted as a single binding and can reach the wire as two commands under one submission. This can bypass whole-command handling and make tracking or quit behavior inconsistent. The browser's input validation does not protect the native GUI path.

**Change:** enforce single-command constraints at a shared submission boundary and report invalid bindings when loading them. If multiple commands are a supported feature, represent them explicitly and dispatch each through the normal command handling path.

**Regression check:** load bindings containing LF, CR, NUL, and excessive length; verify a clear diagnostic and no raw write. Verify valid bindings still reach local command handling and game sends correctly.

### 11. P2 — Removing a session frees its account before shutdown completes

**Evidence: source verified; concurrent frontend requests not reproduced.**

`crates/cena-host/src/table.rs:180` removes a session immediately. The account-in-use check in `Host::add` only examines sessions still in the table. `crates/cena/src/play.rs:392` takes the session under the host lock, releases the lock, and then awaits `hosted.stop()`.

While the old connection is still waiting for logout, another request can add a character on the same account. That defeats the host's stated one-character-per-account rule during the shutdown window. Cleanup of the old session can also overlap setup of the replacement; name-keyed party cleanup aggravates this, as described in finding 6.

**Change:** retain an account reservation in a stopping state until connection shutdown and dependent cleanup have completed. Serialize the relevant account lifecycle without blocking unrelated accounts behind a global network wait.

**Regression check:** delay old-session EOF, request removal, then request an add on the same account. Verify the replacement is refused or explicitly queued until shutdown completes, while an unrelated account can still start.

## Crate coverage

| Crate | Areas reviewed | Result / remaining limits |
| --- | --- | --- |
| `cena-platform` | Byte sources, transport bounds, authentication/TLS paths, pin handling, recording and redaction | No additional actionable defect established in inspected paths. Live authentication, network failure behavior, and OS credential stores were not exercised. |
| `cena-protocol` | Incremental framing, settings payload recovery, entities/markup, numeric attributes | Finding 9. More malformed-input/fuzz coverage would complement the existing fixture tests. |
| `cena-model` | State folding, reconnect state, flags/streams, trigger matching and pacing | Timer consequence in finding 9; trigger integration findings span the binary/session boundary. No independent model rewrite recommended. |
| `cena-map` | Binary loading bounds, exit indexing, route selection and conditions | No additional concrete blocker established. Actual large-map route performance and live navigation remain outside verification. |
| `cena-session` | Actor writes and gates, handle APIs, observation metadata, roundtrips, supervisor, persistence and recorder workers | Findings 2, 3, 7, 9, 10. Existing generation-aware APIs are useful but not consistently used by every caller. |
| `cena-behavior` | Hunt/travel execution, event folding, setup/configuration, profile persistence and trigger loading | Findings 1 and 5. Gameplay-specific combat correctness was not exhaustively audited. |
| `cena-ui` | Command validation, projections and merged history | Shared validation exists; finding 10 is an integration gap. No separate concrete blocker established. |
| `cena-web` | Session routing/socket handling, presentation buffering and recovery, browser tests | Finding 8. No live browser/server walkthrough or security penetration test performed. |
| `cena-host` | Session ownership, account uniqueness, add/remove/shutdown | Finding 11; identity consequences in finding 6. |
| `cena-gui` | Session feed/send, story rendering state, keybindings and application/session controls | Findings 8 and 10. Native focus, DPI, accessibility, sound and window interaction still need a hands-on pass. |
| `cena` | Command wiring, trigger dispatch/reload, settings transactions, relay/party identity, application lifecycle | Findings 2–6 and 11. This is where several otherwise sound crate contracts are lost. |
| `cena-arch-tests` | Dependency/layering checks and source-policy checks | Valuable structural enforcement. Behavioral boundary regressions above require runtime tests as well as architecture checks. |

“No additional defect established” is limited to the inspected paths; it is not certification that a crate has no defects.

## What is working well

- Session IDs and connection generations already exist. The solution is largely to retain and enforce them through every dispatch path rather than invent another identity system.
- There are fake transports and rich fixture tests, making the important failure sequences testable without a live account.
- Command validation and UI projections are shared in a dedicated crate. The GUI can use the same contracts as the web client.
- The travel store already serializes its update transaction, offering an internal pattern for shared settings writes.
- Crate boundaries, lint rules, and architectural tests provide a useful maintenance baseline. The newly found defects largely concern semantic contracts across those boundaries.

## Recommended work order

1. **Automation trust:** recover or stop on observation lag; preserve trigger generation; require readiness for trigger sends; make failed reload status truthful. Deliver explicit tests for stale or incomplete input before changing automation behavior further.
2. **Shared ownership:** serialize configuration transactions; retain account reservations through shutdown; replace name-keyed control with session identity.
3. **Frontend and protocol recovery:** repair quiet-state resynchronization, validate native command submissions, preserve malformed timer uncertainty, and coalesce quit results correctly.
4. **Hands-on QoL follow-up:** walk a new profile from setup through `;heal`, `;hunt`, trigger creation, reconnect, and stopping. Every refusal should identify what is missing and the exact next command or control. Reassess current behavior against the old QoL report rather than assuming old findings remain open.

For broader confidence, follow with recorded-session load testing: deliberately stall consumers, reconnect while commands are queued, and edit shared profiles from two sessions. Measure memory and latency under large streams and many regex matches before declaring a performance problem; this review did not establish one.

## Verification

Local command logs and the offline probe source are under `target/crate-review*`; these are ignored review artifacts, not committed regression tests.

- `cargo fmt --check`: passed.
- `node --test crates/cena-web/assets/tests/session.test.mjs crates/cena-web/assets/atlas/tests/*.mjs`: 71 passed, 0 failed.
- Four offline probes: reproduced findings 3, 7, 8 (GUI), and 9. Finding 3 exercised the production session dispatch API with the same origin/gate used by trigger dispatch, not a complete trigger-to-reconnect scenario.
- `cargo test --workspace`: passed, exit 0. Aggregated test summaries: **3,362 passed, 0 failed, 7 ignored**, including doctests.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed, exit 0.
- `cargo doc --workspace --no-deps`: passed, exit 0.

Probe output:

```text
roundtime before malformed tag: Some(true)
malformed roundtime frames: [RoundTime { value: 0 }]
roundtime after malformed tag: Some(false)
GUI story after missed quiet-end and ordinary line: [Gap]
trigger during Syncing: Ok { at: None }, wire=["stand"]
quit without server EOF: second=Acknowledged, first=TimedOut
```

No live login or gameplay commands were run. No claim is made about macOS/Linux/mobile build success, interactive GUI quality, or live-server outcomes. Review recommendations still need implementation and regression verification.
