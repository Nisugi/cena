# M4 Despana implementation work plan

Date: 2026-09-21. Local implementation branch: `feat/despana-m4`.
Baseline: `804068238c91a4a4377b3ed7996dac9188ca88b7`.
Authority: author decisions in `23-m4-frontend.md`, with `12` and `05` governing architecture.
ATARI requested planning followed by delegated implementation. This authorizes local work,
not a live login, deployment, push, or an assertion that M4 is accepted by Nisugi.

## Outcome and limits

An optional, authenticated, embedded web frontend for a running Hydra session, using
Despana's desktop presentation approach. First slice: Story, manual command input,
room/title/description/exits/contents, hands, vitals, roundtime, lifecycle and retry status.
Single session, identified on every session message. Closing a browser never closes the
game. Existing CLI login remains the only login mechanism.

No maps, travel, macros, command history, aliases, account picker, settings import, Lua,
agent interface, session manager, or automatic panic recovery in this slice. Lua is deferred,
not rejected. No compatibility promise for Vellum's wire protocol. No changes to installed
Vellum, Lich, LAB, character profiles, or running game sessions.

## Source-grounded starting points

- VERIFIED: `cena-ui/src/lib.rs` is a placeholder; the model and input vocabulary belong here.
- VERIFIED: `SupervisedSession::subscribe` returns `Connecting` unconditionally. Its `run`
  consumes the owner, so changing that literal alone cannot supply a detached late subscriber.
- VERIFIED: `SessionHandle` is the command path; browser commands must remain manual input.
- VERIFIED: Vellum Despana's `session.js` isolates its transport, but `app.js` also requests
  map assets directly. Port the scoped presentation, not the entire backend or protocol.
- VERIFIED: the existing CLI `Screen` assembles line fragments; line boundaries must not be
  inferred from WebSocket messages or individual parser frames.

## Integration contract

### Ownership

`cena-session` owns current state, lifecycle, generation and command execution. It provides
a detached read-only observation capability with a coherent initial snapshot and subsequent
events. No web consumer independently applies protocol frames to reconstruct GameState.
Late attach and lag recovery need real current snapshots, not the pre-run initial state.
The implementation must specify its snapshot/event ordering fence and test that race.

`cena-ui` owns small serializable presentation types, input vocabulary, pure projection of
model state and line assembly. It does not depend on session, protocol, HTTP or browser types.
`cena-web` maps native session events to those types, retains bounded display history,
authenticates the browser, serves bundled assets, and submits validated manual input.
The binary wires these together as an optional guest task, with no second backend process.

### Wire and browser rules

- Version 1 JSON messages, explicitly tagged by kind. Session identity and connection
  generation are lossless strings on the wire; presentation sequence is monotonic within
  the server stream. The exact Rust/JS field schema is finalized together before integration.
- Browser authenticates before receiving state or sending commands. Tokens are fresh,
  cryptographically random, memory-only, not game credentials; never use a query-string token.
  A launch URL fragment may bootstrap the browser, which immediately removes it from the URL.
- Listener binds loopback only in M4. Check Host/Origin against the actual local listener;
  no wildcard CORS. Reject unauthorized, oversized, malformed and unsupported messages.
- First application message is a coherent snapshot. Later messages update displayed state,
  append complete styled Story lines, report lifecycle/retry, or return command receipts.
- Snapshot plus updates must not have a subscribe gap. On broadcast lag, resynchronize from
  native current state, explicitly mark lost Story history, and do not invent missed text.
- A changed game generation clears invalidated state and partial line assembly. Unknown
  vitals/hands/roundtime remain unknown, not zero, full, or empty. Browser reconnect obtains
  a fresh snapshot; there is no command outbox, automatic resend or replay.
- A manual command carries the observed session/generation and a bounded request ID. Check
  generation at the actual native send decision, not just in the WebSocket worker. Validate
  one nonempty line and bounded size; refuse newline/NUL injection. Use existing manual
  semantics without claiming/cancelling behavior authority.
- Duplicate request IDs within one WebSocket connection are rejected; no cross-reconnect
  retries are attempted. A lost receipt means unknown delivery, not permission to resend.
- Receipt language distinguishes bytes sent, refused, and uncertain outcome. It never calls
  socket write success proof that the game performed the action.
- Slow browsers cannot block the game reader. All queues/history/message sizes are bounded;
  backpressure leads to explicit resync or viewer disconnect, not game-state loss.
- Render text through DOM text nodes; never interpret game text as HTML. Full markup fidelity
  is deferred, but supported run styles and unknown-tag diagnostics must remain observable.

### Testing shape

No live login. Real committed XML fixtures exercise the existing parser/model/session through
presentation mapping. Synthetic fixtures are explicitly labeled for lifecycle/security/race
conditions, not offered as evidence for undocumented game behavior. WebSocket tests use local
loopback only; browser tests use fixtures. No credentials are read or recorded.

## Bounded delegated work

| Work | Owner | Files | Verification |
| --- | --- | --- | --- |
| Native late-subscribe lifecycle and coherent observation | session agent | `cena-session` + its tests | Attach while Ready, reconnect invalidation, lag recovery, terminal state; stale-generation manual input |
| Mobile compile gate | CI agent | workflow + verification note | Try available target checks; document unavailable NDK/SDK; no unrun pass claims |
| UI state/input and styled line assembly | UI-model agent | `cena-ui` only | Projection/serialization, unknown values, line/stream boundaries, fragment reset |
| Authenticated web server and event mapping | web agent | `cena-web` Rust/manifests/tests | Auth/Origin/Host, snapshot order, lag, command receipts, stale generation, bounded queues |
| Despana browser surface | browser agent | `cena-web/assets` + JS tests | Fixture-driven rendering, reconnect, no command replay, text safety, keyboard usability |
| Planning, shared manifests/arch rules, CLI wiring, integration and independent review | Astra + later reviewer | root/arch tests/binary/docs | Workspace fmt/clippy/doc/tests, real fixture replay, frontend smoke |

No agents edit another lane without a handoff. No broad `cargo fmt` until integration; format
owned files only while parallel work is active. Agents report changed files, exact tests,
known gaps, and any deviation from the agreed contract. No commits or pushes by workers.

## Build order and gates

1. Establish baseline and run existing tests while implementing the lifecycle prerequisite
   and checking mobile CI. Record pre-existing failures independently.
2. Finalize concrete wire DTOs and the detached observation interface. No speculative facade
   or opaque passthrough of parser/model structures to JavaScript.
3. Implement pure UI projection and line assembly, then event mapping and authenticated
   server; browser work proceeds against the shared fixture contract.
4. Wire `--web` into the current binary, leaving non-web CLI behavior intact. Expose the
   ephemeral listener URL only as an explicit pairing handoff; never log credentials.
5. Exercise end-to-end replay, late attach, auth failures, reconnect, generation invalidation,
   viewer backpressure, manual input during a behavior and command-delivery uncertainty.
6. Independent code/security review, fix findings, then rerun workspace and browser checks.
7. Hand off local build and instructions. Live game acceptance remains pending Nisugi's
   presence and explicit coordination; offline success is not live acceptance.

## Verification and rollback

Required: `cargo fmt --check`, workspace tests, architecture tests, clippy with warnings denied,
rustdoc, browser contract tests and fixture-based UI smoke. Cross-target results must state
the actual target/toolchain and distinguish local execution, added CI, and CI not yet run.
Do not raise file caps or weaken architectural tests to make the new crates fit; record
deliberate dependency additions alongside their allowlist entries.

Rollback is local: omit `--web` / optional frontend integration or revert the feature branch
after review. No installed application or character data is migrated by this work.
Any broader native lifecycle change discovered during implementation must be justified by
the first frontend's concrete needs and tested independently, not folded in as a hidden M5.
