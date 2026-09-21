# Hydra / Despana M4 handoff

Local prototype implementation, 2026-09-21. Not a release or live-game acceptance.

## What is built

An optional embedded browser frontend for one native session: styled Story,
manual commands, room details and contents, hands, vitals, roundtime, lifecycle
and reconnect information. The UI uses Despana's charcoal/amber presentation.
The native session remains the sole owner of parsing, state and command arbitration.

The three interfaces are deliberately separate:

1. `cena-session` supplies fresh, coherently fenced snapshots and stamped events,
   plus generation-pinned manual commands through the existing queue.
2. `cena-ui` supplies toolkit-free view/input types and bounded styled-line assembly.
3. `cena-web` authenticates loopback viewers, projects native state, serves bundled
   assets, and translates validated commands. The executable owns its lifetime.

The browser does not receive raw parser frames, claim routine authority, or keep
a command outbox. A missing delivery receipt remains uncertain. Reconnection
gets a fresh snapshot without resending commands. Closing the viewer does not
close the game. Story history is bounded output retained since the web pump
started, not a reconstruction of earlier native output.

## Launch, when the author is present

The current CLI still owns login. Its existing password-entry limitations remain;
do not use a shared/recorded terminal. No account UI or credential store was added.

```sh
cargo run -p cena -- --web
```

Open the private loopback pairing URL printed by the executable. Its fragment
contains a process-local capability; do not share or capture it in screenshots.
The browser removes the fragment immediately and retains the capability only in
memory. A page reload therefore needs the pairing URL again. The server binds
IPv4 loopback only; this is not a remote-phone access endpoint.

Web mode stays open until Ctrl-C or native session completion. An explicit
`--hold 60` still limits the session to the selected hold period. Without `--web`,
the existing ten-second CLI demonstration default is preserved. The optional
frontend cannot silently turn on the demo behavior or other command probes.

## Offline verification

```sh
cargo +1.96.1 fmt --check
cargo +1.96.1 clippy --workspace --all-targets --offline -- -D warnings
cargo +1.96.1 doc --workspace --no-deps --offline
cargo +1.96.1 test --workspace --no-fail-fast --offline
node crates/cena-web/assets/tests/session.test.mjs
```

WebSocket integration tests bind local sockets and need an environment permitting
loopback. They use committed scrubbed XML captures and synthetic transport timing,
never a live account. The browser visual smoke is separately documented in
[browser smoke instructions](../crates/cena-web/browser-tests/README.md); its fixture is not a live game.

Current results and known failures: [working status](m4-despana-status.md).
Android/iOS prerequisites and unrun CI: [mobile verification](m4-mobile-verification.md).
Wire schema and its cross-language fixture: [frontend contract](../crates/cena-ui/WIRE.md).

## Deliberately not claimed

- No live login, installed-app replacement or upstream merge. Publication as a draft PR
  was authorized after the initial local handoff; it does not establish acceptance.
- No map renderer, travel, account picker, macros/history, inventory/skills UI,
  Lua runtime, multi-session manager, or per-session panic recovery.
- No mobile device/backgrounding acceptance. Added compile jobs are not executed CI results.
- No sandbox against hostile code running as the same OS user. Pairing plus
  Host/Origin checks protect the browser access path, not a compromised host.
- No guarantee that a write receipt means the game performed the requested action.

## Knowledge and continuation

The [implementation plan](m4-despana-implementation.md) and status page preserve
ownership, decisions and evidence across handoffs. The checkout is ready for later
Sophia registration/indexing after the changes stabilize. Mining should cite an
exact reviewed revision and exclude credentials and personal/live game logs.
