# Cena — working name for **Hydra**

A private Rust game client for **GemStone IV**, by the author of VellumFE and eohunter.
It does what Lich-5 (Ruby scripting engine/proxy) and VellumFE (Rust client) do together, in
**one binary**, with **multi-session** as the headline feature.

> **THE REAL NAME IS HYDRA** (author, 2026-09-18). `cena` is the **working name**: it is the
> repository directory, every crate prefix (`cena-session`, `cena-model`, …), the binary, and the
> `CENA_*` environment variables. All of that stays as-is for now — renaming a workspace mid-
> milestone is churn with no payoff, and the working name is load-bearing in **116 files across 8
> crates** (measured: `grep -rl cena --include=*.rs --include=*.toml --include=*.md`, excluding
> `reference/` and `target/`).
>
> What this does mean: **do not invent a third name**, and when something user-facing needs a
> product name — a window title, a log banner, a README, a release artifact — it is **Hydra**.
> The name fits the architecture, which is presumably the point: many heads, one body, and
> cutting one off does not kill it.
>
> The rename, if and when it happens, is a mechanical sweep of `cena` → `hydra` across crate
> names, paths and env vars. Worth doing at a milestone boundary, not inside one.

---

## Read this first

**[`plan/12-implementation-spec.md`](plan/12-implementation-spec.md) is authoritative.**
If anything else contradicts it, it wins.

| Read | For |
|---|---|
| `plan/12-implementation-spec.md` | **what gets built.** Start here, always. |
| `plan/05-engineering-rules.md` | how to write it. Conduct rules. |
| `plan/06-testing-strategy.md` | what to test and how |
| `plan/10-eaccess-spec.md` | the login protocol, reimplementable |
| `plan/15-wrayth-protocol.md` | **the game stream** — the XML protocol after login |
| `plan/13-greenfield-vs-evolution.md` | why this is a new codebase, not a Vellum fork |
| `plan/30-m6-hunt.md` | **the current milestone**: M6, the first real behavior, step by step with what is built |
| `plan/33-guard-vocabulary.md` | bigshot's 87 guard words evaluated; PROPOSED, awaiting the author |
| `plan/31-eloot-port.md` | **M6c**: eloot measured and ordered; Stages 1-3 (the hunt's share), skinning and Stages 4a-4c (the selling round during the rest: locksmith pool, gem shop, pawnshop, furrier, collectibles, Chronomage, bank) BUILT, with 4a's leftovers (the hands restored, the jeweler's refusals onward to the pawnshop, scrolls kept), and skinning's last gaps (bounty-only, the chimera, learned names saved) closed |
| `plan/34-loot-ledger.md` | the loottracker port: 55 patterns as one classifier over the chunk (Stage 1 BUILT, `cena-model/src/state/ledger.rs`), the ledger beside the combat recorder in one database per character (Stage 2 BUILT, `cena-session/src/ledger.rs`; `--record`/`--no-record`), `;loot`'s five reports (Stage 3 BUILT, `cena/src/loot.rs`), combat's first five on the same reader (Stage 4 BUILT, `;combat`, `cena/src/combat.rs`), the Red Forest uid bug explained and fixed. All four stages BUILT |
| `plan/35-m7-agent.md` | **M7, the agent protocol**: MCP on loopback, control levels, takeover, all statuses; PROPOSED, the author's 2026-09-24 decisions quoted, three questions open (§9). Prior art in `reference/lich-agent-bridge` |
| `crates/cena/src/architecture.rs` | the workspace as rustdoc: crate graph, one line's journey, the three seams, every rule and its test. Link-checked, so it cannot go stale silently |
| `crates/cena/src/glossary.rs` | **the words**: one per concept, each linked to its item, plus the ones that already mean two things (`claim`, `ladder`, `Desk`, …). Binding (`plan/05` §8) |
| `research/` | **rationale and evidence only. Never instructions.** Contains superseded designs. |
| `inventory/` | what the reference codebases contain, measured |

`research/` holds designs that were **reversed or deferred**. Do not implement from it. It
exists so decisions can be audited, not repeated — and, for the deferred ones, so the work
is not redone from scratch when they come back.

> **CORRECTED 2026-09-21.** This named an embedded Lua runtime as the chief *reversed*
> design. It is **deferred, not reversed** (author): *"Lua is on the table, just not now."*
> The distinction is not pedantic — it changes what a contributor may design toward. The
> error was live: a team proposing to build M4's frontend wrote that "optional Lua remains
> supported in the design", and this file would have had me tell them it was settled
> against. See the scripting entry under **Settled decisions**.

**One exception:** `reference/wiki_clean/Wrayth protocol.txt` is a copy of the official protocol wiki
(<https://gswiki.play.net/Wrayth_protocol>). It is a **primary source we implement from**, not a
superseded design. It is read through [`plan/15-wrayth-protocol.md`](plan/15-wrayth-protocol.md),
which cites it by line and records what it settles, what it contradicts, and what it leaves open.

> **PATH CORRECTED 2026-09-18.** This read `research/Wrayth protocol.txt`, which does not
> exist -- the file is under `reference/wiki_clean/`. The cost was not cosmetic: an agent
> searched the dead path, got zero hits, and concluded the wiki does not document
> `styleIfClosed`. It documents it five times. **A citation that resolves to nothing does not
> fail loudly; it manufactures a false negative.** Check that a cited path exists before
> reasoning from its silence.

---

## Settled decisions — do not reopen

- **One binary.** Not a proxy plus a frontend. (Mobile OSes suspend background processes; also
  over-determined by multi-session.)
- **No embedded scripting language _for now_.** Automation is **curated Rust behaviors**
  (Hunt, Loot, Heal, Bounty, Travel) configured by **data profiles**. Users do not author
  scripts in any milestone currently planned.

  > **CORRECTED 2026-09-21.** This read *"No Lua, no Luau, no Rhai, no DSL"* and sat under
  > a heading saying **do not reopen**, which made a deferral look like a closed door. The
  > author: *"Lua is on the table, just not now."*
  >
  > So the rule that binds is about **sequencing**, not prohibition: nothing being built
  > now may assume a scripting runtime, and no design should be shaped around one. But a
  > contributor proposing Lua later is raising a live question, not reopening a settled
  > one, and should not be told otherwise.
  >
  > What this does NOT license: adding a runtime, a `#[cfg]` for one, or an abstraction
  > whose only purpose is to host one. Rule −1's rule of three still applies, and `12` §9d
  > already refuses a `GameAdapter` built for a deferred DragonRealms on the same grounds.
- **Multi-session**, 3–25 characters in one process, session-as-actor.
- **Parse first.** Nothing above the protocol layer sees raw bytes or unparsed text.
- **One parser, N classifiers** (`plan/12` §3a). Exactly one thing turns bytes into
  structure; everything that recognises a game fact is a *stateless* classifier over its
  frames, and anything needing memory across lines is a stateful consumer above the model.
  A combat tracker is a classifier plus a consumer, **not** a second parser. The parser's
  side of the bargain is that every fact the markup encodes survives into the frames --
  verified against a real attack sequence, including `exist`/`noun` and bold depth.
- **Desktop-first development.** Mobile is a CI compile check, not a product commitment.
- **DragonRealms is deferred**, all-or-nothing. Do not add a `GameAdapter` abstraction for it
  (`12` §9d).

## The rules that outrank the others

From `plan/05-engineering-rules.md`:

1. **Evidence (§−2).** A finding without proof is speculation. Cite `file.rs:123`, give the
   command that produced a number, label claims VERIFIED / INFERRED / UNVERIFIED. Never
   restate a number from memory.
2. **KISS/DRY (§−1).** Build the simplest thing that works, then stop. No trait with one
   implementor. No config option with one value. **Rule of three** before abstracting.
   **Move code down, do not raise the cap.**
3. **A rule that is not enforced is a wish (§0).** Structural rules are compiler-enforced by
   the crate graph; the rest live in architecture tests, written *when the rule is adopted*.

## Commands

The toolchain is pinned in `rust-toolchain.toml` (**1.96.1**, with rustfmt and clippy);
CI and local runs use the same one. Clippy at `-D warnings` is part of the build contract,
not an optional lint pass.

```sh
cargo test --workspace                             # everything, including architecture tests
cargo test -p cena-model                           # one crate
cargo test -p cena-model --test stream_windows     # one integration file: crates/cena-model/tests/stream_windows.rs
cargo test -p cena-model -- speech                 # tests whose name contains "speech"
cargo test -p cena-arch-tests --test citations     # every path cited in plan/ still resolves
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo doc --workspace --no-deps                    # rustdoc link lints are DENY: a broken link is a red build
node crates/cena-web/browser-tests/smoke.mjs       # Despana browser smoke (CI job `browser-smoke`)
```

`/check` runs the full set and reports what failed. Use it before any commit, and after any
agent claims the tree is green. The other project commands: `/where` (milestone, HEAD,
uncommitted work, what is next), `/corpus` (query the log archive; ask the author first),
`/findings` (the research findings on disk) and `/agents`.

`cargo test -p X -- name` filters by **test name**, not file: an integration file whose
tests lack the word runs zero tests and reports green. Name the file with `--test`.

`missing_docs` is **deny** across the workspace, so every `pub` item needs a doc comment,
and `.github/workflows/docs.yml` publishes the rustdoc to GitHub Pages on every push to
`main`. Clippy's test exemption for `unwrap`/`expect`/`panic` covers `#[test]` bodies
only: a helper fn in `tests/` returns `Option`/`Result` and the test unwraps it.

CI (`.github/workflows/ci.yml`) also builds the five core crates (`cena-platform`,
`cena-protocol`, `cena-model`, `cena-session`, `cena-behavior`) for `aarch64-linux-android`
(via `cargo ndk`) and `aarch64-apple-ios` (macOS runner only). A dependency that breaks
either build breaks `plan/12` §1a.

**Do not run the binary.** `cargo run -p cena` logs into the live game in **every** mode:
`--character <name>` (repeatable; with none, it asks), `--web` and `--web-login`. Only the
author runs it (see Credentials). Run options are arguments, not env vars: an env var set
once in a shell drove the character on every later run. M1's `--demo` and the measurement
probes were removed at M6 (`plan/30` §2).
`CENA_MAP` points travel at a converted map file.

`spike/eaccess-spike` and `rtest/` are excluded from the workspace. The spike has its own
lockfile; run cargo inside it only for the spike.

## Architecture at a glance

| Crate | What it is | Depends on (cena crates) |
|---|---|---|
| `cena-platform` | the bottom layer: the pipe a session's bytes come through, and the log | — |
| `cena-protocol` | the wire layer: bytes in, `Frame`s out. The one parser, plus `tags.rs` | platform |
| `cena-model` | typed game state (`state/`) that a session folds frames into, plus game data | protocol |
| `cena-map` | the map's vocabulary (rooms, exits); `plan/21` | — |
| `cena-session` | one character's connection as one actor: socket, parser, state, reconnect | platform, protocol, model |
| `cena-behavior` | curated Rust behaviors (travel, sync, and hunt's profile, guards and importer) | map, session (platform dev-only) |
| `cena-ui` | pure, versioned projection of a session for frontends (`SessionView`, `WIRE.md`) | model |
| `cena-web` | Despana: embedded loopback viewer; the native session stays authoritative | ui, session |
| `cena-host` | the table of sessions one Hydra runs: add, remove, one per account, stop all (`plan/29`) | session (platform dev-only) |
| `cena` | the binary | behavior, host, platform, session, ui, web |
| `cena-arch-tests` | the rules the compiler cannot express | — |

(Measured from each crate's `Cargo.toml`, 2026-09-24:
`awk '/^\[/{s=$0} /^cena-/{print s, $1}' crates/*/Cargo.toml`. Re-measure; do not restate.
`crates/cena-arch-tests/tests/layering.rs`'s `ALLOWED_EDGES` asserts this graph as a set
equality, and `crates/cena/src/architecture.rs` is the long-form map with every item linked.)

> **CORRECTED 2026-09-24.** This table said `cena` depended on five crates and omitted
> `cena-host`, and listed `platform` as a real edge of `cena-behavior` and `cena-host`
> when both are dev-dependencies for the scripted game their tests talk to. It claimed to
> be measured. Where a table can be produced by a command, cite the command.

The flow is **bytes → `cena-protocol` `Frame`s → classifiers → `cena-model` `GameState` →
consumers**. `cena-ui` projects state to frontends, and `cena-web` serves the projection over
a local WebSocket.

**The architecture tests are the rulebook** (`crates/cena-arch-tests/tests/`): `layering.rs`
holds the allowed crate edges (`ALLOWED_EDGES`, the table that *is* the architecture);
`file_rules.rs` caps every source file at **800 lines** by default, with facade files capped
lower and explicit, justified exceptions in `CAP_EXCEPTIONS`; `citations.rs` checks that
every path cited in `plan/` resolves. When a file hits its cap, split it. **Moving code down
is the fix; raising the cap is not.**

## Working in this repo

- **Workspace of crates**, not one crate with modules. The dependency graph *is* the
  architecture: `platform → protocol → model → session → behavior/agent`, with `ui` and the
  frontends beside them. Dependencies point **one way, downward**.
- **Architecture tests exist before the code they govern.** That ratchet is the whole point —
  VellumFE had to retrofit it at ~250K lines.
- **When VellumFE implements something, read VellumFE's version FIRST** — before Lich, before the
  spec, before theorising. It is a *working* implementation against the same live servers, in the
  same language, by the same author. During the login spike this rule was broken three times and
  each time the answer was already in `network.rs`. Lich is the protocol's reference, but Vellum is
  the reference *port*, and the porting hazards (§10.3a of `plan/10`) live only in the port.
- **The protocol facts are already in Lich. Dig them out before asking.**
  > *"all of the protocol facts exist in lich, just have to dig them out."* — the author,
  > 2026-09-18

  Where Vellum is the reference *port*, Lich is the reference for what the wire **means**.
  On 2026-09-18, four status-channel facts were established by the author correcting a design
  in progress, and every one was already written down: the prompt-code/indicator split in
  `lib/constants.rb:72` (`ICONMAP`), the indicator-vs-text-derived split in
  `lib/gemstone/infomon/status.rb`, and indicator accumulation in `lib/common/xmlparser.rb:788`.
  Reading those three files first would have replaced four rounds of questions with one.

  Ask the author about what is **genuinely ambiguous** — game behaviour that no source records,
  or a judgement call about Cena. Do not ask them to recite what `reference/lich-5` already says.
- **Port aggressively where knowledge lives in code**, rewrite where structure matters.
  Specifically port: `ParsedElement` (**63** variants), `KNOWN_WIRE_TAGS` (**116**), the parser
  and its tests, `parser_edge_cases.xml`, crit tables and creature templates
  (`plan/13` §4a). Do **not** reinvent the frame vocabulary.

  > **CORRECTED 2026-09-18.** This said "61 variants" and "~130 tags". Both were wrong, and
  > were restated from memory rather than measured — the error `plan/05` §−2 exists to prevent.
  > Measured against `reference/VellumFE`:
  > ```
  > awk 'NR>37 && NR<405' src/parser.rs | grep -cE '^\s{4}[A-Z][A-Za-z]*'        -> 63
  > awk 'NR>=175 && NR<=192' src/parser/text.rs | grep -oE '"[^"]*"' | wc -l     -> 116
  > ```
  > Cena's own table is **126** — Vellum's 116 plus 10, dropping none. See `plan/15` §1, which
  > records the reconciliation.
  >
  > **CORRECTED AGAIN 2026-09-19.** This said 123, which was right when written and went stale
  > when the Saga reconciliation added three. `plan/15` §1 already said 126. Measured:
  > ```
  > grep -cE '^    "[^"]+",$' crates/cena-protocol/src/tags.rs                  -> 126
  > ```
  > The lesson is the one §−2 already teaches, in its second form: a number copied into a
  > second document is a number that will drift. Where a count can be measured by a command,
  > cite the command.
- **The test corpus is `E:\Gemstone\data\log archive`** — **10,849 `.xml`, 49.55 GB**,
  2024-10-11 → 2026-09-11, ~64 directories (**~35 distinct characters**, not 50+ — some dirs
  are the same character on two instances, and ~22 are organised by class, not character).
  Two years of real wire traffic; the tiebreaker when sources disagree.
  **Only `.xml` is wire data.** The 11,862 `.log` files beside them are a different,
  tag-stripped format — never use them as protocol evidence.
- Reference clones are in `reference/` (gitignored): `lich-5`, `VellumFE`, `scripts`,
  `dr-scripts`, plus the Saga Discord thread. eohunter is at `C:\Gemstone\eohunter`.

## Where the build stands

**M1, M2 and M3 are complete**, and **M4's code is merged** (PRs #1, #2, #3, all
2026-09-21): `cena-ui` and `cena-web` hold the Despana frontend, with a coherent
`SessionObserver`, generation-pinned manual input and a bounded projection pump.

**M5 -- multi-session -- was accepted live on 2026-09-24**, the author present, with two
characters on two accounts (`plan/29` §6 records the run). `cena-host` is the session
table; `cena --character A --character B --web` runs several characters, with the account
login from the OS keyring; one web listener serves a hub page -- a card per character,
start, quit, reconnect, and thoughts, speech, logons, deaths and announcements merged
across characters -- and each character's own page.

**M6 is under way on branch `m6-hunt`** (`plan/30` is the record; read its §7 for what is
built and what is next). **M6a, the foundations, is built as of 2026-09-24**: the M1
scaffolding is gone and there is one run path; the session prerequisites (attendance
counts a person, the authority survives a reconnect, preempt and the behavior watchdog);
the acting primitives (the stance setter, cast roundtime, the write-time `Gate`,
`travel_holding`); and step 4's profile format, inheritance chain and bigshot importer
(`cena-behavior/src/hunt/`, `;hunt import|check|list`). Nisugi's `ojandhaart.yaml`
imports whole. **Open for the author:** `plan/33`'s six questions on the guard vocabulary;
until a word is built, a step carrying it imports **held**, never silently lost.
**Next:** M6b, the engine (`plan/30` §3, §7).

> This section is headed by what is DONE rather than what is next, because that is
> what it has become: milestones of record with the next one named in a line.
> It was called "Next step" while it held one.
>
> **M4 IS NOT ACCEPTED, and the distinction is the point.** `plan/m4-despana-status.md`
> is careful about it and this file should be too: the code is merged and tested,
> and what remains is **evidence from a running game**. One operator-present smoke
> covered LOOK, inventory, hand swaps, viewer reattachment and roundtime. Not done:
> broader live acceptance, live reconnect through the frontend, and fresh CI on
> merged `main` — every recorded green run predates the merge.
>
> **The three baseline failures that doc records were real bugs, not noise** — all
> three are fixed as of 2026-09-22, and each had a symptom that hid it:
> the character store built filenames case-sensitively (one file on NTFS, two on
> ext4, so a Linux character silently got a second empty store); `send_now` pinned
> an exact server second to a value extrapolated from a `std::time::Instant`, which
> `start_paused` cannot control; and 42 rustdoc links were broken, two of them
> naming items that had never existed. Reported as a temp-directory race, a flaky
> test and lint noise respectively.
>
> **M6 is where the deferred work lands**, and it is tabled rather than remembered:
> `plan/20` §0b lists the sending halves of stash, bank, fog and spellsong; §0c's
> remaining gaps are `stance` as an enum and the `resource` capture. **The
> flee/arrival classifier it named as blocking overwatch is DONE** (`dbadaef`):
> the author's rule — *"leaving a room indicates a direction in the `<d>`. arriving
> in the room does not. hiding in the room does not"* — is read straight off the
> markup by `state/departure.rs`, MEASURED at 195 departures all carrying a
> direction link against 0 of 142 arrivals and 0 of 38 hides. `creatures.rs`'s
> `vanished_unaccounted` completes the inference.

### Milestone 1, narrowed (`12` §9c)

1. ~~**Step 0 — capture a Lich login with tcpdump.**~~ **DONE 2026-09-18.** S1/S2/S4 VERIFIED,
   S3 recorded unobservable-by-design (`plan/10` §12.1).
2. ~~**Step 1 — the login spike**~~ **DONE 2026-09-18.** `spike/eaccess-spike` reaches game text
   with XML mode enabled (`plan/10` §11).
3. ~~**Step 2 — the slice:**~~ **DONE 2026-09-18.** One character logs in, shows a room, takes a
   manual command through the shared queue, runs one small behavior, stops reliably, disconnects
   cleanly, replays deterministically.

   **Criteria 1–8 all met** (`12` §7.2). Criterion 1 was exercised **live, with the author
   present** — the one criterion no test in this workspace may run (`CLAUDE.md`, Credentials).
   Measured on that run: `stop` in **84µs** against a 250ms budget; room rendered from a
   `Frame::Component`, not scanned text; manual command interleaved and the behavior continued.
   Criteria 2–8 are test-backed; `plan/10` §11 records what the live run taught.

4. ~~**The deferred tail** — reconnect and criterion 9.~~ **DONE 2026-09-18.** `12` §9c moved
   these out of M1; they are now built and **live-verified**: an Ethernet drop mid-session
   produced `Closed -> Reconnecting -> generation 1 -> full re-login -> Ready -> room`, with
   §5.2's invalidated facts `Unknown` and the login burst re-teaching the rest. Also built:
   the backoff ladder (`[1,2,5,10,30]`s, ±20% jitter, ported from `VellumFE`), the two stops
   (fatal auth vs. `MAX_UNATTENDED_LOSSES`), `quit`-and-await-EOF (`16` §5b), and TCP
   keepalive on the game socket (Lich's `idle: 30 / interval: 30`).

> **A NAMING CORRECTION, 2026-09-18.** This work was called "Milestone 2" throughout, because
> `12` §9c says reconnect "moved to Milestone 2". **That is not §8's Milestone 2**, which is
> *"frame vocabulary breadth + golden corpus; full room/combat/vitals rendering"*. Two
> different things wear the same label and nobody reconciled them until the author asked
> whether there was a plan at all.
>
> What was built is **Milestone 1's deferred tail**. The milestone numbering that governs is
> **`12` §8's table**, and §9c's "Milestone 2" means only "not in the first slice".
>
> The cost of the confusion was a wrong recommendation: `move`/`travel` was suggested as the
> next thing, on the strength of having measurements for it. §8 puts the first real behavior
> at **M6**, four milestones out.

**Milestone 1 is complete, including its deferred tail.** `12` §8's **Milestone 2** —
frame vocabulary breadth, a golden corpus, and full room/combat/vitals rendering — is
**substantially done as of 2026-09-19**:

- **Model work** — `plan/18`'s six steps, complete.
- **Frame vocabulary breadth** — 126 tags; **zero unknown or malformed frames** across
  the whole golden corpus (267 frames), including traffic from a Lich era newer than any
  archive fixture.
- **The golden corpus** — 12 committed fixtures, 41.5 KB, each under the 10 KB budget.
  Six are M2's, cut 2026-09-19 from `E:\Gemstone\dev\lich-5\logs`; see
  `crates/cena-protocol/tests/FIXTURES.md` for provenance and
  `tests/golden_m2_corpus.rs` for the 17 tests.

> **THE CORPUS EARNS ITS KEEP IMMEDIATELY.** Cutting it found a real defect:
> `<crtrStatus>` typed correctly standing alone and degraded to
> `Frame::Structural { raw }` **inside a `<component>` body**, with every flag trapped
> in an unparsed string. MEASURED: **2,537 of 2,568 lines (98.8%)** put it inside a
> component, so the path that worked served 1.2% of real traffic.
>
> Two lessons, both already rules here. First, §3a's reopen signal fired exactly as
> written — a classifier would have had to re-tokenize markup — so the frame was widened
> rather than the consumer taught to re-parse. Second, **the first fix was too broad and
> the suite caught it**: routing every known tag through `thin_frame` turned
> `<nav rm=>` into `WindowHints`. The existing test was right and the change was wrong.
>
> Also: **two of my own assertions were corrected by the fixtures.** I claimed the combat
> exchange carried a `<roundTime>` tag and a `>` prompt. It carries the roundtime as prose,
> and the prompt is `HR>`. A golden cut from real traffic does not accept a claim about
> what the wire "should" contain.

~~**Remaining before M2 is closed.**~~ **M2 IS CLOSED as of 2026-09-21.** Both items
are done: `plan/15` §2b records the `crtrStatus` feed gaining health and two new flags,
and **PR-10's property coverage is built** — `parse_runs_reporting`, `inner_text` and
`directions` now carry properties in three `#[cfg(test)]` modules, verified by eight
mutations.

> **THE GAP WAS NOT "NO TESTS" — IT WAS TESTS THAT COULD NOT REACH THE CODE.**
> `tests/parser_never_panics.rs` was already thorough about panics, and that is
> why the hole was invisible: its `tagish()` generator emits **one tag at a time**
> and never a body between two tags. MEASURED before writing anything: the string
> `<dir` appears **zero times** in that file, so `directions` ran zero iterations
> under every one of its 2,048 cases; `inner_text`'s `rfind("</")` arm — the whole
> function — was never reached; and `parse_runs_reporting` was entered only through
> two unclosed `<component>` literals.
>
> The properties that earn this are the **Rule 2.2a conservation** pair on
> `parse_runs_reporting`: concatenated run text must equal the body's non-markup
> text, and every non-markup tag must be reported. That is a standing check on the
> invariant this crate exists to uphold, and it is the exact shape of the
> nested-link defect found by hand during M3.
>
> **Three of my own assertions were refuted by the tools, not by review**, which is
> the argument for both techniques in one session:
>
> | Claimed | Refuted by | The truth |
> |---|---|---|
> | `inner_text` never returns the opening tag | proptest, 8 cases | nested `<b>` bodies legitimately contain it; the invariant is positional |
> | no run's text contains `<` | proptest, 4 cases | `&gt;&lt;` decodes to `><` — text the game sent |
> | unmodelled tags are reported | **mutation MUT3** | the property only checked *reported* tags were well-formed, so swallowing all of them passed green |
>
> MUT3 is the important one: it is the **PR-1 defect itself**, and a property
> written to catch it passed the mutant. A claim about what is reported says
> nothing until the count is pinned to the input.
>
> A fourth landed in the same place from the other direction: a named test for
> `<dir/>` having no `value` **never reached the branch**, because the scan looks
> for `"<dir "` *with a trailing space*. The assertion was right and the input
> never arrived — the same failure as the four in M3, now five.

**Deferred, each needing an author decision:** SE-4 (authority across generations).
**SE-6 is RESOLVED** (2026-09-23): `SupervisedSession::observer()` plus
`SessionObserver::subscribe` recover from `Lagged` (`plan/19` records it). **MO-3 is FIXED** (M3 step 10): `Effects::active_in`
distinguishes "the game stated this list and your id is not in it" from "nobody has said".

**Milestone 3 — the typed character model — is COMPLETE as of 2026-09-21.**
`plan/20` is its record: the feature list is the author's, and every item is built or
deferred to M6 with a table of what remains. Eleven new modules
(`containers`, `resolve`, `hands`, `bank`, `fog`, `cooldowns`, `message`, `overwatch`,
`spells`, `spellsong`, `equality`), plus the 514-spell table cut from
`data/effect-list.xml`. MEASURED: **137 test suites green**, `state.rs` at **472 of 550**.

> **THREE PORTS SPLIT THE SAME WAY, AND THE SPLIT IS THE FINDING.** `stash.rb`,
> `bank.rb` and `fog.rb` each turn out to be two files wearing one name: a **reading**
> half that answers questions about the character, and a **sending** half that issues
> commands and waits. MEASURED for stash: 13 pure functions against 12 with 45
> send/wait/retry calls between them. The reading half is model work and is built; the
> sending half needs the authority token and roundtime, would put `fput` in a crate with
> no socket, and is M6. `plan/20` §0b tables what remains so it is recorded rather than
> forgotten.
>
> **Two Rule 2.2a losses were found by building on top, not by audit.** The hands
> dropped the `exist` id the wire sends (`Frame::LeftHand` preserved it;
> `GameState::apply` matched `{ item, .. }`), found only when `hand_holding(id)` could
> not be written. And a link nested inside a clickable `<d>` reached no consumer at all
> — MEASURED **322 occurrences across 62 of 208 live logs**, and **zero** in the
> committed fixtures, which is why the golden corpus did not catch it.
>
> **The author corrected four things no amount of testing would have caught**, each
> recorded at the point it bit: `Spell.active` is not `Effects` and still carries what
> never migrated; the spell `type=` tag is a closed vocabulary after all, because a
> one-time port of a near-static list does not need the defensive reading; cooldown kinds
> are two *cast mechanics* rather than two data sources; and "gone" does not mean "hid" —
> an inference I had implemented with two of its three conditions, and tested with a test
> that shared the same mistake.

> **A TEST CAN PASS A MUTATION BECAUSE THE INPUT NEVER REACHES THE CODE.** This happened
> four times in one session and is worth naming as its own failure mode, distinct from a
> weak assertion. The bank's indentation guard, the `effect-list` cooldown tests, the
> speech speaker rule, and the overwatch hiding inference each had a test that looked
> right, asserted the right thing, and **never exercised the distinction it claimed to
> test**. A green mutation run says *look at the input*, not just the assertion.
>
> The `effect-list` one is the sharpest: two copies of that file exist on this machine,
> differing by **exactly** the five `<cooldown>` elements under test. Cut from the older
> one, `with_cooldowns()` is empty and the invariant test passes over zero rows — a green
> suite reporting a feature that is not there. The right file was picked by luck; the
> extractor now records its source path and mtime, and a test asserts the five exist.

> **SOURCE AND DATA FILES AGE INDEPENDENTLY** (author, 2026-09-20): *"reference/lich-5
> should be pulled from upstream so it should be about the same as
> c:\gemstone\dev\lich-5, doesn't mean the data files are the newest."* A clone can be
> perfectly current and still hand you a stale table, because data ships with a release
> and source ships with a commit. MEASURED: the clone has **no `data/` directory at
> all**, so every data file in this port came from a live install. An extractor reading
> one records its source path and mtime in the output header; one reading the clone does
> not need to, because `git log` already says.

The first live session that printed game text (2026-09-18) is evidence for exactly that
milestone: worn inventory arrived as `a` + `pebbled grey leather doublet` split at a link
boundary, spell lists and the services table came through as unstructured text, and 99 events
were dropped from the broadcast ring during the login burst.

## Credentials

Never commit credentials. A free-to-play test account exists; ask the author for it rather
than searching for it, and do not log into a live game service without the author present.
