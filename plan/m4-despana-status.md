# M4 Despana working status

Checkpoint: 2026-09-21. This is a local implementation record, not release acceptance.

## Resume here

- Checkout: `/mnt/FastStorage/SSD Games - Non Steam/cena-despana-m4`
- Branch: `feat/despana-m4`; base `804068238c91a4a4377b3ed7996dac9188ca88b7`.
- Author decisions: [M4 frontend](23-m4-frontend.md).
- Work contract: [implementation plan](m4-despana-implementation.md).
- User authorized local clones, subagent implementation, documentation and eventual Sophia mining.
- The initial handoff was offline-only. A subsequent coauthor/operator-present
  Calvix smoke completed; scope is recorded below. No installed application replacement.
- ATARI authorized publishing this branch as a draft PR to `Nisugi/cena` on
  2026-09-21. Draft publication is not M4 acceptance or merge approval.

## Lanes

| Owner | Scope | Status |
| --- | --- | --- |
| Astra | Interface decisions, manifests, CLI integration, review and verification | Integrated locally; final architecture suite, clippy and formatting pass |
| `m4_session_observation` | Native coherent late subscription and generation-pinned manual input | Split to `feat/session-observation`; 12 observation tests pass |
| `m4_mobile_gate` | Android/iOS CI and verification record | Split to `ci/mobile-core-builds`; both targets passed on original draft |
| `m4_ui_model` | Pure presentation types and styled line assembly | Implemented; 17 tests passing |
| `m4_web_server` | Authenticated transport and bounded projection pump | Implemented; 11 tests passing |
| `m4_browser` | Despana surface and browser contract tests | 16 contract tests and actual-browser refresh/same-tab re-pair smoke pass |
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
- Android and iOS jobs passed at `153b25c` in
  [run 35581947042](https://github.com/Nisugi/cena/actions/runs/35581947042).
  They now belong to the independent mobile branch; see its
  [verification record](https://github.com/Nisugi/cena/blob/ci/mobile-core-builds/plan/m4-mobile-verification.md).
  The extracted revision requires its own run; local mobile toolchains remain unavailable.
- Native observation and existing regression tests pass (independent reviewer: 47 tests).
- Full workspace run with loopback permitted: **1,821 passed, 1 failed, 3 ignored**,
  across 182 result summaries. The subsequently added fourth native/WebSocket test also passes;
  all four pass together in the final targeted run. No live service was used.
- The single failure in that historical workspace run is `character_store::the_name_matches_regardless_of_case`.
  Reproduced with the same pinned toolchain on an untouched worktree of starting commit
  `804068238c91a4a4377b3ed7996dac9188ca88b7`; this change leaves character-store code untouched.
- Nisugi's supplied review reports an independent 182-suite green run at `153b25c`
  and identifies a shared-temp-directory race. Our review-response run still fails
  the case test even alone with `--exact --test-threads=1` on Linux; a concurrent
  run is therefore not an established explanation for all failures. Report both
  observations without claiming the suite is universally green; leave storage
  investigation/fixes to separate work.
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

## Review response and live smoke

- Native prerequisite is independently reviewable on `feat/session-observation`;
  this frontend branch includes it and is stacked on that branch. Mobile CI was
  removed from this diff and extracted to `ci/mobile-core-builds`. No force push.
  Review independently: [session #2](https://github.com/Nisugi/cena/pull/2),
  [mobile CI #3](https://github.com/Nisugi/cena/pull/3),
  [frontend draft #1](https://github.com/Nisugi/cena/pull/1).
- P1 reproduced in a real browser: refresh loses the token; reopening the pairing
  link in the same document never re-mounted. A hashchange handler now consumes
  and strips the token, retires the previous socket/timer, and reconnects. A fresh
  snapshot is required; uncertain commands are never replayed. Regression is
  green, with a dedicated browser CI job in addition to the Node contract suite.
- Documented stderr secret exposure, pre-run legacy subscriptions, and retryable
  versus terminal observation failures. The five-second wait is a caller-budget
  policy pinned by virtual time; actor-future boxing has a reproducible storage
  probe (not a performance or crash-isolation claim).
- Fresh local verification: 12 observation, 17 UI, 11 web, four native/WebSocket
  integration and 16 JavaScript tests
  pass, plus architecture checks and actual-browser fixture smoke. Session
  lib/tests with no-fail-fast finish with only the unchanged case test failing.
- Original Windows CI failed in the unchanged
  `status_and_clock::a_real_roundtime_is_in_effect_and_then_is_not` assertion
  (server clock advanced by one second); its test command stopped before the
  later frontend integration suites. Original Linux CI stopped on baseline
  Rustdoc errors. Neither is described as a green desktop run.
- Earlier live Calvix smoke: manual LOOK and inventory, hand swaps, viewer
  reattachment, and roundtime display worked. QUIT received EOF/acknowledgment
  and lifecycle Closed; terminal exit was normal session completion. No live
  game-network reconnect test or live re-pair-fix retest is claimed. Private
  account data and logs remain local, untracked, and excluded from publication.
- Follow-up visual polish, not this fix: inventory stream separation, duplicate
  announcement presentation, and less technical command receipt wording.

## Remaining acceptance / handoff

- See [operator handoff](m4-despana-handoff.md) for scope and launch/testing commands.
- Broader live acceptance and mobile device tests remain undone. Fresh CI for the
  split branches and updated frontend must be assessed separately from prior runs.
- The baseline test and Rustdoc failures need separate upstream attention before claiming
  an entirely green workspace. No unrelated storage or model fixes were folded into M4.
- Draft updates are authorized; no additional deployment, login or profile migration is part of this response.
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
