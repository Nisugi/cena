# Cena

A private Rust game client for **GemStone IV**, by the author of VellumFE and eohunter.
It does what Lich-5 (Ruby scripting engine/proxy) and VellumFE (Rust client) do together, in
**one binary**, with **multi-session** as the headline feature.

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
  > Cena's own table is **123** — Vellum's 116 plus 7, dropping none. See `plan/15` §1.
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

Reconnect and desync move to Milestone 2.

**Milestone 1 is complete. The next step is Milestone 2** (`12` §9c: reconnect, desync,
criterion 9) — or whatever the author picks up instead.

## Credentials

Never commit credentials. A free-to-play test account exists; ask the author for it rather
than searching for it, and do not log into a live game service without the author present.
