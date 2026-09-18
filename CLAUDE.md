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
| `plan/13-greenfield-vs-evolution.md` | why this is a new codebase, not a Vellum fork |
| `research/` | **rationale and evidence only. Never instructions.** Contains superseded designs. |
| `inventory/` | what the reference codebases contain, measured |

`research/` holds designs that were **reversed** — most importantly an embedded Lua runtime.
Do not implement from it. It exists so decisions can be audited, not repeated.

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
  Specifically port: `ParsedElement` (61 variants), `KNOWN_WIRE_TAGS` (~130), the parser and
  its tests, `parser_edge_cases.xml`, crit tables and creature templates
  (`plan/13` §4a). Do **not** reinvent the frame vocabulary.
- **The test corpus is `E:\Gemstone\data\log archive`** — 10,849 XML logs, ~50 GB, Oct 2024 →
  Sep 2026, 50+ characters. Two years of real wire traffic. Use it for golden and replay tests.
- Reference clones are in `reference/` (gitignored): `lich-5`, `VellumFE`, `scripts`,
  `dr-scripts`, plus the Saga Discord thread. eohunter is at `C:\Gemstone\eohunter`.

## Next step

**Milestone 1**, narrowed (`12` §9c):

1. ~~**Step 0 — capture a Lich login with tcpdump.**~~ **DONE 2026-09-18.** S1/S2/S4 VERIFIED,
   S3 recorded unobservable-by-design (`plan/10` §12.1).
2. ~~**Step 1 — the login spike**~~ **DONE 2026-09-18.** `spike/eaccess-spike` reaches game text
   with XML mode enabled (`plan/10` §11).
3. **Step 2 — the slice:** one character logs in, shows a room, takes a manual command through
   the shared queue, runs one small behavior, stops reliably, disconnects cleanly, replays
   deterministically. **← current**

Reconnect and desync move to Milestone 2.

## Credentials

Never commit credentials. A free-to-play test account exists; ask the author for it rather
than searching for it, and do not log into a live game service without the author present.
