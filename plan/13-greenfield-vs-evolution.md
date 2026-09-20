# Greenfield vs. Evolving Vellum

Written 2026-09-18 in response to review finding #1; **revised the same day** after the author
supplied the reasoning the plan had never recorded.

**Decision: greenfield, as originally intended** — Cena is a new codebase applying what Vellum
taught, with an obligation to port aggressively wherever the knowledge lives in the code (§4).

This is the largest cost driver in the project. The reasoning existed; it had simply never been
written down, so this document records it and the alternative it was weighed against.

---

## 1. The argument that existed but was never written down

> **CORRECTED 2026-09-18, after the author supplied the actual reasoning.** An earlier draft of
> this document said the greenfield choice "was never argued." That was wrong in an important
> way: **it was argued, just never written down**, and the argument is not the one I
> reconstructed.

**VellumFE was a learning process.** The intent was never "build a client, then build another
one." It was: build a client to *find out what building a client actually requires*, then
apply that understanding from the ground up. On that framing, greenfield is not "we ignored
reuse" — it is **"the second attempt benefits from knowing, at line one, what the first
attempt spent 325K lines discovering."**

**And there is precedent, from the same author, that this works.** eohunter is exactly this
pattern: bigshot and forge, rebuilt with the understanding they produced. It is ~18K lines
that consumes core rather than duplicating it, with a written self-audit — measurably better
organized than what it replaced. The pattern has shipped once already.

This is evidence a LOC comparison cannot see, and my original draft weighed only LOC.

## 2. What is genuinely new vs. what is rebuild

Measured against `reference/VellumFE` (324,605 lines of Rust, 2026-09-17):

**Genuinely new — no Vellum equivalent:**

| | Why new |
|---|---|
| Multi-session in one process | Vellum is single-session; this is the headline feature |
| Session-as-actor + lifecycle (`12` §5) | new |
| Command authority, queue, arbitration (`12` §4) | new |
| Typed frames as an **emit** vocabulary | partially new — `vellumTimer`/`vellumCmd`/`vellumImg` already exist |
| Curated behaviors (Hunt, Loot, Heal, Bounty, Travel) | new — this is what Lich does today |
| Agent protocol | new |
| EAccess login | new to Vellum (it connects via Lich or direct), specified in `10` |

**Rebuild of things Vellum already has, working:**

| Subsystem | LOC | Note |
|---|---:|---|
| `src/config/` | 32,099 | the customization surface `09` §2.1 flagged |
| `core/travel/` | 9,308 | |
| `src/parser/` | 7,857 | 4,393 of it tests |
| `core/pathing/` | 5,507 | |
| `src/launcher/` | 3,522 | |
| `core/highlight_engine.rs` | 2,260 | `11` says this is the primary combat readout |
| `network.rs` | 1,476 | |
| `core/mapdb/` | 1,015 | |
| `src/theme/`, `src/tts/` | 1,763 | |
| **Subtotal** | **~65K** | before counting the three frontends (~170K) |

**The roadmap currently rebuilds all of it**, and does not deliver a client anyone would use
until roughly Milestone 8.

## 3. The case for evolution — stated fairly

Two of the plan's own rules point toward evolving Vellum rather than rebuilding, and they
deserve to be recorded properly rather than waved past:

- **`05` §−1 rule 0.4 — prefer deleting to adding.** Greenfield adds ~235K lines of
  reimplementation to reach parity with something that already runs.
- **`04` §6.6** recommends **strangler-fig migration behind characterization suites** as "a
  method worth copying wholesale" — from Vellum's own history, where the author hand-split a
  9,227-line impl behind tests rather than rewriting.

And a third, from the review: **the corpus makes the safety net real.** 10,849 logs / ~50 GB
means any extraction can be characterization-tested against two years of real wire traffic
before and after. That is precisely the condition under which strangler-fig is safe.

## 3a. What the LOC comparison misses

§2 and §3 measure *rebuild cost*. They do not measure the thing that actually decides this:
**inherited structure carries inherited assumptions.**

The concrete one, and it is not small:

> **`AppCore` (~26,825 lines across 29 files) assumes a single session.** Multi-session is
> Cena's headline feature. Extracting it from a codebase organized around its absence is not
> a refactor with a known end — it is open-ended, and it costs *continuously*, because every
> later decision bends around a shape chosen for a different problem.

The same applies in miniature to the config system (32K lines, single-character scoping, where
`12` §6a.2 now requires global→profile→character inheritance) and to the frontend layer, whose
`Frontend` trait `06` already found to be near-fictional.

**A learning rewrite prices this correctly and a cost-of-rebuild comparison does not**, because
the saving is invisible: it is all the wrong turns the second attempt does not take.

## 4. The decision

**Greenfield, as originally intended — with two obligations.**

> **REVISED 2026-09-18.** An earlier version of this document concluded "evolve Vellum in
> place," reached by comparing ~65K lines of reusable infrastructure against the cost of
> restructuring. That comparison was real but incomplete: it priced what greenfield *rebuilds*
> and not what greenfield *avoids* (§3a), and it did not know the learning-rewrite intent
> (§1). Corrected.

Cena is a new codebase that **applies what Vellum taught**, the way eohunter applied what
bigshot and forge taught.

### Two obligations, so this stays a learning rewrite and not a fresh start

**(a) Port aggressively where the knowledge is in the code, not in the author's head.**
Greenfield is about *structure*, not about retyping solved problems. Specifically:

| Port | Why |
|---|---|
| `ParsedElement` (61 variants) + `KNOWN_WIRE_TAGS` (~130) | years of discovering what the wire contains; irreplaceable |
| The parser and its 4,393 lines of tests | the single most valuable inherited asset |
| `parser_edge_cases.xml` and the fixture corpus | every entry is a production bug someone found |
| Crit tables, creature templates | static data; ships as data files |
| ~~spell data~~ | **NOT static.** See §4a.1 |
| mapdb / pathing / travel algorithms | solved problems, portable as algorithms |

### 4a.1 `effect-list.xml` is not a data file, and this row was wrong

**MEASURED 2026-09-20** while scoping the `spell.rb` port. The table above
listed "spell data" beside the crit tables as *"static data; ships as data
files"*. It is not static. `effect-list.xml` (230 KB, 517 spells, the file
`Spell.load` downloads from the EO scripts repo) stores its durations and
bonuses as **embedded Ruby expressions**:

```xml
<duration cast-type='self'>(Spell[101].known? ? 120 : 20) + Spells.minorspiritual</duration>
<bonus type='physical-as'>0-(20+((Spells.minorspiritual-2)/2))</bonus>
```

| Field | Values | Plain integers | Expressions |
|---|---|---|---|
| `duration` | 340 | 163 | **177 (52%)** |
| `bonus` | 232 | 103 | **129 (55%)** |
| `cost` | 395 | 353 | 42 (10%) |

The constructs, counted across all 306 expressions:

```text
 189  Spells.<skill> lookup          82  Spell[n].known? / .active?
 123  array .max/.min                45  Stats.<x>
  84  ternary                        20  if/end block
   5  reget -- reads the SCROLLBACK
```

The last one is the decisive one. Five durations call `reget`, walk the
scrollback backwards, regex-match a `CS: +N - TD: +N` line and do arithmetic
on the capture, to derive how long a spell landed for. That is not data with a
formula in it; it is a program that reads the client's own output buffer.

**Porting it faithfully would require evaluating Ruby**, which
`CLAUDE.md`'s first settled decision forbids: *"No embedded scripting
language. No Lua, no Luau, no Rhai, no DSL."* Three options exist and none is
free:

1. **Take the plain values only** (48% of durations, 45% of bonuses) and treat
   the rest as unknown. Honest, and leaves a spell model that cannot answer
   "how long will this last" for half the spell list.
2. **Re-express the formulas in Rust.** ~306 expressions, each needing a
   reading of what the Ruby meant, against no oracle -- and the `reget` five
   have no Rust equivalent at all, since a model crate has no scrollback.
3. **Derive durations from the wire.** `<dialogData id='Active Spells'>`
   carries a live countdown per spell, which is the *observed* duration rather
   than the *predicted* one. Sufficient for "is it about to drop"; useless for
   "should I cast it".

Option 3 is already implemented -- `Effects` reads exactly that feed -- so the
practical answer is that **Cena does not need most of this file**, and the
port is deferred until a behavior needs prediction rather than observation.
Recorded here so the "spell data is static" claim does not send the next
reader down the same path.

**This is not a defect in Lich.** Its spell model is a scripting engine and
these expressions run in it natively, which is exactly why the file has this
shape. It is a place where the two architectures genuinely diverge, and
`plan/12` §9d's rule applies: do not build the abstraction that would make it
portable.

**This is not a contradiction of greenfield.** The `Frame` vocabulary should be *derived from*
`ParsedElement`, not reinvented — designing my own would mean rediscovering the same 61
variants badly (`01` §2).

**(b) Answer finding #7 — the golden-file rule.** Porting the parser into a new `Frame` enum
invalidates Vellum's pinned snapshots, which `06` §1.2 forbids regenerating as discipline.
**Resolution:** regenerate once, deliberately, as an explicit act at the port, with the diff
reviewed — then the new goldens are pinned and the rule applies from there. Better still,
Cena's first golden run is against the **50 GB log corpus**, which is stronger evidence than
Vellum's 55 fixtures.

### What this costs, and how it is bounded

- **Reality: ~235K lines to reach parity.** The three frontends and the 32K config surface are
  the bulk, and they are the reason `12` §8 does not deliver a fully-featured client until
  ~M8.
- **No dogfooding until then** — the genuine cost of this route. **Mitigation:** Vellum still
  exists and still works. It is the daily driver until Cena overtakes it, which is exactly how
  eohunter and bigshot coexisted.
- **The risk is scope, not feasibility.** `05` §−1 rule 0.1 is the guard: build the simplest
  thing that works. Cena does not need parity with Vellum to be worth running — it needs
  multi-session, which Vellum cannot do at all.

### The evolution route, and why it was rejected

Converting Vellum to a workspace and extracting crates was seriously considered and is the
strongest alternative. Its real advantages: a working client from day one, ~65K lines of
infrastructure retained, and Vellum's golden file staying valid.

**Rejected because** §3a's cost is open-ended and compounds, while greenfield's cost is large
but *bounded and known*. A single-session `AppCore` is not a module to extract — it is an
assumption distributed across 29 files, and the feature that has to displace it is the one the
project exists for.

**Kept from that route:** its discipline. Extract and port aggressively (§4a), characterize
against the corpus before and after, and never rewrite what is already knowledge-in-code.

### What changes in the plan

Less than the reversal might suggest — `12` remains authoritative for *what* is built:

| | Status |
|---|---|
| Repo | new, as originally planned |
| Milestone order | unchanged (`12` §8) |
| Parser | **ported**, not rewritten; goldens regenerated once, deliberately, then pinned |
| Frames | derived from `ParsedElement`, not designed fresh |
| Frontends | rebuilt, informed by `06`'s finding that the trait is near-fictional |
| Customization | rebuilt with global→profile→character from the first setting (`12` §6a.2) |
| Dogfooding | **Vellum remains the daily driver** until Cena overtakes it |

## 5. What would reverse this

- Workspace conversion reveals coupling so pervasive that extraction costs more than rewriting
  (**measurable in step 1** — if it does not converge in a week or two, stop and reconsider).
- Multi-session proves to require a data model so different that `AppCore` cannot be scoped.
  *That* is the genuine greenfield argument, and it should be tested in step 4 rather than
  assumed now.

**Either way the test is empirical and early**, which is the point.
