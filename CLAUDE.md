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
| `research/` | **rationale and evidence only. Never instructions.** Contains superseded designs. |
| `inventory/` | what the reference codebases contain, measured |

`research/` holds designs that were **reversed** — most importantly an embedded Lua runtime.
Do not implement from it. It exists so decisions can be audited, not repeated.

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
- **No embedded scripting language.** No Lua, no Luau, no Rhai, no DSL. Automation is
  **curated Rust behaviors** (Hunt, Loot, Heal, Bounty, Travel) configured by **data profiles**.
  Users do not author scripts.
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

## Next step

**Milestone 1**, narrowed (`12` §9c):

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

**Milestone 1 is complete, including its deferred tail. The next step is `12` §8's
Milestone 2** — frame vocabulary breadth, a golden corpus, and full room/combat/vitals
rendering — or whatever the author picks up instead.

The first live session that printed game text (2026-09-18) is evidence for exactly that
milestone: worn inventory arrived as `a` + `pebbled grey leather doublet` split at a link
boundary, spell lists and the services table came through as unstructured text, and 99 events
were dropped from the broadcast ring during the login burst.

## Credentials

Never commit credentials. A free-to-play test account exists; ask the author for it rather
than searching for it, and do not log into a live game service without the author present.
