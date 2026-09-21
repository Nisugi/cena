# M4 Despana working status

Checkpoint: 2026-09-21. This is a local implementation record, not release acceptance.

## Resume here

- Checkout: `/mnt/FastStorage/SSD Games - Non Steam/cena-despana-m4`
- Branch: `feat/despana-m4`; base `804068238c91a4a4377b3ed7996dac9188ca88b7`.
- Author decisions: [M4 frontend](23-m4-frontend.md).
- Work contract: [implementation plan](m4-despana-implementation.md).
- User authorized local clones, subagent implementation, documentation and eventual Sophia mining.
- No live game login or installed application changes have been performed.
- ATARI authorized publishing this branch as a draft PR to `Nisugi/cena` on
  2026-09-21. Draft publication is not M4 acceptance or merge approval.

## Lanes

| Owner | Scope | Status |
| --- | --- | --- |
| Astra | Interface decisions, manifests, CLI integration, review and verification | Integrated locally; final architecture suite, clippy and formatting pass |
| `m4_session_observation` | Native coherent late subscription and generation-pinned manual input | Implemented; 11 observation tests including concurrent ingress and active abort |
| `m4_mobile_gate` | Android/iOS CI and verification record | Implemented; CI unrun |
| `m4_ui_model` | Pure presentation types and styled line assembly | Implemented; 17 tests passing |
| `m4_web_server` | Authenticated transport and bounded projection pump | Implemented; 11 tests passing |
| `m4_browser` | Despana surface and browser contract tests | Implemented; 14 contract tests and desktop/mobile fixture smoke passing |
| `m4_native_integration_tests` | Real native session and authenticated WebSocket integration | Four tests passing; targeted clippy clean |
| `m4_session_review`, `m4_web_review` | Independent read-only review | Completed; two web findings fixed and tested, session coverage gaps tested |

## Settled interface decisions

- Native `SessionObserver::subscribe().await` returns a coherent `Snapshot` and a
  receiver of stamped `ObservedEvent { session, generation, cursor, event }`.
  Snapshot capture and receiver creation occur in one actor/supervisor turn.
- Preserve existing native event consumers. Observing does not claim command authority.
- Native `send_manual_at(generation, line, deadline)` will use the existing queued
  manual path and return its `Outcome`, not bypass the queue with `send_now`.
  Stale-generation rejection must precede special-case logout handling.
- Browser state is a deliberately scoped view, not raw native state or protocol frames.
- Frontend lifecycle and command uncertainty must stay visible; no automatic command replay.
- Retry attempts are nullable until known. `retry_delay_ms` is the original scheduled
  backoff, never presented as a remaining-time countdown.

## Verification so far

- Pre-publication rerun on 2026-09-21: observation 11/11, UI 17/17, web 11/11,
  native/WebSocket integration 4/4 and browser contracts 14/14 pass. Architecture
  tests, formatting and workspace clippy also pass. An initial missing-API compile
  failure used stale `cena-session` build artifacts (its dependency record omitted
  the new observation modules); a package-only build cleanup resolved it without
  source changes.
- Mobile workflow YAML and embedded shell syntax checked by the CI agent.
- Mobile CI has not run; no cross-target success is claimed. See
  [mobile verification](m4-mobile-verification.md).
- Pinned Rust 1.96.1 installed and dependencies fetched. Mobile probes reach compilation
  but target standard libraries are not installed locally; CI remains unrun.
- Native observation and existing regression tests pass (independent reviewer: 47 tests).
- Full workspace run with loopback permitted: **1,821 passed, 1 failed, 3 ignored**,
  across 182 result summaries. The subsequently added fourth native/WebSocket test also passes;
  all four pass together in the final targeted run. No live service was used.
- The single workspace failure is `character_store::the_name_matches_regardless_of_case`.
  Reproduced with the same pinned toolchain on an untouched worktree of starting commit
  `804068238c91a4a4377b3ed7996dac9188ca88b7`; this change leaves character-store code untouched.
- Workspace rustdoc fails on existing broken/private links: 21 diagnostics in `cena-model`,
  plus a private `Live` link in `cena-session` when checked separately. Both failures were
  reproduced on that untouched starting commit. New `cena-ui` and `cena-web` rustdoc passes.
- Workspace clippy with warnings denied, formatting and diff checks pass. Architecture
  rule failures found during development were fixed without raising or weakening caps.
- Browser smoke passed in headless Brave via existing Playwright, including focused input
  during pending receipts and no replay on reconnect. Controller inspected desktop and
  narrow-screen screenshots. This is fixture evidence, not mobile device/game acceptance.
- Exact production request-history rollover tested: 1,024 stale requests produce no native
  writes, request 1,025 is refused before reconnectable close, and a new viewer can send once.

## Remaining acceptance / handoff

- See [operator handoff](m4-despana-handoff.md) for scope and launch/testing commands.
- Author-present live game test, Windows CI and Android/iOS CI remain unrun.
- The baseline test and rustdoc failures need separate upstream attention before claiming
  an entirely green workspace. No unrelated storage or model fixes were folded into M4.
- Draft publication is authorized; deployment, login and profile migration remain out of scope.
- Sophia indexing/mining is a follow-up after the reviewed revision is stable.

## Durable knowledge

This file and the implementation plan are the local source of truth while code changes.
Sophia had no prior Cena/M4 record at orientation. Register/index the stable checkout and
mine selected high-value modules after integration; do not treat a stale summary as proof
of the edited source. A final controller closeout should link exact commits and tests.
No source mining run has been started for this checkout. Following explicit destination
approval, Sophia wiki page `hydra-despana-m4-integration` was saved and read back successfully
on 2026-09-21 (artifact `3c375623-3348-4ece-9bb1-618e43db7030`). It records the architecture,
repository and local documentation references, verification results and remaining acceptance
gaps. It is authored synthesis, not mined source evidence. The local plan and this checkpoint
remain authoritative while implementation changes.
