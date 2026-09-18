# Cena — Inherited Decisions Under Interrogation

Written 2026-09-17. Companion to `01-architecture.md`. Grounded in `E:\Cena\inventory\`
(nine documents), the lich-5 and VellumFE trees under `E:\Cena\reference\`, eohunter's
docs under `C:\Gemstone\eohunter\docs\`, and 5.5 years / 1,644 PRs of lich-5 GitHub history.

---

## What this document is

The nine inventory documents catalogue **what Lich-5 and VellumFE contain**. This one
does something different and harder.

For six inherited decisions, it asks:

1. **What actually forced this?** Era, language, team size, an external protocol, a bug
   they hit. Evidence over speculation.
2. **Does the constraint survive into Cena?** Rust not Ruby. One binary, not a proxy plus
   a frontend. Lua not Ruby. Mobile. Multi-session. No legacy corpus to keep working. And
   critically: **Cena owns both sides of the frontend wire**, where Lich must serve
   StormFront, Wrayth, Wizard, Genie, Profanity, Frostbite, Saga and Suks — eight
   frontends it did not write and cannot change (`lib/common/frontend.rb:21-29`).
3. **Was it right EVEN THEN**, given what they had?
4. **What should Cena do**, and does that differ from `01-architecture.md`?

### The fairness rule that governs the whole document

**They have to work with what they have.**

Lich's repo under elanthia-online was created 2021-03-09, and it is the **fourth**
custodian of a codebase whose scripting vocabulary predates it by fifteen years. Its
README states the chain: *"Lich was originally created by Shaelun, who brought it up from
an idea to version 3.57. Starting with version 3.58, Lich was maintained by Tillmen until
version 5. Starting with version 5 of Lich, a community effort started..."* It carried
Ruby 2.x/3.0-era code for years; it only recently moved to `REQUIRED_RUBY = '4.0'`
(`lib/version.rb:5`, verified). It serves hundreds of community scripts and eight
third-party frontends it cannot change.

VellumFE is essentially one person's work. eohunter is a script constrained by whatever
Lich core happens to offer.

**Judging a 2021 Ruby decision by 2026 Rust standards is worthless.** The useful judgement
is: *given their language, era, team and compatibility obligations, was this the right
call — and does the reason survive into Cena's very different situation?*

The most valuable verdict in this document is **"correct for them, wrong for us."** A
close second is **"right for anyone"** — decisions that encode real domain knowledge
which Cena should keep without re-deriving.

### Method note

Each decision was researched, then **adversarially judged** by a second pass that re-ran
the greps, re-read the cited lines, and re-fetched the PRs. Where judge and analyst
disagreed, this document says so and adjudicates. Several headline claims were corrected
or downgraded by that process; those corrections are folded in below and flagged.

Inference is labelled **INFERRED** with a confidence level. Nothing inferred is presented
as documented fact.

---

## The verdict table

| # | Decision | The real constraint | Still applies to Cena? | Right even then? | What Cena does |
|---|---|---|---|---|---|
| 1 | **Global-function script DSL** (`fput`, `echo`, `matchwait`) | Inherited Wizard/Genie script vocabulary + a live corpus of hundreds of scripts. Not a design choice — an obligation. | **No** — but for a different reason than the plan states | **Yes**, and calcified around it | Session handle `sess:put()`. **Keep the verb names.** Keep the pause checkpoint by other means. Drop `matchwait`/`goto`. |
| 2 | **Infomon as SQLite KV store** | A community script being absorbed into core; its hash shape came with it. Persistence for a 24h staleness window; a 15-command sync costing 30–60s. | **No** (KV), **yes** (persistence + sync cost) | **Yes-ish**, and it was retrofitted within five weeks | Typed struct; open maps only where the game's vocabulary is genuinely open; serde snapshot for durability. |
| 3 | **eohunter's three seams** (world/events/actions) | 18K lines of production hunting; real bugs encoded as rules. | **Partly** | **Yes** — best-designed thing in the tree | Adopt `actions` wholesale. Adopt `world`'s *rules*, not its *shape*. **Make `events` per-session.** Add a fourth seam. |
| 4 | **Per-line text hooks** (`DownstreamHook`) | Lich is a proxy: bytes forwarded to eight frontends, lossily transcoded one-way by `markup.rb`. There is no inverse. | **No** | **Yes** — the only correct option available | Parse first. **Split observe from filter.** Keep Vellum's unknown-tag design, not `Frame::Unknown`. |
| 5 | **Session = OS process** | Process-global character state: `@@right_hand` is a class variable. One character per process is the only arrangement that namespace permits. | **No** | **Yes, unambiguously** | In-process actors. But **design runaway containment in**, because the process boundary was doing real work. |
| 6 | **Vellum's AppCore + mechanical enforcement** | A 9,227-line `impl` block hit a wall; caps ratchet the hand-split that followed. | **Yes** | **Yes** — caps have never once been raised | Start with enforcement. **Add per-file line caps** — the one rule a crate graph cannot express. |

---

## Decision 1 — The global-function script DSL

**Still applies:** no · **Right even then:** yes, and calcified · **Changes the plan:** yes

### The decomposition that makes this tractable

This decision is usually discussed as one thing. It is three, and they get three different
verdicts:

| Layer | What it is | Right then? | Transfers? |
|---|---|---|---|
| **The verbs** (`fput`, `echo`, `waitfor`) | Inherited vocabulary, pre-dating the repo by ~15 years | Not a choice — an obligation | **Names yes, shape no** |
| **The implicit context** (`Script.current`) | Thread-identity lookup, because process = session | **Yes, obviously correct** | **No** |
| **The pause checkpoint** (inside `Script.current`) | Preemption over an uncooperative corpus | **Yes, and underrated** | **Requirement yes, mechanism no** |

### 1.1 It was inherited, not designed

The proof that this is Wizard-script emulation rather than Ruby design is still in the
source:

- `waitfor` branches on frontend script type: `if (script.is_a?(WizardScript)) and (strings.length == 1) and (strings.first.strip == '>')` — `lib/global_defs.rb:1408` (verified exact)
- `matchwait` with no arguments drives a label table and jumps: `script.match_stack_clear; goto jmp` — `global_defs.rb:1376-1396`
- `Script` carries `attr_accessor :jump_label, :current_label, :match_stack_labels, :match_stack_strings` — `lib/common/script.rb:371`
- There is a literal transpiler subclass, `class WizardScript < Script` — `script.rb:3459`, carrying a `# FIXME: when modernized` comment

`match` / `matchwait` / `goto label` **is** the Wizard `.cmd` scripting model. Top-level
bare verbs over ambient state is what that language *is*; you cannot emulate it with
objects.

**INFERRED, high confidence:** no PR states "we copied Wizard's verbs," but the
`WizardScript` class and the `waitfor` special-case are only explicable as compatibility
with that format.

**Judgement: not a design decision at all.** Judging it as one is the category error this
document exists to prevent.

### 1.2 The corpus pins the globals — in the authors' own words

The clearest statement of the live constraint is **PR #1346** (MahtraDR, merged
2026-05-07):

> *"`parse_args()` is called by dozens of DR scripts (tome, moonwatch, etc.) as a bare
> top-level method. Currently this bridge only exists in `dependency.lic:148` — core lich
> provides `Lich::Common::ArgParser` class but no top-level wrapper. If any script starts
> before dependency loads, it crashes: `--- Lich: error: undefined method 'parse_args'`"*

Two things matter. First, this is **2026 and they are still adding globals** — the
direction of travel is toward the DSL, not away. Second, **the module already existed**:
`parse_args` is a bridge to `Lich::Common::ArgParser`, the real implementation. The
architecture underneath is already modular; **the globals are a compatibility veneer over
it.** That is exactly the shape Cena wants — except Cena gets to skip the veneer.

The file's own header is self-aware to the point of resignation:
`# sadly adding global level script methods (2024-06-12)` (`global_defs.rb:4`).

The census confirms the pressure: `parse_args` is 146 calls across **144 distinct files**
(`07-script-corpus-api-census.md`). There was no other option available to them.

### 1.3 Implicit context was right, because the process WAS the session

- `class Game` holds the connection as class-level singleton state: `class << self; attr_reader :thread, :reader_thread, :server_queue, :buffer, ...` — `lib/games.rb:387`
- `put` is therefore `def put(*messages); messages.each { |message| Game.puts(message) }; end` — `global_defs.rb:1641-1643`. **There is no place to put a session argument.**
- `respond` writes the process-global client socket `$_CLIENT_.puts_main_stream(str)` — `global_defs.rb:1792`
- Multi-session is keyed by **pid** — `internal_api/active_sessions/registry.rb:30-42`

And stated outright in PR #1613: *"Each character keeps its own Ruby process."*

One Lich process = one character, always. Passing a session handle would have been **pure
ceremony with exactly one possible value.** Adding it would have been the wrong call.

**Judgement: obviously correct then.** Breaks only under Cena's new requirement.

### 1.4 The hidden jewel — and the correction that improves it

`Script.current` is not a getter (`script.rb:1101-1105`, verified verbatim):

```ruby
def Script.current
  script = __resolve_current
  script&.wait_while_paused!
  script
end
```

Documented as: *"Blocks the calling thread while that script is paused (unless it has
opted out via +ignore_pause+) before returning, so callers cannot observe or act past a
pause."* There is even a deliberately-named escape hatch — `Script.current_without_pause`,
*"Resolves the calling script for core seams that must not add a pause checkpoint."*
Someone thought carefully about this.

**The analyst claimed this made every global a preemption point, and that it is why both
`;pause` and `;kill` work on hostile scripts. The judge corrected both claims, and I
verified the corrections:**

- **Correction 1 (count).** `grep -c 'Script.current' lib/global_defs.rb` = **50**, out of
  208 functions. About a quarter of the surface, not all of it. The hot verbs do check, so
  the direction holds, but "every global is a preemption point" is false as written.
- **Correction 2 (material).** **Pause and kill are not the same mechanism.** `Script#kill`
  (`script.rb:1908`) and `Script.kill` (`script.rb:1479`) work via `Thread#kill`. Killing a
  runaway Ruby script needs no checkpoint — **the VM does it.** So the alarm "a
  `while true do end` script becomes unkillable in Cena" is a consequence of the plan's
  **coroutine decision** (`01-architecture.md:311`), not of deleting the globals. The
  recommendation survives; the causal chain was wrong, and it misfiled the gap under the
  wrong decision.
- **Correction 3 (better model, and it changes the advice).** Modern Lich's cancellation
  checkpoint **has already moved off the ambient lookup.** `check_execution_guard!`
  (`script.rb:3005-3008`, verified) appears 18 times in `script.rb`. `execution_sleep`
  (`script.rb:3017`) is documented: *"Native helpers use this instead of one long sleep so
  cancellation can unwind their ensure blocks."* There is a
  `ScriptExecutionGuard::Interrupted` exception. `wait_while_paused!` (`script.rb:2895-2912`)
  has **two branches** — a legacy `sleep 0.2 while paused?` poll and a modern
  `check_execution_guard!` loop.

**Adjudication: the judge is right and the finding gets better, not weaker.** Lich's
cancellation checkpoint lives **at the I/O wait, not at the ambient lookup**, and Lich is
actively migrating in that direction. That model is already coroutine-shaped and needs no
ambient context at all. So Cena should copy the **cancellation-token-at-every-yield-point**
design directly — it is the end state Lich is walking toward, and Cena can start there.

The `Script.current` pause block is better read as a **retrofit at a convenient chokepoint**:
pause had to work, `Script.current` was the one function everything already called, the
block went there. That is a competent move, not a designed one.

### 1.5 What Cena should do

**Do not expose a global-function DSL. Bind one `session` handle per script.** But:

**(a) Fix the plan's stated reason.** `01-architecture.md:559` argues *"Blocking globals
are structurally incompatible with multi-session. `fput` with no session argument is
meaningless when three characters are logged in."* **That reasoning is weak on its own
terms** — Lich's thread-binding trick would work in Cena too; a Lua coroutine can carry an
owner just as a Ruby thread does. Implicitness alone does not force the change.

The **real** argument: Lich's implicit context bottoms out in `Game`'s class-level socket
and `$_CLIENT_`, not in the thread — and Cena is deleting the process boundary that made
those singletons safe. Same conclusion, sound reasoning. Use this one.

**(b) Build the preemption checkpoint, and file it under the scheduler.** In `mlua` this is
a **debug hook / interrupt callback on the Lua state** (`LUA_MASKCOUNT`) checking a
per-script pause+kill flag, plus a cancellation token at every await point. Enforced by the
runtime, not by the API shape. `01-architecture.md` does not mention this anywhere; it
belongs in Phase 5 (`:505`), which the plan already calls *"L5's hardest component."*

**(c) Keep the verb names as methods on the handle.** `sess:put()`, `sess:echo()`,
`sess:match_wait()`. This costs nothing and preserves twenty-five years of muscle memory
for exactly the people Cena needs to port scripts. **The implicitness is the defect; the
vocabulary is domain knowledge.**

**(d) Drop `matchwait`/`goto`/`match_stack` entirely, and say so.** The census gives
permission: `matchwait` and `goto` are concentrated in a handful of legacy files
(`07-script-corpus-api-census.md:96,:100` summarises 5 and 3 files; the census prose at
`:760-764` counts 4 and 2 for the GemStone corpus — either way, near-dead). They require a
`jump_label` interpreter inside the script runner (`script.rb:3043-3056`). The census
already reached this conclusion: *"Porting those few scripts by hand is cheaper than
implementing a goto-label FSM in Lua"* (`:763-764`) — so this recommendation is
**confirmation, not novelty.**

`echo` (4,161 calls / 253 files), `fput` (3,234 / 216), `respond` (4,249 / 174) and
`pause` (745 / 177) are the live surface. Build those four properly.

**(e) Prune on the census, not on the file.** 64 of 208 globals are called **zero** times
by any script in either corpus. Another block is alias sediment — `global_defs.rb:2314` is
headed *"## Alias block from Lich (needs further cleanup)"*, and four aliases
(`stop_scripts`, `kill_scripts`, `kill_script`, `stop_script`) name one function. Real API
≈ 144 names; the top 30 cover the overwhelming majority of traffic.

**(f) Enforce the boundary by compilation.** `pub` vs `pub(crate)`. This is not Cena
inventing discipline — it is exactly what `Lich::API`/`InternalAPI` was reaching for and
could not enforce in Ruby, where the seal is defeated by `send` (`lifecycle.rb:280-282`).

---

## Decision 2 — Infomon as a SQLite-backed key-value store

**Still applies:** no · **Right even then:** yes-ish, and retrofitted fast · **Changes the plan:** no

### 2.1 What actually forced it: a script being absorbed, not a data model being designed

Issue #335 (mrhoribu, 2023-03-16), *"infomon library - add infomon library and convert
supporting modules"*, opens:

> *"Infomon.lic script should be integrated into core Lich. In order to do so, need to
> create a new infomon library for set/get/initialization of data as well as a parser to
> watch for various strings."*

`lib/gemstone/infomon.rb:3-4` still carries the tombstone: *"Replacement for the venerable
infomon.lic script used in Lich4 and Lich5 (03/01/23) / Supports Ruby 3.X builds"*, with
`contributors: Tillmen, Shaelun, Athias` — **the community script's authors, not the
library's.** The KV surface is **inherited from the script's existing hash.**

PR #325 (`feat: sqlite infomon`, ondreian, 2023-03-12, +2335/−582) has a one-line body —
*"migrates infomon to use an sqlite database"* — **zero review comments and zero issue
comments.** There is no recorded design debate. This was a maintainer merging his own
migration.

### 2.2 The three constraints that were real

1. **Persistence is genuinely load-bearing, for exactly one reason.**
   `lib/gemstone/experience.rb:99` has `def self.stale?(threshold: 24.hours)`;
   `lib/attributes/enhancive.rb:303` has `last_updated`. A **24-hour** staleness threshold
   is only meaningful if the value survives process restarts.

2. **Sync is expensive and noisy.** `Infomon.sync` (`lib/gemstone/infomon/cli.rb:20-37`)
   issues **15 blocking game commands** at `timeout: 5` each — `info full`, `skill`,
   `spell`, `experience`, `society`, `citizenship`, five `list all` PSM queries, `resource`,
   `warcry`, `profile full`. It must first terminate spell 1212 Shroud of Deception and
   warns `'ATTENTION: TEND TO YOUR SHROUD!'` afterward. **That is 30–60s of game traffic.**
   **Cena inherits this constraint unchanged.**

3. **Ruby 2.x/3.0 had no typed-record story.** No typed `Struct`, no exhaustive matching,
   no derive macros. The alternative was hundreds of hand-written `@@ivar` + accessor pairs.

**Also, and this strengthens the "right for them" half:** Sequel/SQLite was **already a
dependency** — `lich.db3` (Settings) predates `infomon.db`, and they are separate Sequel
handles (`infomon.rb:28`). The marginal cost of reusing an existing dependency in 2023 was
near zero.

### 2.3 Two hypotheses the evidence kills

**"It was for sharing between Lich processes."** No. The table name is per-character:
`("%s_%s" % [XMLData.game, XMLData.name]).to_sym` (`infomon.rb:84-87`). Two processes touch
**disjoint tables.** The multi-process DB work — PR #1319, the most detailed DB PR in the
repo — is about a *different file*, `lich.db3` (Settings). **Cross-process sharing is a
Settings problem, not an Infomon one.**

**"The open vocabulary lets scripts read state Lich's authors never anticipated."**
**Measurably false.** From `07-script-corpus-api-census.md:256, 268-271`:

| API | calls | files |
|---|---|---|
| `Stats.*` | 213 | 29 |
| `Spells.*` | 138 | 7 |
| `CMan.*` | 62 | 6 |
| **`Infomon.*`** | **6** | **2** |

> *"`Infomon` is called directly only 6 times (3× `Infomon.sync`, 2× `Infomon.db_refresh_needed?`, **1× `Infomon.get`**) — scripts consume Infomon's output through `Char`/`Stats`/`Skills`/`Spell`."*

Across **456 community scripts**, there is exactly **one** call that reads an arbitrary
Infomon key. **The extensibility was theoretical.**

And where the keyspace *is* dynamic, it is still closed: PSM ranks interpolate
(`infomon/parser.rb:439,447,454`), but `short_name` comes from hardcoded in-repo tables —
`psms/cman.rb` (84 entries), `ascension.rb` (80), `feat.rb` (39), `shield.rb` (39),
`weapon.rb` (30), `armor.rb` (16), `warcry.rb` (12). **New game content requires a code
change anyway.** The KV store buys nothing; it moves the table from the type system into
string concatenation.

### 2.4 What the KV encoding cost: four bugs in one three-line function

`Infomon._key` normalizes a key. Today it reads
`key.to_s.downcase.tr(' -', '_').gsub(/_+/, '_')` (`infomon.rb:131-133`, verified). Over
three years it produced:

- **PR #388** (mrhoribu, 2023-06-12): `.gsub('_-_', '')` ate the underscore in lore skills —
  `Infomon._key('sorcerous lore - necromancy')` → `sorcerous_lorenecromancy`.
- **PR #392** (gs4-Xanlin, 2023-06-13): *"SQLite naming conventions use alphanumeric and
  underscore for naming, using a dot notation requires quoting the name. Dots are for
  identifying objects... **Would be better to do this before shipping.**"*
  (**Correction from the judge: #392 was closed unmerged.** The concern was evidently
  addressed by other means; it should not be listed among shipped fixes.)
- **Issue #1215 / PR #1216** (jscholly, 2026-02-16) — the worst. Chained bang-methods:
  `.tr!` returns `nil` on no-match, so the third transform ran on `nil`. Infomon stored
  `skill.two-handed_weapons` while `Skills.two_handed_weapons` read `skill.two_handed_weapons`.
  The reporter:

  > *"I think it is giving '0' and not nil or "" is because of a `.to_i` tacked onto the
  > end of the Infomon.get in Skills.rb ... **'Two-Handed Weapons' is the only skill that
  > contains a hyphen that is not surrounded by spaces.**"*

  A character's two-handed weapon ranks read **0** — not an error, not nil, a
  plausible-looking zero. Any hunting script gating on that skill silently misbehaved.
- **Issue #586** (mrhoribu, 2024-05-24), a namespace collision: *"two PSM abilities share
  the same mnemonic... Weapon Charge (polearms) and Shield Charge. Possible fix is moving
  away from storing in infomon db as `psm.mnemonic` to `type.mnemonic`."*

**Every one of these is a compile error in Rust.** The KV encoding didn't cause bad code;
it removed the mechanism that would have caught it.

### 2.5 The retrofit that reveals the real end state — this is the transfer trap

**The version shipped in PR #325 had no cache.** Grepping the PR's own diff of
`lib/infomon/infomon.rb` for `cache|Cache|Queue|Thread` returns **zero hits** — only
`setup!`, `get`, `set`. `self.get(key)` was a synchronous `self.table[key: ...]` query per
read.

Within about **five weeks** of merge (created 2023-03-12, merged 2023-05-16; cache PRs
#395/#396 merged 2023-06-21), they bolted on an in-memory hash, an async write queue with
a barrier-token flush, a mutex, and `upsert_batch` (#383). PR #395 (ondreian):

> *"Continues Tysong's work to enable performant, async writes, but having reads be never
> stale"*

**The shipping Infomon is a RAM hash with a SQLite write-behind log.** The database is not
the data structure — **it is the durability tier.**

**Anyone reading today's `infomon.rb` and concluding "they chose a DB for in-memory state"
is reading the retrofit, not the decision.** Cena should copy the **end state** (RAM is the
model, disk is a snapshot), not the shape.

### 2.6 The natural experiment — and the judge's correction to it

The analyst's strongest claim: **the same repo, same maintainers, same era, same Ruby —
DragonRealms did the opposite.** `lib/dragonrealms/drinfomon/drstats.rb` is plain
`@@race`/`@@guild`/`@@strength` class variables with typed accessors; `drskill.rb` is a
real `DRSkill` class with `attr_accessor :rank, :exp, :percent`; and
`grep -rn "Sequel\|sqlite" lib/dragonrealms/drinfomon/` returns **nothing** (I re-ran this
— confirmed empty).

**Judge's correction, and it is right:** DR is a **materially smaller problem**, not the
same problem solved differently. DR's startup issues **6** commands at `timeout: 1`
(`drinfomon/startup.rb:53-78`) versus GS's **15** at `timeout: 5` — so DR never faced
constraint #2 at all, and re-syncs unconditionally every login because it is cheap enough
to. DR also has **no PSM layer** (`lib/gemstone/psms/` holds 7 tables, ~300 records, with
no DR equivalent), so it never faced the ~300 interpolated keys.

**Adjudication:** DR proves SQLite was **not forced by Ruby or by the era**. It does *not*
prove the GS choice was unjustified given GS's larger surface. Take the narrower claim.

### 2.7 What Cena should do

**This does not change `01-architecture.md`** — the plan already specifies a typed game
model (§3, Phase 3). It sharpens the design:

1. **Typed named fields** for the ~120 closed-vocabulary values (10 stats × 6 variants,
   45 skills, experience, society, citizenship, account). The game will not invent an
   eleventh stat without a client release.
2. **A typed-accessor-over-open-map hybrid** only where the vocabulary is genuinely open —
   the text-derived status conditions and PSM ranks. **Vellum already ships this pattern**
   at `src/core/state.rs:340-410`: a `BTreeMap<String, T>` with `set()/get()/is_known()`
   accepting any id the game invents, plus a macro generating typed getters over it. Its
   own comments (verified): *"Absent means 'never reported'"*, and `is_known()` exists
   because *"a multi-account display does [need this]"*.
3. **Keep `updated_at`** — but as a per-field or per-group `Option<SystemTime>` on the
   struct, not a DB column. `Experience::stale?` and `Enhancive::last_updated` are real.
4. **Keep a persistence layer**, but make it a **serde snapshot of the typed struct** (one
   file or row per character), not the storage model. The 15-command sync cost is real and
   **Cena will pay it too**; a typed snapshot avoids it just as well as a KV table, and
   survives schema evolution via `#[serde(default)]`.
5. **Do NOT expose a generic `state.get("some.key")` to Lua.** The census says nobody
   wanted it. Expose `world.stats.strength`, `world.skills.combat_maneuvers`; for the
   genuinely open sets expose an iterator (`world.status:all()`,
   `world.psm:rank(type, name)`) so a Lua script can still see an id Cena's authors never
   anticipated. That preserves the one real benefit of open vocabulary without
   stringly-typing the other 95%.

---

## Decision 3 — eohunter's three seams (world / events / actions)

**Still applies:** partly · **Right even then:** yes · **Changes the plan:** yes

**Verdict: two-thirds genuine insight, one-third Ruby scaffolding — and the plan's most-
quoted selling point for it (multi-session) is the one claim the evidence contradicts.**

### 3.1 Actions: correct then, correct now, correct for anyone

`actions.rb:6-17` states the contract: *"preconditions -> settle RT -> target still live ->
send through the refusal ladder -> confirmation ... **No bare fput-and-hope anywhere else.**"*

The decisive choice is the skipped/failed distinction, and the author documents the bug
that forced it (`actions.rb:129-140`):

> *"A gate that refuses returns :skipped, not :failed. Every one of them returns before
> `perform`, so **by construction the game heard nothing**: the action declined itself.
> :failed is reserved for a command the game refused or did not answer... Before this, a
> stun or an unaffordable technique read as five failures in five ticks and **stopped a
> live hunt in about a second with nothing on the wire**. ... bigshot's `bs_put` waits a
> stun out and carries on."*

(The last sentence is a **fourth independent implementation converging on the same rule** —
the analyst originally truncated the quote and lost that evidence for their own case.)

A genuine architectural insight: **the watchdog must count wire events, not decisions.**

**Independently validated by upstreaming.** lich-5 **PR #1587 (merged)**:

> *"`fput`'s refusal ladder has no bounds: a `...wait N` or 'struggle to stand' loop can
> resend forever, a transient refusal ends the send with a bare `false` the caller cannot
> tell from any other failure... bigshot's `bs_put` and eohunter's send ladder each carry a
> private copy of the ladder for exactly those reasons."*

Three independent implementations converging is strong evidence this is **domain truth,
not local taste.**

**Arm-before-send** (`actions.rb:310-315`) is correctly identified in the plan
(`01-architecture.md:180-190`) and needs no correction. One nuance worth keeping — the
author's own caveat against over-trusting correlation (`architecture.md:123-126`):

> *"Arming excludes pre-arm bus emissions, but a scanner can deliver an older queued line
> later, so event payload correlation remains necessary and **does not itself prove that
> this command caused the event**."*

**Cena: adopt wholesale.** `MAX_RESENDS = 5`, `SEND_DEADLINE = 30`, `RT_SETTLE_CAP = 15`,
`PERMANENT_REFUSALS` — hard-won, and should live in Rust where every Lua script inherits
them.

### 3.2 Events: right interface, wrong owner — and the plan inverts it

**The interface is good.** `events.rb`'s header claims *"Depends on nothing (pure Ruby) so
it is fully spec-testable"* and that holds. `ArmedWait` (`events.rb:33-114`) is careful
work: monotonic deadline (`Process::CLOCK_MONOTONIC`), `ensure cancel` so a raising
stop-check still releases the subscription, idempotent `cancel`, and a subscriber that
raises is reported but *"never breaks other subscribers or the emitter"*
(`events.rb:167-174`).

**The ownership is fatal, and the plan says the opposite.**
`01-architecture.md:177-178`:

> *"**`world` is an instance, never a global.** Three characters means three worlds. **This
> is what makes multi-session free rather than retrofitted.**"*

Half true. `World` *is* an instance — one `World.new` at `scripts/eohunter.lic:687`,
threaded through `runner.rb`. Clean dependency injection.

But **`Events` is a process-wide singleton.** `events.rb:118-121` declares `@mutex`,
`@subscribers`, `@any_subscribers`, `@waiters` at module scope; `events.rb:123` opens
`class << self`. All 95 `Events.emit` sites and all 33 `Events.on` subscriptions share one
bus.

eohunter's own test suite documents the consequence —
`spec/eohunter/single_instance_spec.rb:5-10`:

> *"engine.rb's load does remove_const on EO::Engine, and Ruby resolves constants at call
> time, so a second instance hands the first one's before_dying a different Watch and
> Events: **killing the older instance uninstalls the LIVE one's tracker handlers, its
> DownstreamHook and its event bus, and the running hunt goes silently deaf to every
> combat fact, disarm and flee line.**"*

The shipped remedy is **prohibition** — `scripts/eohunter.lic:127` refuses to start a
second instance.

**So the design Cena cites as its multi-session foundation is the design whose
multi-session story is "refuse to run twice."**

**Judge's correction, and it is important:** the root cause of the single-instance
prohibition is **not** the global bus. It is `engine.rb:29` —
`::EO.send(:remove_const, :Engine) if defined?(::EO::Engine)` — **reload hygiene forced by
Lich's script-loading model** (one long-lived Ruby process, scripts `load`ed in, no module
unloading). `engine.rb:25-28` says so:

> *"A script restart loads this file into a Lich that still holds the last run's
> EO::Engine. Drop it first, so every constant below is defined once... and nothing from a
> file since edited survives the reload."*

**Adjudication:** the global bus and the reload dance are **two consequences of the same
single-process constraint**, not independent choices. Cena's fix comes from Rust's
ownership and module model, not from bus placement alone. **This is a Lich-imposed
constraint, not a design failure — the author had no cheap alternative.**

**Cena: the bus is owned by the session**, alongside its `World`. Three characters means
three buses and the guard disappears.

### 3.3 The bus is doing two jobs

Of the `Events.emit` sites, only a handful are in `watch.rb` (real parsed game facts); the
overwhelming majority are **engine narration** — `:rest_started`, `:rest_stuck`,
`:rest_stranded`, `:routine_action_started`, `:waiting_for_followers`. And there are only
**two** blocking-await consumers in the entire engine (`actions.rb:312`, `routines.rb:717`).

So *"momentary facts travel on the event bus"* describes a small fraction of actual
traffic. The rest is a **structured log / observability channel** — confirmed by
`controller.rb:668`: `@event_handler = Events.on(:any) { |event| record(event) }`.

Not a criticism; fusing them is reasonable at 18K lines. But **Cena should separate them
deliberately**: **game facts** (typed, from the parser, awaited) versus **engine telemetry**
(script-emitted, subscribed, recorded). Conflating them in Rust means every telemetry
emission pays the typed-event tax, and the `:any` subscriber becomes a **cross-session
information leak** across the very boundary Seam C exists to protect.

*(Count caveat: the analyst's per-file emit/regex tables did not replicate exactly under
the judge's broader grep patterns. Rankings and conclusions survive; do not quote the
integers as precise.)*

### 3.4 World: the rules survive, the shape does not

**Does it stay read-only? Yes — genuinely.** Zero `put`/`fput`/`Script.start`/`dothistimeout`
in `world.rb`. No instance state mutated after construction. Only writes are two
memoizations and lazy sub-facade construction. **The read-only rule held perfectly across
982 lines.** Credit where due.

**Is it the only reader? Nearly.** Five real bypasses in 18K lines — `actions.rb:290`
(`GameObj.targets`, deliberate, the shared live-target gate), `combat.rb:241`,
`flee.rb:403`, `routines.rb:1013`, `routines.rb:1774`. All are **inventory/container
reads**, suggesting `World`'s `Hands` facade was built for combat and never grew a
container model. And `flee.rb:400` labels its own violation honestly:
`# The seam: outside Lich there is no inventory.`

**Does it stay parse-free? Yes in substance.** The regex uses in `world.rb` all match
against **already-parsed** `GameObj`/`Effects` values — `o.noun.to_s =~ /vine/`,
`npc.status =~ /dead|gone/` — not raw game lines. Predicate matching over structured data,
not parsing. Parsing did not disappear; it **relocated** to `routines.rb` and `cleanse.rb`.

**But the shape is a Ruby-mocking artifact.** The tell is the run of methods at
`world.rb:363-434`, each body a bare constant: `def xmldata = ::XMLData`,
`def gameobj = ::GameObj`, `def status = ::Lich::Gemstone::Status`, `def spell = ::Spell`,
`def map = ::Map`, `def char = ::Char`. The author confirms it,
`docs/guides/architecture.md:108-111`:

> *"The source accessors (`xmldata`, `gameobj`, `status`, `spell`, `map` and the rest) are
> **the spec seam**: a spec stubs those and fakes any state without a game."*

`world.rb` has exactly **156** methods, of which **103 are one-line `def x = ...`
delegations.** They exist because `::XMLData` is a hard-coded global constant that RSpec
cannot replace.

**Judge's correction (accepted):** "zero semantic content" overshoots. The same block
contains `def clock = Time` — genuine time injection, valuable in Rust too — and implements
the documented rule that fallible Lich reads are *"rescued to a safe default at that seam
and nowhere else"*, which is a **real architectural boundary**. Narrow the claim to the
bare-constant accessors specifically.

**Cause does not transfer.** In Rust the read-only guarantee comes from `&` and the
injectability from a trait or generic parameter. Porting the facade *shape* yields 156
pass-through methods the borrow checker already guarantees for free — ceremony that then
**accretes**: `world.rb` went 596 → 982 lines in **seven days** (4edaf6c 2026-09-10 → HEAD
fd131b2 2026-09-17), immediately after a 257-line deletion. **The rules transfer fully; the
implementation is Ruby-specific scaffolding.**

### 3.5 The missing fourth seam

eohunter grew four private state classes — `Engage::State`, `Cleanse::State`,
`Maintain::State`, `Routines::State` — in the gap the three-seam model leaves.

**Judge's correction (accepted, and it sharpens the argument):** these are largely
**fight/room-scoped latches**, not indefinitely durable facts — `engage.rb:877` resets on
`:entered_room` (`@state.new_room!`) and `routines.rb:21` calls it *"per fight"*. So the
right argument is **not** "World should have held them." It is that they fit **neither
bucket** of the author's own binary discriminator (`architecture.md:114`: *"Momentary facts
travel on the event bus; durable facts live in World"*). **The taxonomy is incomplete.**

**Cena: add a per-session, script-scoped `State` seam** for facts with a lifetime between
"one event" and "durable world fact" — fight-scoped, room-scoped, engagement-scoped
latches. Four independent rediscoveries in one codebase is enough evidence.

### 3.6 What Cena should do

- **Adopt `actions` and arm-before-send unchanged.** Right for anyone.
- **Adopt `world`'s rules** (read-only, no sends, no text parsing) but **not its shape** —
  a `&GameState` borrow, not a 156-method delegation wall.
- **Reject the global `events` bus.** Per-session, owned by the session.
- **Split game facts from engine telemetry.**
- **Add the fourth seam** — scoped mutable state.
- **Do not copy the 50 ms polling sleeps** (`events.rb:70`, `actions.rb:341`,
  `actions.rb:361`). See transfer traps.

---

## Decision 4 — Per-line text hooks vs parse-to-typed-frames

**Still applies:** no · **Right even then:** yes — the only correct option · **Changes the plan:** yes

**Verdict: `01-architecture.md` §0.1's decision is right and should stand. Its stated
reason is wrong, and the correction changes what Cena must build.**

### 4.1 Lich already parses first — the plan's framing is inaccurate

`lib/games.rb:974-983`, `Game#process_server_string`, in order (verified):

```ruby
process_xml_data(server_string) unless server_string =~ /^<settings /   # :975
Lich::Common::Inventory.observe(server_string) if defined?(...)        # :981
process_downstream_hooks(server_string)                                # :983
```

and `process_xml_data` runs `Ox.sax_parse(XMLData, server_string, ...)` at `games.rb:1068`.

**Lich parses the stream into fully typed state (`XMLData`, `GameObj`, `Char`, `Effects`,
`Room`) *before* any hook or script sees anything.** The plan's §0.1 sentence — *"Lich
hands scripts raw text lines and lets each script parse. That single choice causes most of
its structural problems"* — is **not accurate as a description of the pipeline.**

**Judge's correction, which I verified and which matters:** Lich has **three** script-facing
paths, not two.

- **(a) Typed state** — `XMLData` / `GameObj` / `Char`.
- **(b) The ordinary text feed** — `games.rb:1089-1094` (verified) calls
  `Script.new_downstream_xml(server_string)` and then, per line,
  `Script.new_downstream(line)` on **line-split stripped text**. This is where `get`,
  `waitfor`, `matchwait` and `watchfor` read (`script.rb:1542-1544`). It is a **separate,
  non-mutating** path that **cannot touch the frontend**. **Most script-visible text
  arrives here, already separated.**
- **(c) `DownstreamHook`** — the frontend path, mutating, shared.

**So the analyst's headline — "script filtering and frontend forwarding are THE SAME
PIPELINE" — is overstated.** The seams are not merged. `DownstreamHook` is a **privileged
opt-in escape hatch**, not the default route.

What *is* true and useful: **the `DownstreamHook` chain is shared between script filtering
and frontend forwarding**, and that is where the bug class lives (`games.rb:1149-1181`):

```ruby
if (alt_string = DownstreamHook.run(server_string))
  alt_string = sf_to_wiz(alt_string) if Frontend.supports_gsl?
  alt_string = prefix_origin_sentinel(alt_string) if Frontend.supports_sentinel?
  send_to_client(alt_string)
end
```

The string a script mutated **is** the string written to the frontend socket.

### 4.2 Why Lich cannot canonically re-encode — the proxy obligation, quantified

`lib/common/frontend.rb:21-29` — **eight** frontend definitions Lich did not write:
`stormfront/wrayth`, `profanity`, `genie`, `frostbite`, `suks`, `wizard`, `avalon`, `saga`.
Capability vocabulary: `%i[xml gsl streams mono room_window sentinel]` (`frontend.rb:20`).

`lib/common/markup.rb` (264 lines, verified) is the transcoder, and it is **lossy and
one-way**. `sf_to_wiz` rewrites XML into GSL escape channels: `<pushBold/>` →
`"\034GSL\r\n"`, `<style id="roomName"/>` → `"\034GSo\r\n"`,
`<pushStream id="death"/>` → `"\034GSw00003\r\n"`; **everything else:
`line.gsub(ANY_TAG, '')` — deleted.**

**There is no inverse.** GSL has no representation for most of the XML vocabulary, so
Lich's only correct behaviour for an XML frontend is *forward the original bytes*. A
canonical intermediate re-encoding would silently degrade Wrayth to Wizard's capability
set.

**This fully vindicates the choice. It was not laziness — it was the only correct option
available.**

### 4.3 The `QUIET_STATE_TAGS` bug — correctly cited, incompletely diagnosed

PR **#1530** (merged), in the author's own words:

> *"`Lich::Util.issue_command`'s `quiet: true` filter drops any chunk inside the matched
> start/end range via `next(nil)`. `DownstreamHook.run` is called on the raw, unsplit
> socket chunk (before line-splitting...), so the server can bundle a formatting-state tag
> onto the same chunk as text quiet mode is about to discard. In practice: a quiet range
> ending at `<prompt>` arrives bundled with the game's closing `<output class=""/>` tag...
> and the whole chunk — prompt included — gets dropped. **The frontend is left stuck in
> mono formatting until an unrelated, later mono tag happens to close it.**"*

**The true cause is *chunk framing*, not text-vs-types.** If Cena parses first, that
specific bug genuinely does vanish.

**But the general hazard does not.** Cena will still have a **presentation-state** class of
frame (mono on/off, bold push/pop, stream open/close) whose suppression corrupts the
renderer. A Lua script filtering `Frame::Text` can still drop a `StreamOpen`/`StreamClose`
pair if the filter API hands it a batch or a span rather than one frame at a time.
**Types alone do not fix it.** The fix is that **presentation-state transitions must be
non-suppressible by script filters, by construction.**

The surviving comment is candid (`util.rb:88-91`): *"Only the mono/formatting `<output>`
tag is confirmed to hit this in practice today. The list is deliberately an array of
patterns (not a single regex) so a future confirmed case — e.g. pushBold/popBold or
pushStream/popStream — can be added as its own entry."* **Read that as a hazard register,
not a solved problem.**

And the fix's own fix, PR **#1532**, is the more instructive artifact:

> *"**Confirmed in production: with an unterminated preserved-tag chunk, that byte was
> rendering on screen as a literal, visible 'US' character** during quiet-suppressed
> output — reported as a regression starting with the tag-preservation fix."*

**Layered string surgery over a shared pipeline generates a new bug at each layer.** That
is the strongest available argument for Cena's typed approach — and simultaneously a
warning that Cena's frame→bytes encoder, if it ever serves an external client, inherits
this fragility wholesale.

### 4.4 `DownstreamHook` is being used as an observer seam it is not

Lich's newest subsystems register hooks that **return the string unchanged** —
`lib/gemstone/combat/messages.rb:137`, `combat/tracker.rb:560`, `common/xmlparser.rb:842,866`.

And the newest subsystem **routed around the chain entirely**. PR **#1524** wires
`Inventory.observe` as a direct parser-thread call at `games.rb:979-981`, with the stated
reason:

> *"**Crash-safe.** `observe` never mutates the stream, never sends upstream, and **never
> raises** — the parser thread's only rescue does `@socket.close` and would drop the
> player's session."*

**Judge's correction (accepted, and it produces a better lesson):** the analyst said these
taps abuse the mutating chain *"because no observer seam exists."* An observer seam **does**
exist — path (b). The real reason is **framing**: these taps need the **unsplit socket
chunk** (combat segments and nerve-tracker state span line boundaries), and path (b) only
offers line-split stripped text.

**The corrected lesson is stronger:** *the observer seam must offer whatever granularity
consumers need (chunk / line / frame), or consumers get driven into the mutating path by a
framing mismatch.*

### 4.5 Verdict correction: "calcified" is wrong

The analyst graded this *yes-but-calcified*. **The judge is right to reject that.**
Calcification means a workaround that outlived its reason. All eight frontends still exist
(`frontend.rb:21-29`) and PRs #1530/#1532 show the constraint biting in 2025–2026. **Lich's
text path is load-bearing today, not vestigial.**

Accurate verdict: **correct then, still correct for them, wrong for Cena** — the highest-value
category in this document. Grading it "calcified" is precisely the unfairness the fairness
rule warns against.

*(Also noted: PR #1634, cited elsewhere as evidence of Lich's typed-contract direction, is
**state OPEN**, not merged. The quoted text is verbatim and the argument survives, but the
certainty should be lower.)*

### 4.6 What Cena should do — four corrections to §0.1

**(1) Fix the stated reason.** §0.1 says the text API *"causes most of its structural
problems."* The real cause is that **Lich's `DownstreamHook` filter chain and its
frontend-forwarding path are the same pipeline.** Cena removes that class of bug by
**separating the seams**, not merely by having types. Keep text-vs-types as a second,
independent improvement.

**(2) Split `DownstreamHook` into two Cena APIs.**

- `session:observe(frame)` — **cannot mutate**, no ordering hazard, parallelizable.
  Offer it at more than one granularity (frame, line, and raw chunk) so nobody is driven
  into the filter path by a framing mismatch.
- `session:filter(frame) -> Option<Frame>` — **presentation only**, affects the local UI,
  **never** other scripts' input.

Lich's hook priorities (PR #1621) exist only because everything shares one mutating chain.
Two seams shrink that machinery substantially — **but it does not vanish.** Two Lua scripts
both filtering presentation still compose order-dependently. **Cena's filter seam needs
deterministic, specified ordering.**

**(3) Replace `Frame::Unknown(RawTag)` with Vellum's actual, better answer.** The plan's
`Unknown` (`01-architecture.md:~130`) is a **typed dead-end** — a Lua script receiving
`Frame::Unknown` can do nothing useful with it.

Vellum ships the right design at `src/parser/text.rs:174-195` + `src/parser.rs:1036-1051`
(verified): a sorted `KNOWN_WIRE_TAGS` table, binary-searched, splitting unknown tags into
two cases — **known but unhandled** → `tracing::debug!`, swallowed; **genuinely unknown
name** → `text_buffer.push_str(tag)`, i.e. **rendered as visible text.** Its own comment
states the rule:

> *"Unknown names indicate new server markup and are passed through as visible text (never
> silently dropped) so protocol changes announce themselves."*

That **is** the same-day-fix property, and it is strictly better than Lich's, because Lich
silently loses unmodelled content into prose while Vellum's warns **and** stays visible.

Cena should carry `Frame::Text` with a `raw: Option<String>` provenance field plus a
`Frame::UnknownTag { name, raw }` that (a) renders as text by default, (b) logs at warn,
and (c) **is matchable from Lua by tag name** — so a script author can handle a new
Simutronics tag the day it appears without a Cena release. Add an explicit escape hatch: a
Lua `session:on_raw_line(fn)` observer receiving the pre-parse line, documented as a
compatibility hatch, rate-limited, and never on the path that feeds the UI.

**(4) The wire is BIDIRECTIONAL and structured — §0.1 does not mention this at all.**
Lich scripts inject their own dialog XML into the frontend. VellumFE's redesign spec:

> *"Wrayth is a declaration-first protocol — the game itself declares windows
> (`<streamWindow>`, `<dialogData>`, resident `<openDialog>` panels), and lich scripts
> inject their own dialogs using the same XML (UberBar)"*
> — `.beads/artifacts/window-system-redesign/spec.md:5`

Vellum invented its own extension tags (`vellumTimer`, `vellumCmd`, `vellumImg`,
`parser.rs:78-104`) to do the same.

**Cena's `Frame` vocabulary must therefore be an *emit* vocabulary too.** Lua must be able
to construct frames — a dialog, a timer, an image, a window — and have the UI render them,
without writing XML. **That is the single largest capability Cena gets for free from owning
both sides of the wire, and the plan does not mention it.**

**(5) Two missed hazards the plan must address** (judge's find, and both are real):

- **Ox's permissiveness is load-bearing domain knowledge a strict Rust parser loses by
  default.** `games.rb:1054-1058` (verified): *"Ox is a permissive parser: it handles
  Simu's not-quite-XML stream without the clean/retry dance REXML required (nested quotes,
  missing 'd' end tags, etc. are tolerated rather than raised)."* Plus
  `repair_malformed_attributes_and_reparse` (`games.rb:1119-1131`) handling two specific
  server malformations — the `settingsInfo` "space not found" bug, and same-quote-in-quoted-value
  from Simu's dynamic dialogs (`title='Tsetem's Items'`). And `convert_special: false`
  (`games.rb:1068`) with a documented entity-corruption history. **A strict Rust XML parser
  will reject this stream.** Cena's L2 must be written permissively from the start.
- **Stream desync.** `check_stream_desync!` (`games.rb:1097-1103`, verified) promotes
  truncation-class parse errors to `GameStreamDesyncError`, which logs and **resets
  `XMLData`** *"rather than killing the server thread."* **Typed frames make desync WORSE
  than text**, because typed state accumulates and silent corruption persists. Cena needs
  an explicit **desync-detect-and-reset** design; `01-architecture.md` §2 does not address
  it.

---

## Decision 5 — Session as an OS process, and the active-sessions service

**Still applies:** no · **Right even then:** yes, unambiguously · **Changes the plan:** yes

### 5.1 The real constraint: process-global character state

Three candidate causes are disprovable from the tree.

**Not the proxy architecture.** Lich supports `--without-frontend` (`main.rb:140, :350,
:390, :697`), and `Lifecycle.resolve_role` returns `'headless'` for exactly that case
(`lifecycle.rb:75`). If the proxy shape were forcing, headless mode is the obvious place to
collapse two characters into one process. It isn't done.

*(Judge's correction, accepted: this is **strong circumstantial evidence, not a disproof**.
`arg_normalization.rb:16-18` shows `--headless PORT` normalizes to
`--without-frontend --detachable-client=PORT`, so headless and socket-serving are not
disjoint; and `main.rb:697` shows true headless still runs the full frontend-emulation
pipeline toward the game server. The load-bearing form is "nobody ever even asked whether
headless could merge characters" — absence of evidence.)*

**Not the GIL.** Lich already runs scripts as in-process Ruby threads: `Thread.new`
(`script.rb:192`), per-script `ThreadGroup` isolation (`:1886`, `:2538`). **This is the
genuinely decisive disproof** — it is affirmative, not absence of evidence.

**Not crash isolation as a *cause*.** It is a real benefit they receive, but nothing in the
repo argues "we fork so a crashing script kills one character."

**It is the global namespace.** `lib/common/gameobj.rb:25-42` (verified):

```ruby
@@loot = []; @@npcs = []; @@npc_status = {}; @@pcs = []; @@inv = []
@@right_hand = nil; @@left_hand = nil; @@room_desc = []
```

Those class variables **are** the character's hands, inventory and room. `XMLData` is a
process-wide singleton referenced **575 times** across `lib/`. `$_CLIENT_` appears **69**
times. `Script` keeps the running-script table in class variables (`script.rb:345-346`).

**A second character in the same Ruby process would write its right hand over the first
character's right hand.** One character per process is not an architecture choice; it is
the only arrangement that namespace permits.

**INFERRED, high confidence:** this is inheritance, not decision. I found no PR, issue or
comment in 1,644 PRs proposing multi-character-in-one-process, or defending one-per-process.
**It is the water, not a choice.**

### 5.2 The scale the plan under-states

PR #1319:

> *"Multiple Lich processes (one per game character) share a single `lich.db3` file. With
> the default DELETE journal mode, any write takes an exclusive lock that blocks all
> readers. **With 25 concurrent character sessions**, reads were colliding with writes
> frequently enough to produce thousands of `SQLite3::BusyException: database is locked`
> warnings per day."*

**Twenty-five, not three.** `01-architecture.md`'s "3+ characters at once" should be
recalibrated — the community operates this an order of magnitude past that.

**Judge's correction, and it inverts the analyst's inference:** the `BusyException` storm is
a **multi-process artifact**, not a preview of Cena's internal contention. The cause is 25
OS processes taking exclusive **file** locks under DELETE journaling; the fix is WAL. A
single Cena binary with one writer task or a connection pool has an ordinary in-process
mutex instead. #1319's own body concedes the residual is negligible (*"~1 write every 2.4s
across 25 processes, each holding the lock for <1ms"*).

**Adjudication: #1319 is evidence FOR collapsing to one process.** Keep the scale
recalibration; **drop the contention inference.**

### 5.3 Was it right even then? Yes, and the retrofit is competent

`lib/internal_api/active_sessions.rb:22-30` is the best design comment in the file
(verbatim):

> *"Ownership is coordinated with a cross-process advisory file lock ... rather than a
> fixed, well-known TCP port. The single process that holds the lock binds an ephemeral
> port and publishes it in the discovery file; peers read that port to reach the owner.
> **Because the kernel releases an advisory lock the instant its owning process dies, a
> crashed owner never blocks a successor, and there is no fixed port for a stuck process to
> squat.**"*

That is a correct and non-obvious solution to leader election among peers where any peer
may die. The local-only-TCP choice is justified too (`server.rb:13-16`): *"intentionally
local-only TCP to keep behavior consistent across Linux, macOS, and Windows."*

**The bug-fix history is the finding.** #1302 (zombie server deadlock), #1317, #1334,
#1340, #1355, #1359, #1452, #1520, #1612 — **nine bug-fix PRs against ~1,600 lines.**
Cross-process coordination in a language without process supervision is expensive to get
right, and **roughly 900 of those 1,643 lines exist only to serve the process boundary.**

### 5.4 The brief's "two multi-client mechanisms" premise is false

There is no second mechanism for the same problem. They are **orthogonal**.

- **Detachable-client = N frontends on ONE session.** `detachable_client_registry.rb` is
  **58 lines** — an `@clients` array behind a `Mutex`, with `primary`/`primary?`, entirely
  inside one process, fed by a `TCPServer` accept loop at `main.rb:897-943`.
- **Active-sessions = ONE registry across N processes, one character each.**

**Frontends-per-session versus sessions-per-machine.** Lich has both because it must: it
cannot merge sessions (globals) and it cannot own the frontends (third parties). **This
does not indicate hidden difficulty; do not spend design effort reconciling them.** The
`role` trichotomy `'headless' | 'detachable' | 'session'` (`lifecycle.rb:75-78`) is just one
mechanism reporting itself to the other, and is **meaningless for Cena** — it describes
which external frontend arrangement a Lich process is in.

Detachable-client's origin is **containers and remote play**, not multi-character: issue
#311 (2023-02-27) — *"Enables greater flexibility in detached client mode, especially where
this is deployed via docker or other containers."*

### 5.5 Cross-session messaging WAS considered, built twice, and rejected — for a reason Cena does not share

This inverts the plan's position at `01-architecture.md:~222`, which calls a cross-session
messaging API *"its biggest footgun."*

The inventory records "Lich has no cross-session messaging verb; the protocol is exactly
ping/upsert/remove/snapshot (`server.rb:228-242`); eohunter built 1,531 lines of DRb
instead." All true. **But "core never added messaging" is not the whole story.**

- **PR #1613** — *"feat(all): prototype bounded read-only session coordination"*
  (therealatari, 2026-09-13), 1,826 additions. Scope: *"Draft implementation of the reviewed
  coordinated-independent-sessions proposal, read-only slices A/B. Each character keeps its
  own Ruby process. This is not Sessionize, a new broker, a hunting engine, or a
  write-capable control interface."* Tested: *"Full exact-branch suite: 7,591 examples, zero
  failures"*; *"final ten-owner p99 2.786 ms."*
- **PR #1618** — the write-capable sibling. Together **4,973 added lines.**
- **Both closed unmerged on 2026-09-14**, one day later.

The stated reason is @calael's one-line review:

> *"Still think that this could be done in a lib. Any reason it should be in core? It's
> pretty big chunk of code, and not all folks use it."*

**It was rejected as CORE BLOAT for a general-purpose Ruby engine serving a broad
community — not as a bad idea.** Cena has one owner, one consumer profile, and multi-session
as a stated goal. **That rejection reason does not transfer.**

This also answers the inventory's *"Aspirational — zero callers"* reading of
`Lich::API`/`lib/api/active_sessions.rb` (`08-lich-arbiter-direction.md:151`). The 44 lines
with zero callers are **the surviving read-only slice of a rejected 4,973-line proposal** —
a deliberate reversal, not abandonment.

**The design work is done and free to take:** request/receipt lifecycle with receiver-issued
preflight tickets; immutable request IDs bound to owner/peer session identity plus a
canonical argument digest; generation fencing; idempotent resubmit; conflict on
meaning-change-under-same-ID; and the explicit **`Operations`** (discrete, request/receipt)
versus **`Leases`** (long-lived, fenced, renewable) split.

Also take eohunter's inversion (`hunting-engine-plan.md:1033-1040`):

> *"every tick each follower pushes one `Report` ... and pulls its `Order`s, so the leader
> never makes a remote call and cannot be stalled by a follower"*

**That is the right shape in-process too**, because it prevents one session's scheduler from
blocking on another's.

**Judge's correction on scheduling, accepted:** the evidence supports *"messaging was
rejected as core bloat for a broad-community engine, a reason Cena does not share, and
#1618's design is free to take."* It does **not** support *"build it now."* @calael's bloat
concern has a Cena-shaped analogue: **a seam built before a consumer exists is maintained
without feedback.** And #1618's own body stages this — `Leases` are explicitly *"a future
sibling interface requiring separate design and approval."*

**Defensible version: design the ticket/digest/fencing contract now, because it constrains
the session-actor boundary. Implement when a second session actually needs it.**

### 5.6 Runaway containment — the real cost of deleting the process boundary

In-process actors genuinely do lose crash isolation. Lich's own admission, PR #1575
(merged 2026-09-16): execution guards are *"cooperative control, not a sandbox or
preemptive timeout. It does not intercept arbitrary Ruby, direct socket access, or blocking
code without checkpoints."*

Ruby cannot contain a runaway in-process; an OS process can. Cena's mitigations:

- **(a) Lua debug hooks** — `lua_sethook` with `LUA_MASKCOUNT` every N VM instructions.
  Unlike Ruby this **is** a real preemptive checkpoint, so `while true do end` becomes a
  killable coroutine.
- **(b) Per-session memory budget** via a custom `lua_Alloc` failing past a cap, so OOM is
  one session's script erroring rather than the process aborting.
- **(c) One Tokio task (or dedicated thread) per session**, so a wedged session cannot
  starve the runtime's other sessions.
- **(d) `catch_unwind` at the session task boundary** plus supervisor restart.

**Judge's correction, accepted and important — the recovery is softer than it sounds.**
`LUA_MASKCOUNT` counts VM instructions only: it does **not** preempt a blocking FFI call, a
blocking syscall from a C library, or a long-running C function. `catch_unwind` is defeated
by `panic = "abort"` (common in release profiles) and does not catch aborts from foreign
frames. A custom `lua_Alloc` returning NULL raises a **recoverable** Lua memory error, but
**Rust-side** allocation failure still aborts the process.

So (d) reads correctly as: *"a panicking session restarts alone, provided the build does not
use `panic = "abort"` and the panic originates in Rust."* The triple genuinely buys back
**most** of the process boundary — not all of it. **Say so in the plan.**

Also: Lich's **PID-liveness sweep** (`registry.rb:121-130`) is *positive* about reaping a
crashed peer **with no cooperation from it**. Cena must keep that property via
supervisor/`JoinHandle` observation, **not just drop it as "transport accident."**

### 5.7 What Cena should do

**Keep in-process actor sessions** — the forcing constraint is a Ruby-namespace artifact
Cena does not inherit, and Rust makes the sealing real.

**Do NOT port:** TCP server, JSON line framing, `SecureRandom.hex(32)` auth token, discovery
file, flock ownership election, Windows sharing-violation retry ladder
(`[0.01, 0.02, 0.04, 0.08, 0.16]`), 2s heartbeat, the `role` trichotomy.

**Do keep:** snapshots-never-handles; the 10-field enumerated payload; **presence ≠
connected**; generation counters; inert fallbacks; a kill switch; supervisor-observed
liveness.

---

## Decision 6 — Vellum's AppCore and mechanical architecture enforcement

**Still applies:** yes · **Right even then:** yes · **Changes the plan:** yes

### 6.1 The central question, and why its premise is wrong

> *Vellum tried to enforce architecture mechanically and still ended up with a 26k-line
> AppCore. Does that mean the enforcement failed?*

**Neither — the premise measures the wrong thing.**

26,825 is the line count of the **directory** `src/core/app_core/`, across **29 files**.
**No file in it approaches that.** The largest is `commands.rs` at 5,132 — a dot-command
dispatcher, deliberately **uncapped**. `state.rs` — the file the cap names — is **2,089
lines**.

The surrounding ratio is the real evidence:

| Layer | Lines |
|---|---|
| `src/` total | 324,605 |
| `src/core/` | 102,438 |
| &nbsp;&nbsp;`src/core/app_core/` | **26,825 (26% of core, 8% of src)** |
| &nbsp;&nbsp;`src/core/messages/` | 11,107 |
| &nbsp;&nbsp;`src/core/travel/` | 9,308 |
| &nbsp;&nbsp;`src/core/pathing/` | 5,507 |
| &nbsp;&nbsp;`src/core/layout_engine/` | 4,572 |
| `src/frontend/` | 152,957 (of which `gui/` 81,702) |
| `src/data/` | 8,359 |

**The heavy domain logic — travel, pathing, layout solving, highlighting, mapping, the
message pipeline — lives OUTSIDE AppCore**, in sibling modules under `core/`. A true god
object would have swallowed those. **Nothing in `tests/architecture.rs` forced that; the
author did it anyway.**

**Judge's corrections to the analyst's structural claims (both accepted):**

- `pub struct AppCore {` is at **`state.rs:40`** and closes at `:423` — the analyst cited
  the *closing brace* as the declaration site. Actual field count: **105**, not 88.
- **"state.rs is ~1,600 lines of struct declaration" is false.** Struct = lines 40–423 (383
  lines); `impl AppCore` = 425–2018 (1,593 lines, 42 methods), of which only ~380 are the
  two constructors. The rest includes **real logic** — `process_server_data_inner`
  (1392–1564), `apply_custom_quickbars`, `apply_session_cache`, `get_available_commands`,
  `add_system_message`. **The "facade holds only struct + constructor + accessors"
  characterization overstates the purity.**

**The "not a god object" conclusion still stands — but it stands on the sibling-module
evidence, not on `state.rs`'s internal composition.**

### 6.2 The caps: when, why, and the decisive fact

| Date | SHA | What |
|---|---|---|
| 2025-10-10 | — | Repo created |
| 2025-12-10 | `d22aa826` | Beta 2 rewrite; core/data/frontend split; `Frontend` trait born |
| 2026-07-05 | `c2e9af23` | **First architecture test** |
| 2026-07-05 | `c523713a` | First **line cap**: `config.rs` ≤ 700 |
| 2026-08-06 | `49125c03` | Catalog seam enforcement |
| 2026-08-09 | `d9b875bb` | **The wall**: split the 9,227-line AppCore impl |
| 2026-08-09 | `f022411b` | **Caps introduced** |
| 2026-08-10 | `6381d0cf` | **Caps tightened** |
| 2026-09-16 | `7a7145e2` | `server_time_offset_has_a_single_owning_field` |

**The first architecture test landed nine months into a repo now at 324K lines.** This
directly corrects `01-architecture.md:426-428`, which claims enforcement *"is why its
layering survived 310K lines."* **The causality runs the other way.** Enforcement arrived
at roughly line 250K, **after** the author had already hand-split a 9,227-line impl.

**Enforcement ratchets a refactor you already did; it does not do the refactor for you.**

The author's own words, `f022411b` — *"chore(arch): close enforcement gaps opened by the
monolith splits"*:

> - *`core_is_android_safe` now scans `src/parser/` submodules (the split had left **43% of
>   the parser outside the desktop-crate ban**)*
> - *new `split_parents_stay_facades` test caps the six residual parent files ... **so the
>   monoliths can't silently re-form***
> - *the eight state mutators/menu builders widened to `pub(crate)` during the state.rs
>   split are narrowed ... **the wider visibility let frontends bypass the
>   validate-then-persist wrapper***

**That third bullet is a warning for Cena:** a mechanical refactor *silently opened a
boundary the visibility system was supposed to hold*, and the author had to notice by hand.

**The decisive fact: the caps have never been raised.** Measured today:

| File | Lines | Cap | Headroom |
|---|---|---|---|
| `src/frontend/gui/app.rs` | 3,572 | 3,600 | **99%** |
| `src/core/app_core/state.rs` | 2,089 | 2,100 | **99%** |
| `src/core/messages.rs` | 793 | 800 | **99%** |
| `src/parser.rs` | 1,063 | 1,100 | **96%** |
| `src/frontend/gui/app/widgets.rs` | 648 | 700 | 92% |
| `src/frontend/tui/window_editor.rs` | 1,514 | 1,700 | 89% |
| `src/config.rs` | 595 | 700 | 85% |

Four files sit within 1–4% of a cap that has never been relaxed. **This is not a dead rule
with comfortable margins; it is actively pushing code down into submodules.**

The policy is in `CLAUDE.md` rule 4: *"**If a cap trips, move code down, don't raise the
cap**."*

**Judge's correction on the tightening (accepted).** `6381d0cf` is often described as
tightening caps "to the new sizes." It actually set caps with **8–41% deliberate slack**
above post-refactor actuals (e.g. `state.rs` actual 1,901, cap 2,100). **That slack has
since been consumed** — `app.rs` +252, `state.rs` +188, `messages.rs` +166, `parser.rs`
+175 in six weeks — which is *why* the caps bite at 99% today. State it that way.

**Honesty correction on the record length:** the dependency rules have a ~2.5-month record;
**the facade caps have a six-week record.** Six weeks of no-raises is the best evidence
available and is genuinely encouraging, but it is not proof, and Cena will act on this for
years.

### 6.3 What the caps enforce that a crate graph cannot

Every *other* rule in `tests/architecture.rs` is a **dependency** rule implemented as grep,
because Vellum is one crate with modules: `core_and_data_do_not_reference_frontend`,
`gui_does_not_reference_tui`, `core_and_data_do_not_reference_egui`, `core_is_android_safe`.

**Cena's workspace-of-crates makes every one of those free.** `01-architecture.md:456-458`
is right about that — and that is exactly the problem: **it makes rules 1–5 free, not
sufficient.**

**No crate graph can express "this file holds only the struct and the dispatcher."** The
line cap is the **one rule Cena must consciously choose to keep**, and it is the one that
actually constrained AppCore. **`01-architecture.md` §7 lists seven rules, all dependency
rules, and omits it.**

### 6.4 Two tests worth stealing that are not layering rules

**`server_time_offset_has_a_single_owning_field`** (`7a7145e2`, 2026-09-16). AppCore
carried a duplicate `server_time_offset` that **nothing assigned**, from the Beta 2 rewrite
until 2026-09-16 — **ten months**. Every GUI countdown rendered against offset 0; measured
over 6,373 prompts, *"49.8% inflate by 2s or more."* The author's rule is the transferable
artifact:

> *"**Delete the field rather than sync it: two fields that must agree will drift again,
> and deleting turns every reader into a compile error.**"*

**Cena's equivalents:** server clock offset, roundtime, current-room id, active-session
handle. **Single-owning-field tests from commit one.**

**`catalog_access_goes_through_the_seams`** (`tests/architecture.rs:288-318`) — proves a
seam is the only route to a capability by banning the symbols outside it, with a path
allowlist.

**Judge's correction, accepted and it kills the naive port.** That test bans **Rust symbol
names**. Cena's Seam B (`world`/`events`/`actions`) is reached by **string keys from Lua
script code**, which a symbol-ban grep **cannot see at all**. Cena needs a different
mechanism: a **registry-of-exposed-names test**, or a **single Rust chokepoint** that the
grep *can* police.

### 6.5 The `Frontend` trait: port the seam, not the trait — confirmed, with provenance

`grep -rn 'impl Frontend'` returns **exactly one** hit
(`src/frontend/tui/frontend_impl.rs:12`). `src/frontend/mod.rs` has **five commits ever**
(`d22aa826`, `850095bb`, `1385f54d`, `9285de6e`, `f38c361a`), **none adding an
implementor.**

`01-architecture.md` §Seam D already gets this right. The **causal story** it lacks: the
trait is not a bad design, it is a **December-2025 design for a one-frontend world that the
February-2026 GUI outgrew**. It specified the **wrong axis** — a polled loop — which eframe
inverts.

**The generalizable lesson is about timing, not traits:** *do not write a frontend trait
until the second frontend exists.* Cena, with four frontends planned and an explicit
`02-frontend-seam.md`, is in a genuinely different position. **Do not over-read "port the
seam, not the trait" into general trait-aversion.**

### 6.6 The window-system redesign: method worth copying wholesale

The strongest **process** evidence in the tree, and language-independent:
**strangler-fig migration behind a characterization suite.** Phases, a governing invariant
per phase, and *"5,015 passed / 0 failed before and after"* on the refactor commits.

And the **wire-evidence discipline**: the redesign's conclusions came from *"11.4 GB of real
Lich logs, 2368 files, 8 characters"*, which found `exposeDialog` ×4,265,
`deleteContainer` ×7,559, `exposeContainer` ×2,714 — **all silently dropped by the live
parser** — and `combat` as the most frequent dialog id at 2.6M.

**Cena's L1/L2 should be designed against a corpus census, not against StormFront
documentation.**

### 6.7 What Cena should do

1. **Keep mechanical enforcement and start with it** — §7 is correct. **But correct its
   reasoning** per §6.2 above.
2. **Add per-file line caps as rule 8**, with *"move code down, don't raise the cap"* in
   `CLAUDE.md`, and a test that fails on a cap **increase in the diff**, not just a breach.
3. **Expect and accept a Cena `AppCore` equivalent.** A MUD client's session state genuinely
   is interconnected. Count **files, not directories**, and give it a cap from commit one.
   **Do not inherit the number** — see transfer traps.
4. **Do NOT reflexively split the command dispatcher.** `commands.rs` is 5,132 lines and
   deliberately uncapped. A big flat dispatch table is the right shape for a command
   surface. Cena should make the same choice for its `.command` / Lua-command surface.
5. **Steal the two invariant tests** in §6.4 — with a Lua-aware mechanism for the seam test.
6. **Adopt strangler-fig + characterization suites** for every large refactor.
7. **Design TUI + GUI + web concurrently** so the frontend abstraction is *extracted* from
   three loop models rather than forecast from one.

---

# Transfer traps

**The most valuable findings in this document.** Each is a correct, well-evidenced result
whose **CAUSE** is specific to Ruby, to an era, or to a constraint Cena does not have — so
the conclusion does not carry to Rust/Lua, and may even invert.

The archetype, already caught once: **the combat parser's 58µs-vs-35µs measurement**
(`lib/gemstone/combat/parser.rb:34-40`). Many small literal unions beat one big union. It
is a **true measurement** and a **Ruby regex artifact**. Rust's `RegexSet`/`aho-corasick`
invert the conclusion.

But note the part of that comment which **does** survive: *"When def counts grow into the
hundreds-per-family, the proven fix is a first-word bucket index (see CritRanks — 2,389
patterns, indexed by first literal word), not a bigger union."* **Literal-prefix dispatch
beats linear alternation** is a real algorithmic insight — and it is exactly what
`RegexSet`/`aho-corasick` implement natively. **So Cena should NOT hand-roll bucket
indices; it gets the same win from `RegexSet`.** Correct conclusion, obsolete
implementation.

---

### TRAP 1 — `Script.current` looks like the textbook global-state antipattern

**649 calls, implicit ambient context, no session argument — exactly what a Rust rewrite
exists to delete.** But `Script.current` is not a getter; it is a **pause checkpoint**
(`script.rb:1101-1105`). Delete it naively and `;pause` stops working on scripts whose
authors never thought about pausing.

**Two corrections that make this trap subtler than it first appears:**

- **It is ~50 of 208 globals, not all of them.** The hot verbs check; most do not.
- **Pause and kill are different mechanisms.** Kill uses `Thread#kill` and needs no
  checkpoint — the Ruby VM does it. **So the "unkillable `while true do end`" risk in Cena
  comes from the coroutine decision (`01-architecture.md:311`), not from deleting the
  globals.** It is real; it is just filed under the wrong decision.

**What actually transfers:** the **requirement** (a runtime must be able to stop a script
that did not consent), not the mechanism. And Lich has **already moved off the ambient
lookup** — `ScriptExecutionGuard` / `check_execution_guard!` / `execution_sleep`
(`script.rb:3005-3017`) put the checkpoint **at the I/O wait**. That model is already
coroutine-shaped. **Copy the end state, not the retrofit.**

### TRAP 2 — "global-style APIs are untestable"

`spec/lib/common/global_defs_spec.rb` (148 lines) tests **2 of 208** functions, and to do
it it must `File.readlines` the source, regex-locate the target by line, slice it out, and
`eval(..., TOPLEVEL_BINDING)` — because requiring the file **mutates `Object`**
(`global_defs.rb:2316`: `undef :abort if respond_to?(:abort)`; PR #1156 shows the crash:
*"undefined method 'abort' for class 'Object'"*).

**This is a Ruby-specific consequence of top-level `def` being private methods on
`Object`.** Lua globals live in `_G`, an **ordinary swappable table**; a Lua sandbox can be
handed a fake `_G` per test trivially.

**Do not conclude "global APIs are untestable" as a general law.** The testability argument
against globals is real for Lich and **near-zero for Cena**. It should not appear in Cena's
rationale — the process-singleton argument is the one that holds.

### TRAP 3 — Infomon's SQLite shape

**Reading today's `infomon.rb` and concluding "they chose a database for in-memory state"
is reading the retrofit, not the decision.** PR #325 shipped **cacheless** (verified: zero
`cache|Queue|Thread` hits in its diff). Within ~5 weeks they added a RAM hash, an async
write queue, a barrier flush, and a mutex.

**The shipping design is: RAM is the model, SQLite is the durability tier.** Copy the end
state, not the shape.

**And "stringly-typed keys are flexible" was never even correct** — four normalization bugs
in one three-line function over three years, plus a namespace collision (#586). Every one
is a compile error in Rust. **The KV encoding moved the type system into string
concatenation and the strings drifted.**

### TRAP 4 — eohunter's `World` facade shape

**156 methods, 103 of them one-line `def x = ::CONST` delegations, exist because RSpec
cannot stub a hard-coded Ruby constant.** The author confirms it
(`architecture.md:108-111`).

**In Rust the read-only guarantee is `&` and the injectability is a trait.** Porting the
facade *shape* yields 156 pass-throughs the borrow checker already guarantees for free —
pure ceremony that then **accretes** (596 → 982 lines in seven days). **The rules transfer
fully; the implementation is Ruby-specific scaffolding.**

*(Narrowed per the judge: `def clock = Time` is genuine time injection and the rescue-to-safe-default
rule at that seam is a real boundary. The trap is the bare-constant accessors specifically.)*

### TRAP 5 — the 50 ms polling loops

`events.rb:70` (`sleep([remaining, 0.05].min)`), `actions.rb:341`, `actions.rb:361`. This is
a workaround for Ruby's GVL and the cost of condvar plumbing around Lich's downstream hook
thread, and it silently sets a **50 ms floor on every confirmation**. Rust gives real
`Notify`/channel wakeups at microsecond cost. **Copying the constant imports a latency floor
with no cause in Cena.** Same shape as the 58µs regex artifact.

### TRAP 6 — the 2-second heartbeat, flock election, and the whole active-sessions transport

`lifecycle.rb:32-33` states the heartbeat exists for *"detecting service-owner failover
quickly enough for multi-session use"* — i.e. noticing that the process holding the flock
died. **100% a process artifact.**

The flock-election / ephemeral-port / discovery-file design is **genuinely excellent
reasoning** (`active_sessions.rb:22-30`) that is **entirely inapplicable** — it solves "who
is the server when any peer might die," a question Cena cannot ask. **Admire it; port none
of it.** Same for the Windows sharing-violation retry ladder.

### TRAP 7 — the `SQLite3::BusyException` storm as a preview of Cena's contention

**It inverts.** 25 OS processes taking exclusive **file** locks under DELETE journaling is a
multi-process artifact; the fix is WAL. A single binary with one writer task has an ordinary
in-process mutex. #1319's own body: *"~1 write every 2.4s across 25 processes, each holding
the lock for <1ms."* **#1319 is evidence FOR collapsing to one process.** Keep the "25
concurrent sessions" scale recalibration; drop the contention inference.

### TRAP 8 — the 26,825-line AppCore number

**It is a directory total, not a file total**, across 29 files, and 4,000+ of it is test code
the author deliberately relocated *into* the tree (`6381d0cf` moved every split parent's
`mod tests` into a `tests.rs` child). **Reading it as evidence that enforcement failed
inverts the actual story** — the enforcement is *why* the number is spread across 29 files
instead of concentrated in one. **Anyone repeating this analysis from directory line counts
will reach the opposite of the correct conclusion.**

### TRAP 9 — the caps' absolute values

**2100 / 3600 / 800 are specific to Vellum's files at a moment in 2026-08.** Copying the
numbers is cargo-culting. What transfers is (i) that they only ever went **down**, and (ii)
the `CLAUDE.md` conduct rule attached to them.

**And this document nearly committed the same error itself:** an earlier draft proposed
*"Cena's session-state struct file stays ~1,500 lines."* That number is calibrated off a
**single-session** Rust struct where each doc-commented field costs ~3.6 lines (105 fields =
383 lines). **Cena's 3+ session struct blows that budget on field declarations alone.** Keep
the rule; **set the number empirically at Cena's first split, as Vellum did.**

### TRAP 10 — "the trait has one implementor, so traits are bad"

The `Frontend` trait failed because it was written **before the second frontend existed**
and specified the **wrong axis** (a polled loop). **The lesson is about timing, not traits.**
Cena, with four frontends planned from the start, is in a genuinely different position.

### TRAP 11 — `catalog_access_goes_through_the_seams` ported naively

It bans **Rust symbol names** with a path allowlist. **A Lua-facing seam is reached by
string keys from script code, which a symbol-ban grep cannot see at all.** Cena needs a
registry-of-exposed-names test or a single Rust chokepoint the grep *can* police.

### TRAP 12 — Ox's permissiveness, in reverse

This one is a trap **in the other direction**: a Rust rewrite will *silently lose* a
capability nobody wrote down as a feature. `games.rb:1054-1058`: *"Ox is a permissive
parser: it handles Simu's not-quite-XML stream without the clean/retry dance REXML
required."* Plus `repair_malformed_attributes_and_reparse` for two specific server
malformations, and `convert_special: false` with a documented entity-corruption history.
**A strict Rust XML parser will reject this stream.**

### TRAP 13 — typed frames make desync WORSE, not better

`check_stream_desync!` (`games.rb:1097-1103`) promotes truncation-class parse errors to
`GameStreamDesyncError`, logs, and **resets `XMLData`** rather than killing the server
thread. **Typed state accumulates; text does not.** A desync that would show as garbled
prose in Lich shows as *silently wrong typed state* in Cena, persisting indefinitely.
**Cena needs an explicit detect-and-reset design.**

---

# Right for anyone

The opposite category, and equally valuable: **decisions that encode real domain knowledge
about these games or about client architecture generally.** Cena should keep these
**without re-deriving them.**

### The send ladder and the skipped/failed distinction

`actions.rb:129-140`. **The watchdog must count wire events, not decisions.** A gate that
refuses returns `:skipped`, not `:failed`, because by construction *"the game heard
nothing."* Before this: *"a stun or an unaffordable technique read as five failures in five
ticks and stopped a live hunt in about a second with nothing on the wire."*

**Four independent implementations converged on this** — eohunter's ladder, bigshot's
`bs_put`, Lich's `fput`, and PR #1587 which upstreamed it. That is domain truth.

### Arm before send

`actions.rb:310-315` and, independently, Lich's `issue_command`. Subscribe **before**
sending, or you lose fast responses and wait forever on an event that already happened.
**The most common bug in game automation.** Any Cena request/response API must be
arm-then-send; a naive `send_and_wait(cmd, pattern)` reintroduces the race in every script.

With the author's own caveat kept (`architecture.md:123-126`): arming *"does not itself
prove that this command caused the event"* — payload correlation remains necessary.

### Every wait returns a reason, never a bare bool

`confirmed` · `timeout` · `interrupted` · `dead` · `cancelled`, plus an interrupt predicate
so a stopping engine cannot hang. This fixes Lich's inconsistent failure values
(`matchtimeout` → `false`, `dothistimeout` → `nil`, `fput` → `false`-or-symbol).

### Hazards are matched on the object list, never on room description text

`world.rb:894-899`: *"**never on room description text: 'mist' and 'fog' are scenery in
hundreds of rooms, and treating them as hazards would have us fleeing half the map.**"*

And `world.rb:915-920`: creatures are excluded from the hazard scan because *"a vine, web or
cloud creature stays on the npc list after it dies, so the flee trigger never cleared and
**the hunter fled the room for good**."*

**Two live bugs encoded as a four-lambda table.** This looks like an arbitrary hardcoded
list; it is a **scar**. Carry it into Cena's Rust game model where every script inherits it
instead of each rediscovering it.

### Permanent vs transient refusals

`actions.rb:86-92` `PERMANENT_REFUSALS`. *"You don't seem to be able to move your legs"* is
matched by Lich's **transient** rung (`global_defs.rb:1760`), so a severed leg got the
command resent five times.

**Judge's nuance, accepted — half of this is domain knowledge and half is a workaround.**
The knowledge transfers: *a severed limb is a permanent refusal.* The **shape** does not:
an override list patching a wrong classifier. **Cena owns its classifier and should
classify correctly at source** rather than carry a patch list.

### Presence ≠ connected

`lifecycle.rb:224-229`, and this is a **named postmortem**, not schema cruft:

> *"This method exists because MahtraDR's shutdown testing demonstrated that immediate
> unregister is too blunt an API signal: it hides a still-running Lich process from
> ActiveSessions while scripts, saves, socket closeout, and database closeout may still be
> executing."*

**The cause transfers exactly.** A Cena session whose game socket dropped but whose Lua
scripts are still draining teardown must be **visible-but-not-connected**, or peer sessions
make bad decisions. Same category: the composite
`connected: @connected && (@listener_port.nil? || @listener_connected)` (`lifecycle.rb:333`)
— computed **centrally** so no consumer forgets to AND the two booleans. Cena has the
identical shape (game socket vs frontend attached).

### Readiness is the first `<prompt>`

Issue #1616 / PR #1617. `100.times { sleep 0.1; break if XMLData.indicator['IconJOINED'] }`
looked like a deliberate readiness wait. It was a **truthiness bug** — indicator values are
the strings `'y'`/`'n'` (`xmlparser.rb:789`), **both truthy in Ruby** — costing every
attaching frontend a flat 10s. The replacement rule **is** domain knowledge:

> *"Readiness is the first `<prompt>`. In both games' login feeds it follows everything the
> push describes; `IconJOINED` and `<endSetup/>` arrive **before** spell, hands, and room
> exits."*

**Cena should encode "first prompt = state is complete" as a protocol fact.**

### Cross-call markup buffers are a protocol fact, not sloppiness

`markup.rb:23-25`: *"sf_to_wiz and strip_xml both carry unterminated markup across calls,
because **a pushStream element can be split over two reads**."* And `games.rb:432-435`:
*"clear it here so a fragment left open before a reconnect/session reset does not bleed
into the next session."*

`$sftowiz_multiline` looks like global-variable sloppiness. **It encodes a hard protocol
fact.** Cena's Rust parser needs exactly this state machine — **owned by the session**, so
reconnect drops it structurally rather than requiring a remembered global clear.

### Unknown tags must stay visible

Vellum, `src/parser/text.rs`: *"Unknown names indicate new server markup and are passed
through as visible text (never silently dropped) **so protocol changes announce
themselves**."* Strictly better than Lich's, which loses unmodelled content into prose.

### Delete the field rather than sync it

Vellum, `7a7145e2`: *"**two fields that must agree will drift again, and deleting turns
every reader into a compile error.**"* Earned by a duplicate `server_time_offset` that
nothing assigned for **ten months**, inflating 49.8% of countdowns by ≥2s across 6,373
measured prompts.

### Binding is identity, not name

VellumFE's window redesign: *"the governing invariant every phase: **binding is
identity**."* Fixed real bugs (duplicate windows; a TUI progress-bar colour fallback keyed
on window *name*, `tui/sync.rs:647`, re-keyed to `data.id`). The wire confirms why:
per-entity dialog ids like `injuries-10154507` ("Zoleta's Injuries") exist, **so any
name-keyed scheme collides the moment two players are in the room.** Key window identity on
binding from the first line of window code.

### The window decomposition is the protocol's own factorization

`<w id vis frame location panel x y width height open detach ts/>` — the redesign spec calls
it *"the Rosetta stone."* Lich logs captured **Wrayth's own** `<!-- CLIENT --><stgupd>`
layout sync, showing Wrayth itself models a window as *binding-id + kind + zone + frame +
rect + open/vis*. Vellum's `WindowBinding` + `ViewKind` + `PlacementHint` converged on the
same factorization independently. **Adopt it; do not re-derive it.** It is what the
protocol's own author chose, and it is the shape the wire will keep handing you.

### Design against a wire corpus, not against documentation

11.4 GB of real Lich logs, 2,368 files, 8 characters — which found `exposeDialog` ×4,265,
`deleteContainer` ×7,559, `exposeContainer` ×2,714, **all silently dropped by the live
parser.** Cena's protocol layer should be designed against a census, not StormFront docs.

### Follower-push, never leader-pull

eohunter, `hunting-engine-plan.md:1033-1040`: *"every tick each follower pushes one
`Report` ... and pulls its `Order`s, so **the leader never makes a remote call and cannot be
stalled by a follower**."* Right in-process too — it prevents one session's scheduler from
blocking on another's.

### Reap a dead peer without its cooperation

Lich's PID-liveness sweep (`registry.rb:121-130`) is **positive** about reaping a crashed
peer. Cena must keep the property via supervisor / `JoinHandle` observation, not drop it as
transport accident.

---

# What Cena does differently, and why

The five ways Cena's situation genuinely differs, and what each one buys.

### 1. Rust, not Ruby

**Buys:** the seal is real. `Lich::API`/`InternalAPI` is a stated architectural intention
that Ruby cannot enforce — `lifecycle.rb:280-282` reaches past `private` via `send`, with a
comment admitting it. In Rust that is `pub(crate)`.

**Buys:** the whole class of key-normalization bugs (Decision 2) becomes compile errors.

**Buys:** `RegexSet`/`aho-corasick` invert the combat parser's union-splitting conclusion.

**Costs:** a strict XML parser rejects Simutronics' not-quite-XML (Trap 12). Permissiveness
must be a deliberate choice, written in from the start.

### 2. One binary, not proxy + frontend

**Buys:** no `markup.rb`. No `sf_to_wiz`. No capability probing, no frontend launching or
locating, no hosts-file/SAL redirection, no eight-frontend adapter set. `01-architecture.md`
§5a estimates ~400–600 lines plus the launch subsystem, and it is right.

**Buys:** no flock election, no discovery file, no auth token, no heartbeat, no
sharing-violation retry ladder — ~900 of active-sessions' 1,643 lines.

**Costs:** the process boundary was doing real work. Crash isolation and preemptive
containment of a runaway must now be **designed in** (Decision 5.6), and the Rust/Lua
recovery is good but not total.

**Does NOT buy:** simpler *inbound* parsing. The game's XML/GSL is Simutronics' protocol,
unmovable, alongside EAccess auth. **The freedom is strictly outbound.**

### 3. Cena owns both sides of the frontend wire

This is the deepest difference and the plan under-uses it.

**Buys:** the observe/filter split (Decision 4.6). Lich cannot have it — its filter chain
*is* its forwarding path.

**Buys, and the plan does not mention it at all: the wire becomes bidirectional and
structured.** Lich scripts already inject dialog XML into frontends (UberBar); Vellum
invented `vellumTimer`/`vellumCmd`/`vellumImg` to do the same. **Cena's `Frame` vocabulary
must be an *emit* vocabulary too** — Lua constructs a dialog, a timer, an image, a window,
and the UI renders it, with no XML anywhere. **That is the single largest capability Cena
gets for free from owning both sides.**

**Buys:** `$frontend` can go. `01-architecture.md` §5a is right to reject the census's
recommendation to expose it — scripts branch on identity to ask a **capability** question.
Under a single frontend the identity is a *lie* scripts would silently mis-branch on.

### 4. Mobile

**Buys nothing; costs everything, and that is the point of making it a design input.**

It decides the concurrency model (coroutines, not thread-per-script) — which in turn
creates the runaway-containment obligation (Trap 1's real home). It forces L0–L5 to compile
for Android/iOS from day one, enforced in CI rather than discovered at port time. Vellum
already proves the shape and enforces it (`core_is_android_safe`).

### 5. Multi-session, with no legacy corpus

**Buys:** the session handle instead of globals, with no compatibility obligation forcing a
veneer. Lich *knows* the modular architecture is underneath — `parse_args` is a bridge to
`Lich::Common::ArgParser` — and still must ship the veneer. **Cena gets to ship only the
module.**

**Buys:** per-session event buses, which removes eohunter's single-instance prohibition.

**Buys:** freedom to drop `matchwait`/`goto`/`match_stack`/`WizardScript` outright.

**Costs:** everything Vellum's clean architecture scorecard does *not* validate. **INFERRED,
medium confidence:** every rule in Vellum's `tests/architecture.rs` is about layer, frontend
or platform separation. **None is about session isolation**, because there is exactly one
`AppCore` per process. Cena's 3+ concurrent characters add a boundary class Vellum never had
to enforce and therefore offers **no evidence about**. **Do not read Vellum's clean
scorecard as evidence that Cena's harder problem will be equally tractable** — Seam C
(snapshots-never-handles) is unvalidated by anything in either tree.

---

# Corrections to `01-architecture.md`

Concrete and specific. Where a decision was adopted uncritically and did not survive
interrogation, it is marked so plainly.

### C1 — §0.1's stated reason is wrong (the decision is right)

**Current** (`:20-24`): *"Lich hands scripts raw text lines and lets each script parse. That
single choice causes most of its structural problems."*

**Problem:** inaccurate. Lich parses to typed state (`XMLData`, `GameObj`, `Char`) **before**
any hook or script runs (`games.rb:975`, `:1068`), and feeds scripts line-split text via a
**separate, non-mutating** path (`games.rb:1089-1094`).

**Replace with:** the real cause is that **Lich's `DownstreamHook` filter chain and its
frontend-forwarding path are the same pipeline** (`games.rb:1149-1181`). Cena removes that
bug class by **separating the seams**; types are a second, independent improvement. **Keep
the parse-first decision — it is right.**

### C2 — §0.1 must add the observe/filter split

Add two distinct APIs, not one hook:

- `session:observe(frame)` — cannot mutate, no ordering hazard, parallelizable. **Offer
  multiple granularities (frame / line / raw chunk)** or consumers get driven into the
  filter path by a framing mismatch, exactly as Lich's `Combat::Tracker`,
  `Combat::Messages` and `xmlparser` were.
- `session:filter(frame) -> Option<Frame>` — presentation only, local UI only, **never**
  other scripts' input.

**And specify deterministic filter ordering.** Two Lua scripts filtering presentation still
compose order-dependently; PR #1621's priority machinery shrinks but does not vanish.

### C3 — §0.1 must add: presentation-state frames are non-suppressible by construction

`QUIET_STATE_TAGS` (`util.rb:79-140`) is a **hazard register, not a solved problem** — its
own comment says pushBold/popBold and pushStream/popStream *"can be added as its own entry"*
when confirmed. Types alone do not stop a Lua filter dropping a `StreamOpen`/`StreamClose`
pair. **Make the invariant explicit.**

### C4 — §2: replace `Frame::Unknown(RawTag)` with Vellum's design

`Unknown` is a **typed dead-end** — a Lua script receiving it can do nothing. Vellum ships
the better answer (`src/parser/text.rs:174-195`, `parser.rs:1036-1051`): a sorted
`KNOWN_WIRE_TAGS` binary search splitting **known-but-unhandled** (debug-log, swallow) from
**genuinely unknown** (**render as visible text**, so protocol changes announce themselves).

**Cena:** `Frame::Text` with `raw: Option<String>` provenance, plus
`Frame::UnknownTag { name, raw }` that renders as text, logs at warn, and **is matchable
from Lua by tag name.** Add `session:on_raw_line(fn)` as a documented, rate-limited
compatibility hatch, never on the UI path.

### C5 — §2 must add a stream-desync design

Not currently addressed. **Typed frames make desync worse than text**, because typed state
accumulates silently. Port the shape of `check_stream_desync!` (`games.rb:1097-1103`):
detect truncation-class parse failure, log, **reset the typed state**, do not kill the
session.

### C6 — §2/L2 must specify a permissive parser

`games.rb:1054-1058` documents Ox tolerating *"nested quotes, missing 'd' end tags"*; 
`repair_malformed_attributes_and_reparse` (`games.rb:1119-1131`) handles the `settingsInfo`
"space not found" bug and same-quote-in-quoted-value (`title='Tsetem's Items'`);
`convert_special: false` has a documented entity-corruption history. **A strict Rust XML
parser will reject this stream.**

### C7 — §0.1 / §2 must state that the wire is bidirectional

**Entirely absent from the plan.** Lich scripts inject dialog XML (UberBar); Vellum invented
`vellumTimer`/`vellumCmd`/`vellumImg`. **Cena's `Frame` vocabulary must be an emit vocabulary
too** — Lua constructs dialogs, timers, images and windows; the UI renders them; no XML
anywhere. This is the largest free win from owning both sides of the wire.

### C8 — §Seam B (`:177-178`): the multi-session claim is backwards

**Current:** *"`world` is an instance, never a global. Three characters means three worlds.
**This is what makes multi-session free rather than retrofitted.**"*

**Problem:** `World` is an instance, but **`Events` is a process-wide singleton**
(`events.rb:118-123`), and eohunter's shipped remedy for a second instance is **refusal to
start** (`eohunter.lic:127`, `spec/eohunter/single_instance_spec.rb:5-10`). **The design the
plan cites as its multi-session foundation has "refuse to run twice" as its multi-session
story.**

**Replace with:** the bus is **owned by the session**, alongside its `World`. Note honestly
that this is a **Lich-imposed constraint** (one Ruby process, `load`-based script reloading
forcing `remove_const` at `engine.rb:29`), not a design failure — the author had no cheap
alternative.

### C9 — §Seam B: split game facts from engine telemetry

Only a small fraction of eohunter's `Events.emit` traffic is parsed game facts; the rest is
engine narration recorded by an `:any` subscriber (`controller.rb:668`). **Conflating them
in Rust means every telemetry emission pays the typed-event tax, and the `:any` subscriber
becomes a cross-session information leak across the very boundary Seam C protects.**

### C10 — §Seam B: adopt `world`'s rules, not its shape

`world.rb`'s 156 methods (103 bare-constant delegations) are **RSpec scaffolding**. In Rust
the read-only guarantee is `&` and injectability is a trait. **Porting the shape yields
ceremony that accretes** — 596 → 982 lines in seven days.

### C11 — §Seam B: add a fourth seam

eohunter grew `Engage::State`, `Cleanse::State`, `Maintain::State`, `Routines::State` —
four private classes for facts fitting **neither** bucket of its own discriminator
(*"momentary facts travel on the event bus; durable facts live in World"*,
`architecture.md:114`). These are fight- and room-scoped **latches**. **The taxonomy is
incomplete.** Add a per-session scoped `State` seam.

### C12 — §Seam C (`:222`): cross-session messaging is not "the biggest footgun"

**Current:** *"a cross-session message-passing API is genuinely needed, but it is also the
easiest way to destroy the isolation Seam C exists to protect."*

**Problem:** Lich's own maintainers **built it twice** — PR #1613 (read-only, 1,826 lines,
7,591 passing examples, p99 2.786ms) and PR #1618 (write-capable) — and **closed both
unmerged on 2026-09-14** for a reason that does not transfer: @calael's *"Still think that
this could be done in a lib. Any reason it should be in core? It's pretty big chunk of code,
and not all folks use it."* **Rejected as core bloat for a broad-community engine, not as a
bad idea.**

**Replace with:** **design the contract now** — receiver-issued preflight tickets,
request IDs bound to owner/peer identity plus a canonical argument digest, generation
fencing, idempotent resubmit, conflict on meaning-change-under-same-ID, and the
`Operations` vs `Leases` split — **because it constrains the session-actor boundary.
Implement when a second session actually needs it.** (@calael's bloat concern has a
Cena-shaped analogue: a seam built before a consumer exists is maintained without feedback.)

Also correct the reading of `Lich::API`'s zero callers: it is the **surviving read-only
slice of a rejected proposal**, a deliberate reversal — not abandonment.

### C13 — §Seam C: keep supervisor-observed liveness

The plan deletes the heartbeat correctly, but Lich's PID sweep (`registry.rb:121-130`) is
**positive** about reaping a crashed peer **without its cooperation**. Cena must keep the
property via `JoinHandle`/supervisor observation.

### C14 — §5's coroutine decision must carry a containment design

**The plan's own footgun, currently unstated.** Ruby gets free preemption from `Thread#kill`;
coroutines do not. Add to Phase 5 (`:505`):

- `lua_sethook` / `LUA_MASKCOUNT` interrupt checking a per-script pause+kill flag
- a cancellation token at **every** await point (this is where Lich has already moved —
  `check_execution_guard!`, `execution_sleep`, `script.rb:3005-3017`)
- a per-session `lua_Alloc` memory budget
- one task per session; `catch_unwind` at the session boundary plus supervisor restart

**State the limits honestly:** `LUA_MASKCOUNT` does not preempt a blocking FFI call or a
long-running C function; `catch_unwind` is defeated by `panic = "abort"`; Rust-side
allocation failure still aborts. **This buys back most of the process boundary, not all of
it.**

### C15 — §5's "3+ characters" should be recalibrated

PR #1319 documents **25 concurrent character sessions** in production. **But do not import
its `BusyException` contention as a preview** — that is a multi-process file-lock artifact
and is evidence *for* collapsing to one process.

### C16 — §7's causal claim is backwards

**Current** (`:426-428`): mechanical enforcement *"is why its layering survived 310K lines."*

**Problem:** Vellum's first architecture test landed 2026-07-05, nine months into the repo,
at roughly line 250K — **after** the author hand-split a 9,227-line `impl`. The facade caps
landed 2026-08-09.

**Replace with:** **enforcement ratchets a refactor you already did; it does not do the
refactor for you.** Also note the honest record length: dependency rules ~2.5 months, facade
caps **six weeks** of never being raised. Encouraging, not proof.

### C17 — §7 must add rule 8: per-file line caps

§7's seven rules are **all dependency rules**, which Cena's crate workspace makes **free —
not sufficient**. **No crate graph can express "this file holds only the struct and the
dispatcher."** The cap is the one rule that actually constrained AppCore.

Add: per-file caps on every facade/dispatcher parent; *"move code down, don't raise the
cap"* in `CLAUDE.md`; and a test that fails on a cap **increase in the diff**, not just a
breach. **Do not copy Vellum's numbers** — set them empirically at Cena's first split.

**And do not reflexively cap the command dispatcher.** `commands.rs` is 5,132 lines and
deliberately uncapped; a big flat dispatch table is the right shape for a command surface.

### C18 — §7 must add single-owning-field tests

Vellum's `server_time_offset_has_a_single_owning_field` caught a duplicate field that
nothing assigned for ten months, inflating 49.8% of 6,373 measured countdowns by ≥2s.
**Cena's equivalents: server clock offset, roundtime, current-room id, active-session
handle.** From commit one. The rule: *"delete the field rather than sync it."*

### C19 — §7's seam test needs a Lua-aware mechanism

`catalog_access_goes_through_the_seams` bans **Rust symbol names**. Cena's Seam B is reached
by **string keys from Lua**, invisible to a symbol grep. Use a registry-of-exposed-names test
or a single Rust chokepoint the grep can police.

### C20 — §6/§9 should state the Lua API design explicitly

Adopted from Decision 1, currently only implied:

- **Keep the verb names** as methods on the session handle — `sess:put()`, `sess:echo()`,
  `sess:respond()`, `sess:pause()`. Twenty-five years of muscle memory; the *implicitness*
  is the defect, the *vocabulary* is domain knowledge.
- **Drop `matchwait`/`goto`/`match_stack`/`jump_label` outright**, and say so. The census
  already concluded *"porting those few scripts by hand is cheaper than implementing a
  goto-label FSM in Lua"* (`07-script-corpus-api-census.md:763-764`).
- **Prune on the census:** 64 of 208 globals have zero corpus callers; `global_defs.rb:2314`
  is headed *"## Alias block from Lich (needs further cleanup)"*. Real API ≈ 144 names; the
  top 30 carry the traffic.
- **Do not put the testability argument in the rationale.** It is a Ruby `TOPLEVEL_BINDING`
  artifact (Trap 2) and near-zero for Cena.

### C21 — §3 (Phase 3) should specify the state model shape

From Decision 2: typed named fields for the ~120 closed-vocabulary values; a
`BTreeMap`-plus-typed-accessor hybrid **only** for the genuinely open sets (status
conditions, PSM ranks), copying Vellum's `src/core/state.rs:340-410`; `updated_at` as a
per-group `Option<SystemTime>` on the struct; persistence as a **serde snapshot**, not a
storage model; **no generic `state.get("key")` exposed to Lua** — the census found exactly
one corpus call that reads an arbitrary Infomon key, across 456 scripts.

**And budget for the sync cost.** `Infomon.sync` is 15 blocking commands at `timeout: 5`
(`infomon/cli.rb:20-37`) — 30–60s of game traffic, requiring Shroud of Deception to be
dropped first. **Cena pays this too.**

---

# Credit where due

It would be easy to read this document as a teardown. It is not one, and the evidence does
not support one.

**Lich's authors are the fourth custodian of a fifteen-year-old scripting vocabulary**, and
they carry a live corpus of hundreds of community scripts and eight third-party frontends
they cannot change. Nearly every decision interrogated here was **the correct call given
what they had**. Several are correct for anyone. The ones Cena reverses, it reverses because
Cena is in a different situation — not because they were wrong.

Specific things worth naming:

- **The flock-election design** (`active_sessions.rb:22-30`) is genuinely excellent
  distributed-systems reasoning. It solves leader election among mutually-untrusting peers
  where any peer may die, using only the kernel's advisory-lock semantics. Cena will port
  none of it, and that is a statement about Cena's situation, not about its quality.
- **`Script.current_without_pause`** is a deliberately-named escape hatch for seams that must
  not add a pause checkpoint. Someone thought carefully about a subtle re-entrancy problem.
- **`ScriptExecutionGuard`** is Lich walking, unprompted, toward exactly the
  cancellation-token model Cena needs — in Ruby, where it is much harder.
- **PR #1613/#1618** are 4,973 lines of careful coordination design with a full test suite
  and a measured p99, closed by their author one day after opening because a reviewer made a
  fair scope argument. That is good engineering **and** good judgement about someone else's
  codebase. Cena takes the design for free.
- **eohunter's `actions.rb`** is the best-argued piece of code in either tree. The
  skipped/failed distinction, the refusal ladder, arm-before-send, and the honest caveat that
  arming *"does not itself prove that this command caused the event"* are all things a less
  careful author would have gotten wrong and never noticed. It was upstreamed into Lich core
  (#1587) because it was right.
- **eohunter's hazard table** (`world.rb:894-920`) is four lambdas that look arbitrary and
  are in fact two production bugs, each written down with the reason. *"Treating them as
  hazards would have us fleeing half the map"* and *"the hunter fled the room for good"* are
  the kind of comments that save the next person a week.
- **VellumFE's line caps have never once been raised** in six weeks and several hundred
  commits, with four files sitting at 96–99% of a cap. That is discipline, from one person,
  with nobody watching.
- **VellumFE's window redesign** was driven by a census of 11.4 GB of real wire logs rather
  than by documentation, and it found four tag families the live parser was silently
  dropping. That method is worth more than the redesign it produced.
- **The `server_time_offset` postmortem** is a ten-month-old bug found, measured across
  6,373 prompts, fixed, and then **converted into a test so it cannot recur.** That last step
  is the one most people skip.

Cena starts where it does because these people did the work. The most useful thing this
document does is distinguish **what they learned** — which is free, and should be taken —
from **what their tools forced on them** — which is not ours to inherit.
