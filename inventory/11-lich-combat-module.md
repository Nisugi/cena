# Lich's combat module, measured before porting

**Status:** research, 2026-09-20. Every number below was produced this session by
the command beside it, against the **live** Lich at `C:\Gemstone\lich-5` (the
author's own install), not the `reference/` snapshot. The two differ in exactly one
file:

```sh
diff -rq reference/lich-5/lib/gemstone/combat C:/Gemstone/lich-5/lib/gemstone/combat
# -> only async_processor.rb differs; processor.rb is 2,436 lines in both
```

> **AUTHOR, 2026-09-20:** *"I've spent years perfecting this thing and it's pretty
> amazing."* The def comments trace their lineage to `ctparser` (`flares.rb:5`,
> `sequences.rb:5`: *"Converted from ctparser/FLARE_DEFS"*). Git on `lib/gemstone/combat`
> shows 18 commits from 2026-01-06 to 2026-09-16; the design predates the module.
> This is the author's reference implementation, to be **ported faithfully**, not
> redesigned.

---

## 1. Size and shape

| Layer | File | Lines | Role |
|---|---|---:|---|
| classifiers | `parser.rb` | 383 | per-line dispatch into the def families; stateless |
| **state machine** | `processor.rb` | **2,436** | the chunk FSM: attack → resolutions → damage → crit; owns every routing rule |
| plumbing | `tracker.rb` | 612 | `DownstreamHook`, prompt-bounded buffer, settings, thread hand-off |
| plumbing | `async_processor.rb` | 115 | one ordered worker thread fed by a `Queue` |
| messages | `messages.rb` | 180 | non-combat line families (disarm, ambusher, bolted) as events |
| persistence | `recorder.rb` | 1,061 | SQLite, seven tables |
| **defs** | `defs/*.rb` + `.yaml` | **4,673** | the pattern grammar, 15 files |
| reports | `scripts/combat_stats.lic` | 1,570 | `cstats` — session, creature, flare, HP, accuracy reports |

```sh
wc -l C:/Gemstone/lich-5/lib/gemstone/combat/*.rb            # 4,787
wc -l C:/Gemstone/lich-5/lib/gemstone/combat/defs/*          # 4,673
wc -l C:/Gemstone/lich-5/scripts/combat_stats.lic            # 1,570
```

**9,460 lines** in the engine and defs, which matches `inventory/08`'s figure. Plus the
script. The processor is *"the single densest logic file"* (`inventory/02`), and the
measurement bears that out in an unexpected way:

```sh
awk 'NR>=361 && NR<=1830' processor.rb > /tmp/pe.rb    # parse_events
wc -l /tmp/pe.rb                                        # 1,470
grep -cE '^\s*(if|elsif|unless|when|case)\b' /tmp/pe.rb # 97 branches
grep -cE '^\s*#' /tmp/pe.rb                             # 768 comment lines
grep -oE '20[0-9]{2}-[0-9]{2}-[0-9]{2}' /tmp/pe.rb | sort -u | wc -l   # 11 dated citations
```

**52% of the FSM is comments**, and eleven distinct hunt-log dates are cited inside
it. Nearly every branch records the log line that made it necessary. That is what
makes a 1,470-line state machine portable: the *why* is written down beside the
*what*.

## 2. The def grammar

Definitions are `Struct`s with an ordered list of patterns, one family per file:

| Family | File | Defs | What it recognises |
|---|---|---:|---|
| attacks | `attacks.rb` | 93 | initiations: `You swing ... at X!`, cmans, maneuvers; 3p forms capture `attacker` |
| spells | `spells.rb` | 55 | spell initiations, spliced into `ALL_ATTACKS` (Divine Wrath alone has ~35 deity forms) |
| flares | `flares.rb` | 100 | weapon scripts, enchants, GEFs; each carries `damaging` / `aoe` / `spawns` |
| outcomes | `outcomes.rb` | 14 | why nothing landed: miss, evade, block, parry, warded, resisted, fumble, hindrance |
| **resolutions** | `outcomes.rb` | 8 | the roll lines — see §2a |
| statuses | `statuses.rb` | 30 | **add/remove pairs** per status: blind, immobilized, prone, stunned, webbed ... |
| spell losses | `spell_losses.rb` | 15 | 3p wear-off lines pinned to a spell number, *"wiki first, log pairing for what the wiki can't settle"* |
| assaults | `assaults.rb` | 5 | single-target multi-round brackets: flurry, barrage, pummel, guardant thrust, thrash |
| sequences | `sequences.rb` | 4 | multi-target brackets: mstrike, volley, natures_fury, earthen_fury |
| UCS | `ucs.rb` | 5 patterns | position tier, tierup, smite |
| damage | `damage.rb` | 4 lists | `... N points of damage!` and its many shapes; a `damage` substring gate first |
| messages | `messages.rb` | by `CONTRACTS` | non-combat events with a typed payload contract each |
| supplements | `supplements.rb` | — | player-supplied YAML defs, compiled into the same structs |

```sh
cd C:/Gemstone/lich-5/lib/gemstone/combat/defs
for f in *.rb; do echo "$f: $(grep -cE 'Def\.new\(' $f)"; done   # 329 total
grep -ohE '/[^/]*\(\?<' *.rb | wc -l                                # 776 named-capture patterns
```

`inventory/09` counts **1,082** regexes across the def files by its own broader
command; the 776 here are those with a named capture. Both are the same corpus.

### 2a. The eight roll grammars

`outcomes.rb:375-430`, `module Resolutions`. Every named capture becomes a key:

| type | line shape | live count in the author's DB |
|---|---|---:|
| `as_ds` | `AS: +351 vs DS: +299 with AvD: +22 + d100 roll: +88 = +162` | 4,412 |
| `smr` | `[SMR result: 191 (Open d100: 13, Bonus: 56)]` | 3,764 |
| `cs_td` | `CS: 384 - TD: 419 + CvA: 13 + d100: 59 == 37` | 100 |
| `ssr` | `[SSR result: N (Open d100: N)]` | 81 |
| `uaf_udf` | `UAF: N vs UDF: N = N.N * MM: N + d100: N = N` | 6 |
| `activation` | `1d100: 30 + Modifiers: 375 == 405` (imbed/wand) | — |
| `maneuver_roll` | legacy `[Roll result: ...]`, kept as a fallback | — |
| `fear` | `FS: N - FD: N + FvP: N + d100(L): N = N` | — |

The `as_ds` die is not always d100 (`d\d+` — *"d80, d40 at higher tiers"* with True
Strike). The recorder keeps `roll` separate from `result` so `result - roll` is the
deterministic margin.

### 2b. Rust `regex` can compile all but one def

Rust's `regex` has no lookaround or backreferences, by design (linear time). Measured
against the defs:

```sh
cd C:/Gemstone/lich-5/lib/gemstone/combat/defs
for feat in '(?=' '(?!' '(?<=' '(?<!' '\1' '(?>' '\h' '\R' '\K'; do
  echo "$feat: $(grep -F -- "$feat" *.rb | wc -l)"; done
```

| Feature | Hits | Where | Portable? |
|---|---:|---|---|
| `(?<!` | 1 line, 2 uses | `statuses.rb:118` — `(?<target>.+?)(?<! dazed and)(?<! paralyzed and) is (?:knocked\|driven) to ... knees!` | **the one real case** |
| `(?!` | 1 | `supplements.rb:270` — a *file-path* filter, not a def | n/a |
| `\1` | 1 | `supplements.rb:224` — an error-message string | n/a |
| everything else | 0 | | |

So **one def of 329** needs the mechanism Cena's crit port already built for the
same problem: `crit/match_index.rs:111-143` compiles a positive pattern plus a
separate `exclusion: Option<Regex>` veto, because `slash_critical_table.rb:91` carries
`^(?!.*removes skull.)Blow to head.` The `knees` status is the identical shape —
match, then veto on ` dazed and` / ` paralyzed and`. The workspace's `regex` is 1.13.1
(`Cargo.lock`), which accepts the `(?<name>...)` capture syntax the defs use verbatim.

## 3. The state machine, honestly inventoried

### 3a. What persists across chunks

`processor.rb` module ivars, by assignment count:

```sh
grep -nE '^\s+@[a-z_]+ (=|\|\|=)' processor.rb | awk -F'@' '{print "@"$2}' | awk '{print $1}' | sort | uniq -c
```

| ivar | Role |
|---|---|
| `@held_cast` | a bare `cast` gesture whose spell result arrives in the **next** chunk |
| `@held_pre_flares` | flares announced before the swing they belong to, claimed by weapon next chunk |
| `@active_assault` | the open flurry/barrage/pummel bracket; its target is carried onto targetless rounds |
| `@death_watch` / `@death_announced` | creatures a chunk killed, confirmed against `<crtrStatus dead="1">` **after** the chunk's attacks emit |
| `@position_recovered` | **reset per chunk** — a stand-up must not suppress a knockdown crit in the next chunk |
| `@observation_batch_id`, `@deferred_emits`, `@delta` | emission bookkeeping and debug |

Five substantive cross-chunk facts. Everything else is chunk-local.

### 3b. What is chunk-local

`parse_events` (`processor.rb:361-530`) declares ~26 named locals before its loop:
`current_event`, `parse_state` (only two values: `:seeking_attack`, `:seeking_damage`),
`current_target`, `flare_ctx`, `pending_flares`, `spawn_pending`, `active_spawn`,
`pending_echoes`, `spawn_root`, `active_sequence`, `pending_resolutions` (an array —
*"the old single slot silently overwrote all but the last"*), `pending_ambush`,
`pending_redirect`, `chunk_dispels`, `chunk_deaths`, `cast_owner` (keyed **per victim**,
load-bearing), `foreign_latch`, `orphan_hits`, `orphan_outcomes`, `interrupted_own`,
`released_parent`, `pending_release_cast`, `deferred_casts`, `last_inbound_attacker`,
`single_hit_parent`, `nocked`.

Each has a comment saying which hunt log made it necessary.

### 3c. The routing rules the author calls hard-won

`docs/COMBAT_DEFS_ONBOARDING.md` §5, *"don't rediscover"*, verbatim:

1. Maneuver-class rolls (SMR/SSR/fear) **precede** their per-target line; AS/DS-class
   rolls **follow** their attack line. A roll arriving on a settled event belongs to
   the next target.
2. The bare-gesture `:cast` event is a wrapper when a spell-specific initiation
   follows in-chunk: rolls transfer, wrapper discarded, `via: :cast`.
3. A damaging flare's cursor closes once it has damage (or it steals the next
   swing's roll).
4. Inbound events carry no creature target **by design** and must never adopt one
   from a later link — *"emotes handed creatures their own damage."*
5. **No death defs, ever.** Death state is owned by `<crtrStatus dead="1">`; death
   messages are catalogued but never pattern-matched into events.

Rule 1 is visible in the first real blob pulled this session
(`2026-09-18_17-25-15.xml:43-50`): the `[SMR result: 191 ...]` line sits *above* the
tangleweed attack line it belongs to.

Rule 5 is measured: in that same 28,157-line log, `<crtrStatus ... dead="1">` arrives
**378 times across 76 distinct creatures**, against 52 death-shaped prose lines.

## 4. The recorder and the reports

Seven tables (`recorder.rb:1-60`), and the author's own database holds fifteen hunts:

```sh
python3 -c "import sqlite3; c=sqlite3.connect('C:/Gemstone/lich-5/data/GSIV/Nisugi/combat_stats.db'); \
  [print(n, c.execute(f'select count(*) from {n}').fetchone()[0]) for (n,) in \
  c.execute(\"select name from sqlite_master where type='table'\")]"
```

| table | rows | what one row is |
|---|---:|---|
| `sessions` | 15 | one hunt; opens on the first combat event, closes after 5 idle minutes |
| `creatures` | 1,161 | `(session_id, exist_id)`, with `killed_at` and **`kill_credit`** |
| `attacks` | 8,596 | one swing/cast/inbound/orphan, with **spawn-tree** links and attribution flags |
| `resolutions` | 8,363 | one roll line; `roll` kept apart from `result` |
| `flares` | 8,345 | one flare on an attack |
| `hits` | 9,337 | one damage fact with its crit denormalised: location, rank, wound rank, fatal, amputated |
| `statuses` | 13,533 | add/remove stream, attack-attributed by window |

**The attribution flags are the design.** `ours` is *"decided at write time: our own
outbound attack — not inbound, not a nearby player's, not on a foreign target, not an
unowned tick, not an orphan sink."* Measured:

| flag | attacks |
|---|---:|
| `ours` | 7,051 |
| `inbound` | 1,163 |
| `orphan` | 295 (**3.4%**) |
| `foreign_caster` | 44 |
| `unowned` | 30 |

And `kill_credit` says *how* each of the 1,023 kills was attributed:

| credit | kills | meaning |
|---|---:|---|
| `crit` | 821 | a fatal crit — ground truth |
| `window` | 144 | room-feed death inside an attack window |
| `last_own_hit` | 51 | no window; the last damaging attack was ours |
| `last_hit` | 2 | ...was someone else's |
| NULL | 5 | pre-migration |

**80% of kills are credited by a fatal crit**, which is the one signal that cannot be
wrong. The reports (`cstats`) are SQL over these: session totals, per-creature attack
trees, flare analytics, DS/TD readings, HP estimates, aim/accuracy/time-to-kill, and
session comparison.

## 5. The test corpus — larger than any port so far

| Source | Size | Notes |
|---|---:|---|
| `spec/lib/gemstone/combat/` | 21 files, **6,470 lines** | `processor_inbound_spec.rb` alone is 1,893 |
| `spec/fixtures/replay/` | **71 blobs** | one real-feed chunk per attack def, with an `# expect:` header |
| `spec/fixtures/` | 87 files total | includes named regressions: `cloak_of_shadows_interrupt.txt`, `dispel_on_nock.txt` |
| `logs/GSIV-Nisugi/2026/09/` | 242 files | the author's September; `.xml` is wire data |
| `combat_stats.db` | 4.9 MB | **a per-attack answer key** for the hunts it recorded |

```sh
ls C:/Gemstone/lich-5/spec/fixtures/replay | wc -l        # 71
wc -l C:/Gemstone/lich-5/spec/lib/gemstone/combat/*.rb    # 6,470
```

### 5a. The replay contract

`replay_spec.rb` drives each fixture blob through the real `parse_events` and reduces
the events to the header vocabulary:

```text
##### attack 5110a6c49c3f7711
# expect: attacks=attack | res=as_ds | flares=ensorcell | outcomes=none | dmg=1 | statuses=
You swing a perfect mithril war-hammer at <pushBold/>a <a exist="37507821" noun="construct">greater construct</a><popBold/>!
  AS: +351 vs DS: +299 with AvD: +22 + d100 roll: +88 = +162
   ... and hit for 20 points of damage!
   Torn muscle in <pushBold/>the <a exist="37507821" noun="construct">greater construct</a><popBold/>'s left leg!
```

`dmg` must match exactly; the set fields are a **floor** — every expected fact must
be present, extras are tolerated. That contract is directly portable: the fixtures
are raw feed lines with markup intact, which is exactly what Cena's parser consumes.

### 5b. The database as an oracle, with one caveat

Each DB session can be located in a log file by `started_at`:

```sh
# bisect each session's started_at against the log filenames' timestamps
7/15 sessions fully inside one log file; 1,100/8,596 attacks
```

The other eight span a log boundary because Lich opens a new log file periodically.
A replay oracle at full scale needs to concatenate consecutive logs; the seven
contained sessions are usable as-is.

### 5c. Not on this machine

The onboarding doc cites a **257,568-signature** deduplicated blob corpus at
`C:\Gemstone\dev\combat_corpus\` with a 92.7% full-match replay rate, and
`logs/examples/` as the fixture home. **Neither directory exists here**
(`ls` empty). The 71 curated fixtures are the slice of that corpus that shipped.

## 6. What Cena already has, and what it gets for free

| Cena piece | Where | What it replaces in Lich |
|---|---|---|
| the prompt-bounded chunk | `state/chunks.rs`, MEASURED 1,712/1,712 blobs | `tracker.rb:531-545`'s buffer and `<prompt time=` flush |
| crit tables with exclusion | `crit/`, 2,394 entries | `CritRanks`, and the lookaround mechanism §2b needs |
| the bestiary | `state/creature.rs`, 627 templates | `CreatureTemplate`, for `max_hp` estimates |
| `<crtrStatus>` classifier | `state/creature/status.rs`, 13 + 11 flags | `creature_base.rb`'s two flag tables |
| `Effects` with `expires_at` | `effects.rs` | the shape `STATUS_DURATIONS` timers need |
| `Frame::Prompt { time }` | `state.rs:395` | `prompt_time(chunk)` for `occurred_at` |
| links on every run | `Run { link: Option<Link> }`, `LinkKind::Exist { id, noun }` | `TARGET_LINK_PATTERN` and `BOLD_WRAPPER_PATTERN` re-scans |

Three of Lich's defended positions are **structural** in Cena:

- **Roster components never enter the chunk.** `streams.rs:152` pushes only
  main-stream `Frame::Text`; `Frame::Component` goes to `room.apply_component`
  (`state.rs:339`). Lich needs `ROOM_ROSTER_REFRESH` (`tracker.rb:449`) and a
  provenance rule to keep roster links from looking like targets.
- **Pronouns carry the creature's `exist`.** `plan/12` §3a verified `her` resolving to
  the shield-maiden's id; Lich fixed an attacker logged as `"his"` after a 2026-09-07
  hunt.
- **The four re-scanning bugs** `state/chunks.rs` already lists (the `"his"` attacker,
  the phantom `"grim gigas skald's"`, the doubled space, ten unattributed bleed ticks)
  are the class the typed frames remove.

### 6a. The one prerequisite: `ChunkLine` drops the links

`streams.rs:153-156`:

```rust
self.chunk.push_line(super::chunks::ChunkLine {
    text: line.plain(),
    bold: line.bold_fragments(),
});
```

`ChunkLine` keeps the plain text and the bold fragments and **discards every
`Link`**. `chunks.rs`'s module doc says *"a chunk here holds parsed lines and no
consumer needs a second parser"* — true of the pipeline, not yet of the type. A
combat consumer reading the chunk today could see `a grim gigas skald` and never learn
`exist="340826187"`.

**This is the first change**, and it is independent of combat: `ChunkLine` should
carry the `Runs` it was built from, with `text` and `bold` derived. `character/`'s
readers keep working; the links become reachable.

## 7. Two positions Cena's plans already take

- **`plan/06` §1.4:** *"Lich already does this: PR #1559 is 'combat module — event
  defs, observer emissions, replay-verified parsing, SQLite recorder.' ... Cena should
  build the recorder in Phase 1, not later — every session any developer plays becomes
  a test fixture."* And: *"a bug found in live play is reproduced as a replay fixture
  before it is fixed."*
- **`plan/06` §1.9, the trap:** Lich measured many small literal unions beating one
  big one (58µs vs 35µs/line). *"True in Ruby ... Rust's `RegexSet`/`aho-corasick`
  invert it. Never port a performance conclusion; port the discipline of measuring."*
  `PatternGate`'s *idea* — a cheap literal prefilter derived from each pattern — is
  worth keeping; its *numbers* are not.
- **`plan/12` §6a.4:** *"Highlights are the primary combat perception channel ...
  Typed events enable something Saga cannot do: highlight by actor and event class."*
  The consumer's output is events, not only a ledger.

## 8. What this inventory does not settle

Decisions that are the author's, listed so they are asked rather than assumed:

1. **Persistence.** The character store chose serde + JSON. The combat recorder is
   relational — seven tables, 8,596 attacks in fifteen hunts, and every report is a
   SQL join — which argues for `rusqlite`, a **new workspace dependency**.
2. **Where the FSM lives.** `plan/12` §3a's table puts consumers in *"cena-session and
   above"*; M3 put the `info` block consumer inside `GameState` (`cena-model`). The
   combat FSM holds five cross-chunk facts (§3a) and emits events; the recorder does
   file I/O, which `model_does_no_file_io` keeps out of `cena-model` regardless.
3. **First-pass scope.** The classifiers (§2, ~329 defs) are pure and testable in
   isolation. The FSM (§3) is one 1,470-line unit whose value is in its routing rules,
   and it cannot be half-ported — a swing with no roll routing is an orphan. The
   recorder and reports (§4) are a third unit.
