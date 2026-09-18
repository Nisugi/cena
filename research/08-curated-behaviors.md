# Decision: Curated Behaviors in Rust, Configured by Data Profiles

Decided 2026-09-18. **This supersedes [`03-lua-dialect.md`](03-lua-dialect.md) and
substantially revises [`07-agent-interface.md`](07-agent-interface.md).**

---

## The decision

**Cena has no embedded scripting language.** Automation ships as **curated behaviors
written in Rust**, compiled into the binary, configured by users through **data profiles**
(YAML/TOML). Users do not author scripts.

The behaviors are Cena's versions of what the community scripts do today — a hunting
engine, a looter, a herb/healing manager, a bounty runner — rebuilt as first-class
**actions** in the binary rather than as scripts on top of it.

| | Before (Lua) | Now (curated Rust) |
|---|---|---|
| Automation lives in | Lua scripts over an FFI boundary | Rust behaviors in the binary |
| Users customize by | writing scripts | editing profiles |
| Language decision | Luau vs LuaJIT vs PUC vs Rhai | **none — it's Rust** |
| Safety model | sandbox, preemption, capability grants | ordinary code review |
| API surface | must serve 474 unknown scripts [^corpus] | must serve ~10 known behaviors |

[^corpus]: VERIFIED. 251 `.lic` in `reference/scripts/` + 223 in `reference/dr-scripts/`
    (`find … -name '*.lic' \| wc -l`, 2026-09-17). The census
    (`inventory/07-script-corpus-api-census.md`) analyses 456 of them as "live" after
    excluding spec/test files.

---

## Why this is the right call

### 1. It deletes the single riskiest subsystem

The plan called the coroutine scheduler *"L5's hardest component"* (`01-architecture.md` §5).
The dialect research found that mlua's async — Cena's central concurrency mechanism — is
**the exact thing someone forked the binding to fix** (`mluau`, created because *"mlua's
async implementation is prone to freezes and deadlocks"*).[^mluau] Luau added ~118K lines of
C++ with documented[^luau]

[^mluau]: INFERRED from the ecosystem research angle (`03-lua-dialect.md`, ecosystem
    researcher), quoting the `mluau` fork's stated rationale. **Not independently verified by
    me.** Would be settled by reading the fork's README/issues directly. The decision below
    does not rest on it alone.

[^luau]: UNVERIFIED as an exact figure — sourced from the runtimes research angle comparing
    Luau's C++ tree to Lua 5.1's ~14K. The *order of magnitude* is what the argument uses;
    the precise count is not load-bearing.
cross-compilation friction on aarch64/iOS.

All of that is now gone. Not mitigated — **gone**. Behaviors are async Rust functions on
tokio, which is a solved problem with no FFI, no guest GC, no yield-inside-metamethod rule,
and no toolchain risk on mobile.

### 2. The safety machinery was defending against a threat that no longer exists

Preemption of runaway scripts, per-VM memory budgets, sandbox escape tests, capability
grants, `catch_unwind` around guest code, `lua_Alloc` budgets — every one of those exists
because *untrusted code* runs in-process. Curated behaviors are code we wrote and reviewed,
exactly like the parser and the network layer.

Interrogation correction **C14** demanded a full runaway-containment design with explicitly
stated limits (`MASKCOUNT` doesn't preempt blocking FFI, `catch_unwind` is defeated by
`panic=abort`). **That entire correction is now moot.**

### 3. The API stops being a product and becomes an internal interface

The census's Tier 0/1/2 ranking existed to predict what 474 unknown scripts would need. We
no longer need to predict — **we know every caller, because we wrote them.** An API that is
wrong gets changed, and the compiler finds every call site.

This inverts the hardest constraint in the project. Interrogation correction **C20** spent
its length on which of 208 Lich globals[^globals] to keep, prune, or rename for script
authors. Now

[^globals]: VERIFIED by the `dig:globals` interrogation against
    `reference/lich-5/lib/global_defs.rb` (2,370 lines). The same dig found **64 of the 208
    have zero callers anywhere in the 456-file corpus** — itself a finding that only survives
    because it was measured rather than assumed.
the answer is: expose what our behaviors need, name it well, change it freely.

### 4. Types all the way down

No marshalling across a boundary. A `Frame` is a `Frame` from the socket to the behavior
that consumes it. The refusal enum (`Transient`/`Permanent`/`Roundtime`/`Stunned`/`Webbed`/
`Dead`) is exhaustively matched by the compiler, not by a Lua `if` chain. Interrogation
correction **C19** — that Seam B needed a Lua-aware architecture test because string keys
from Lua are invisible to a symbol grep — **is moot**; Rust symbols are greppable and, more
importantly, the crate graph enforces the seam directly.

### 5. Performance, unambiguously

The hot path measured in
[`../inventory/09-hot-path-measurements.md`](../inventory/09-hot-path-measurements.md) —
~2,395 crit patterns and 1,082 combat-def regexes per line, per session — never crosses a
language boundary. Behaviors and matching are the same language. On mobile this is the
difference between comfortable and marginal.

---

## What it costs — stated honestly

### 1. No community ecosystem

**This is the real trade, and it is a product decision, not a technical one.** Lich has 474
community scripts because anyone could write one. Under curation, a user's niche workflow
exists only if it is on the roadmap.

Mitigations that preserve most of the value:
- **Profiles must be genuinely expressive.** The measure: could a bigshot/eohunter profile's
  worth of customization be expressed? Those profiles already encode hunting rooms, target
  lists, flee rules, prep commands, loot policy, group roles. That is a lot of behavior
  without a line of code.
- **The agent interface (§ below) is the pressure valve.** An LLM can compose novel behavior
  from existing actions without anyone writing a script.

### 2. Curators are the bottleneck

Every new strategy and game update routes through whoever writes behaviors. Lich survives
game changes partly *because* a script author can patch same-day. Cena's answer is a release
cadence plus data-driven behaviors — a new creature or crit pattern should be a **data file
change, not a code change**. That is why the data/code split matters
(`inventory/02` §"Data vs code"): crit tables, creature templates and spell data ship as
data and can be updated without a rebuild.

### 3. Rust is slower to iterate than a script

A behavior change means a rebuild and a release, where Lich edits a `.lic` and reloads. Real
cost. Partly offset by profiles (tuning needs no rebuild) and by hot-reloadable data files.

### 4. We lose the option value of an embedded language

If curation proves too restrictive, adding a language later is possible but not free — §"If
we reverse this" below keeps that door honestly open.

---

## What survives from the previous work

Most of it, because the hard thinking was never really about Lua:

| Still stands | Why |
|---|---|
| **The layer stack** (`01` §1) | unchanged; `cena-script` becomes `cena-behavior` |
| **Parse-first to typed frames** (`01` §0.1) | stronger now — no marshalling at all |
| **Session as in-process actor** (`01` §0.2) | unchanged |
| **Serialized command queue** (`01` §0.3) | unchanged and simpler; behaviors are tokio tasks |
| **The send ladder in Rust** (`05` §7.1) | was always Rust; now behaviors call it directly |
| **Typed refusal classification** (`05` §7.2) | unchanged |
| **Arm-before-send** (`05` §6.5) | unchanged — a race, not a language artifact |
| **Three seams: read / observe / act** (`01` §3) | **unchanged as a discipline** — now enforced by crate boundaries and `&` borrows rather than by an API wall |
| **All architecture/testing rules** (`05`, `06`) | unchanged |
| **`fancy-regex` + multi-pattern dispatch** | unchanged; it was always Rust-side |

**The regex research survives intact** and is now cleaner: patterns were always going to be
compiled and matched in Rust. The only thing lost is the registration API for Lua.

**One irony worth noting:** eohunter's three seams were adopted partly because `World` gave
a testability seam Ruby couldn't otherwise provide — the interrogation found that shape was
a **Ruby-mocking artifact** (correction **C10**). In Rust with curated behaviors, the
discipline is kept and the ceremony is dropped, exactly as C10 recommended. The conclusion
arrived at from two independent directions.

---

## The agent interface, revised

[`07-agent-interface.md`](07-agent-interface.md) assumed an LLM would be "a script whose
decision function lives on a socket." With no scripting language, that framing changes — and
**the goal survives, in better shape**:

- The agent protocol (`observe` / `act` / `ask`) is now the **only** external automation
  surface, which makes it more important, not less.
- `act` submits intents that map to the same actions curated behaviors use. Same ladder,
  same queue, same typed results.
- **The agent becomes the answer to "no user scripts."** A user who wants novel behavior
  points an LLM at their session instead of writing Lua. That is a better answer for most
  people than learning a scripting language.
- Everything in `07` §4 (safety) and §5.1 (anything internal must be expressible over the
  protocol) still holds. Rule 5.1 is now the *primary* discipline keeping the action
  vocabulary honest, since there is no second consumer to keep it honest.

**Correction to `07`:** it says an agent gets "no capability a Lua script lacks." Restate as:
**no capability a curated behavior lacks.** The principle — one path to the game, reached
two ways — is unchanged.

---

## Behaviors to build

From the user: *"we would have our version of eohunter, eloot, eherbs, ebounty, etc. The
things those scripts accomplish would be built in as actions."*

Candidate first set, to be sized against the corpus census's usage data:

| Behavior | Replaces | Core capability |
|---|---|---|
| **Hunt** | eohunter / bigshot | target selection, engage, flee, rest, loadout, group |
| **Loot** | eloot | pick up, sort, stow, discard by policy |
| **Heal** | eherbs / ecleanse | wounds, herbs, cleanse, first aid |
| **Bounty** | ebounty | accept, route, complete, hand in |
| **Travel** | go2 | pathfinding, room graph, transit |
| **Sell / stow** | various | inventory management, merchants |

**Design rule:** a behavior is a **state machine over typed frames**, driven by a profile,
using only the three seams. If two behaviors need the same sub-capability, it becomes an
action, not a copy.

**Conformance target, restated:** the test is no longer "can the Lua API express eohunter?"
It is *"does Cena's Hunt behavior do what eohunter does?"* — which is more demanding and more
honest. eohunter's `core-consumption-audit.md` and its profile format remain the best
available specification of what a real hunting engine needs.

---

## If we reverse this

Curation is a product decision that could change. Keeping the door open costs almost
nothing if done deliberately:

1. **Keep behaviors behind a trait.** If behaviors implement a `Behavior` trait driven by
   frames and profiles, a future scripted behavior implements the same trait.
2. **Honor Rule 5.1** (`07` §5): anything a behavior can do must be expressible over the
   agent protocol. That protocol *is* a ready-made scripting API if one is ever wanted.
3. **Do not scatter game knowledge into behaviors.** The ladder, refusal classification and
   game model stay in the core, where any future consumer benefits.

Do those three and adding a language later is an additive project, not a rewrite.

**Tripwires that should prompt a revisit:**
- Profiles keep growing conditional logic (profiles turning into a language is the classic
  failure mode — watch for `if`/`when` keys creeping in).
- Users repeatedly want behavior the roadmap will not reach.
- The agent interface proves insufficient for novel automation.

---

## Status of superseded documents

- **[`03-lua-dialect.md`](03-lua-dialect.md)** — superseded. Kept for its regex research
  (§ on `fancy-regex` vs PCRE2 vs Oniguruma, and multi-pattern dispatch) which still governs,
  and as the record of why an embedded language was considered and set aside.
- **[`07-agent-interface.md`](07-agent-interface.md)** — amended as above; needs its "Lua
  script" references restated as "curated behavior."
- **[`01-architecture.md`](01-architecture.md)** — `cena-script` becomes `cena-behavior`;
  §5's coroutine/dialect discussion is replaced by "behaviors are async Rust tasks."
- **[`04-inherited-decisions.md`](04-inherited-decisions.md)** — corrections C14, C19, C20
  are moot; the rest stand.
