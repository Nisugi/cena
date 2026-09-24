# 33 — The guard vocabulary: bigshot's 87 words, evaluated

**Status: PROPOSED — for the author's review** (`plan/30` §7, M6a step 4). Nothing here is
built. The profile format and the importer (§7 of this doc) are designed from the verdicts, so
a verdict changed here changes them.

**Source.** `reference/scripts/scripts/bigshot.lic` at `7fd0b97` (2026-09-17), 10,246 lines.
Every row cites the line its branch is on. The live install at
`C:\Gemstone\lich-5\scripts\bigshot.lic` is 10,242 lines; the rows were read from the clone.

**Count.** MEASURED, 87 words: `sed -n 3197p bigshot.lic | tr '|' '\n' | wc -l`.

---

## 1. How bigshot reads a guard

A routine line is a command followed by guards in parentheses: `kweed (buff5 !hidden)`.
`command_check` (`:4217`) answers **"skip this step?"**, checking each guard in turn and
skipping on the first that says so. So all the guards on a line have to pass. Each guard
goes through up to four checks in order:

| Check | Line | Covers |
|---|---|---|
| **amount** | `:3204-3225`, run at `:4228` | a letter and a number: `h50`, `m20`, `mob3`, `tier2` |
| **buff time** | `:4240` | `buffN`, which looks up **the verb's own buff** in `:3228-3241` |
| **repeat delay** | `:4270` | `repeatdelayN` |
| **state** | `check_state_condition`, `:4296` | every other word, and `EB"…"`, `empowered30` and `thp20` |

**Three facts about bigshot that decide the design:**

1. **A misspelled guard is silently ignored.** `check_state_condition` ends in `else false`
   (`:4522`). An unrecognised word therefore never skips anything: `(hiden)` just runs.
   `"lying"` is a live example. It has a branch at `:4455`, but it is **not in the
   `:3197` regex**, so it can never be read. Hydra refuses an unknown word when the profile
   loads.
2. **Polarity is not consistent.** Most words mean **"run only when X"**: `(stunned)` skips
   unless the target is stunned. **Four mean the opposite, "run only when not X":**
   `frozen`, `prone`, `rooted` and `voidweaver` (`:4418`, `:4428`, `:4430`, `:4388`). So
   `(frozen)` means "the target is **not** frozen". `plan/30` §5 already records one such
   case, `buff5`. Hydra has one rule, **every guard names when the step runs**, and the
   importer flips these four.
3. **A `!` is not always a negation.** Six pairs mean two unrelated things:
   - `!506`/`!celerity` (`:4347`): not "Celerity is down", but "not in Celerity's last 3 s"
     (`timeleft <= 0.05`, in minutes).
   - `burst`/`!burst` and `surge`/`!surge` (`:4354`, `:4380`): the bare word reads a
     **buff**, and the `!` form reads a **cooldown**.
   - `mob`/`!mob`, `valid`/`!valid` and `tier`/`!tier` (`:3216-3225`): `<` against `>`,
     so both forms run at exactly N.

   Where Hydra keeps the idea, each meaning gets its own word.

---

## 2. Verdicts

The verdicts are **keep**, **rename**, **merge** (into the named survivor), **drop**, and
**later** (sound, but the model cannot answer it yet, so it is built when a profile needs it).

"Runs when" gives the meaning **after** the polarity is made uniform. Where that differs from
the word's reading, the row says so.

### 2a. Me: vitals and encumbrance (amount checks, `:3204-3225`)

In bigshot `h50` **skips** when health is below 50, so it means "run when health is at
least 50". The plain `!` form is a clean negation for these five, and they keep it.

| Word | Runs when | Hydra reads | Verdict |
|---|---|---|---|
| `h` | health % ≥ N | `vitals` health, percent | **rename** `health_at_least N` |
| `m` | mana ≥ N (points, not %) | `vitals` mana | **rename** `mana_at_least N` |
| `s` | stamina ≥ N | `vitals` stamina | **rename** `stamina_at_least N` |
| `v` | spirit ≥ N | `vitals` spirit | **rename** `spirit_at_least N` |
| `e` | encumbrance % < N | `character.encumbrance_percent` | **rename** `encumbrance_below N` |
| `essence` | shadow essence ≥ N | not captured (`CLAUDE.md`: the `resource` capture is an open M6 gap) | **later** `essence_at_least N` |
| `k` | **I am** not kneeling (`!k`: I am). Takes a number and ignores it | `status.known().kneeling()` | **rename** `self_kneeling`, with no number. Bare `kneeling` is the **target's** (§2d), and sharing it was the trap |

### 2b. Me: conditions and place

| Word | Line | Runs when | Hydra reads | Verdict |
|---|---|---|---|---|
| `hidden` | `:4396` | I am hidden | `status.known().hidden()` | **keep** (Nisugi) |
| `disease` | `:4394` | I am diseased | `status.known().diseased()` | **keep** |
| `poison` | `:4400` | I am poisoned | `status.known().poisoned()` | **keep** |
| `outside` | `:4398` | the room is outdoors (Lich: exits read `Obvious paths:`) | the room's exits label or `<roommeta>`. INFERRED: which one holds it is checked when built | **keep** |
| `splashy` | `:4507` | the room has the map tag `meta:splashy` | `cena-map` `Room::meta` | **keep** |
| `pcs` | `:4511` | no other player outside my group is here | `room.players` minus `group` | **rename** `alone` (bigshot's `(pcs)` means **no** players) |

### 2c. Me: effects

**21 words are one guard with its effect name built in.** Each is "is buff X up", with X
fixed by the word. Hydra keeps the four general forms and the importer turns each fixed word
into one of them. Adding an ability then needs no new word.

| Word | Line | Runs when | Verdict |
|---|---|---|---|
| `EB"…"` | `:4298` | a buff matching the text is up | **keep** `buff "<name>"` |
| `ES"…"` | `:4298` | a spell matching the text is up | **keep** `spell "<name>"` |
| `EC"…"` | `:4298` | a cooldown matching the text is up | **keep** `cooldown "<name>"` |
| `ED"…"` | `:4298` | a debuff matching the text is up | **keep** `debuff "<name>"` |
| `buff` | `:4240` | **not** within N s of the end of the verb's own buff. If that buff is down, it runs (`plan/30` §5) | **rename** `expiring "<name>" N` (runs when **not** expiring). The importer resolves the name from the verb through `:3228-3241` |
| `empowered` | `:4319` | no Empowered of +N or more is up | **keep** `empowered_below N` (Nisugi's `empowered30`) |
| `506` | `:4346` | Celerity (spell 506) is up | **merge** → `spell "Celerity"` |
| `celerity` | `:4356` | identical to `506` | **merge** → `spell "Celerity"` |
| `animate` | `:4348` | Animate Dead is up | **merge** → `spell "Animate Dead"` |
| `barrage` | `:4350` | Enh. Dexterity (+10) | **merge** → `buff` |
| `bearhug` | `:4352` | Enh. Strength (+10) **or (+20)** | **merge** → `buff "Enh. Strength"` |
| `burst` | `:4354` | **any** Enh. Dexterity is up; `!burst` = Burst of Swiftness cooldown **not** up | **merge**, two ways: → `buff`, and `!burst` → `!cooldown "Burst of Swiftness"` |
| `coupdegrace` | `:4358` | any Empowered is up | **merge** → `buff "Empowered"` |
| `flurry` | `:4360` | Slashing Strikes | **merge** → `buff` |
| `fury` | `:4362` | Enh. Constitution (+10) | **merge** → `buff` |
| `garrote` | `:4364` | Enh. Agility (+10) | **merge** → `buff` |
| `holler` | `:4366` | Enh. Health (+20) | **merge** → `buff` |
| `momentum` | `:4368` | Glorious Momentum | **merge** → `buff` |
| `pummel` | `:4370` | Concussive Blows | **merge** → `buff` |
| `rapid` | `:4372` | Rapid Fire | **merge** → `buff` |
| `rebuke` | `:4374` | Righteous Rebuke | **merge** → `buff` |
| `scourge` | `:4376` | Ardor of the Scourge | **merge** → `buff` |
| `shout` | `:4378` | Empowered (+20) | **merge** → `buff` |
| `surge` | `:4380` | like `burst`, for Enh. Strength / Surge of Strength | **merge**, two ways, as `burst` |
| `tailwind` | `:4382` | Breeze Archery Tailwind | **merge** → `buff` |
| `thrash` | `:4384` | Forceful Blows | **merge** → `buff` |
| `vigor` | `:4386` | Tangleweed Vigor | **merge** → `buff` |
| `voidweaver` | `:4388` | **no** Voidweaver buff is up (**inverted**) | **merge** → `!buff "Voidweaver"` |
| `yowlp` | `:4390` | Yertie's Yowlp | **merge** → `buff` |
| `justice` | `:4515` | I have a Swift Justice charge. Counted from two game messages (`:2827`) | **later**: needs a classifier for the charge count |
| `reflex` | `:4517` | Arcane Reflexes is up. Scanned from two messages (`:2760`) | **later**. UNVERIFIED whether the effects list shows it; if it does, **merge** → `buff` |

**On `"<name>"`:** bigshot compiles the text into a case-insensitive **regex** (`:4302`).
Hydra matches **the effect's displayed name, whole, ignoring case**, and does not use a
regex. Where bigshot relied on a partial match, a trailing `…` (`buff "Enh. Strength…"`) says
"starts with". The importer imports literal text as it is and holds any line whose text uses
regex metacharacters.

### 2d. The target: statuses

Read from the creature's statuses (`StatusName`, `state/combat/status.rs:24`), which
`<crtrStatus>` and the crit tables feed.

| Word | Line | Runs when | Verdict |
|---|---|---|---|
| `stunned` | `:4451` | the target is stunned | **keep** |
| `webbed` | `:4453` | webbed | **keep** |
| `sleeping` | `:4449` | sleeping | **keep** |
| `calm` | `:4437` | calmed | **keep** |
| `disoriented` | `:4439` | disoriented | **keep** |
| `kneeling` | `:4445` | the **target** is kneeling | **keep** (see `k`) |
| `sitting` | `:4447` | sitting | **keep** |
| `flying` | `:4406` | flying | **keep** |
| `hovering` | `:4441` | hovering | **keep** |
| `immobilized` | `:4443` | immobilized | **keep**, and it absorbs `frozen` |
| `frozen` | `:4418` | the target is **not** immobilized, and its status text does not say frozen (**inverted**) | **merge** → `!immobilized`. Hydra's statuses have no separate "frozen", so it is the same fact |
| `rooted` | `:4430` | **not** rooted (**inverted**) | **keep**, with the importer flipping it → `!rooted` |
| `prone` | `:4428` | **not** any of sleeping, webbed, stunned, kneeling, sitting, prone or immobilized (`PRONE_STATUSES`, `:2736`), **inverted** | **rename** `down` for that set, and the importer flips it → `!down`. The name hides six other statuses |

### 2e. The target: what it is

| Word | Line | Runs when | Hydra reads | Verdict |
|---|---|---|---|---|
| `undead` | `:4432` | the target is undead | object types / bestiary `undead` | **keep** |
| `noncorporeal` | `:4425` | noncorporeal | object types | **keep** |
| `ancient` | `:4404` | the name starts with `grizzled` or `ancient`, except `ancient ghoul master` | the name | **keep**, with the exception kept as it is |
| `ascended` | `:4459` | the `<crtrStatus>` flag | `Classification::Ascended` | **keep** |
| `ascension_boss` | `:4461` | " | `AscensionBoss` | **keep** |
| `challenging` | `:4463` | " | `Challenging` | **keep** |
| `disengaged` | `:4465` | " | `Disengaged` | **keep** |
| `inferior` | `:4467` | " | `Inferior` | **keep** |
| `mini_boss` | `:4469` | " | `MiniBoss` | **keep** |
| `mount` | `:4471` | " | `Mount` | **keep** |
| `rider` | `:4473` | " | `Rider` | **keep** |
| `sympathetic` | `:4475` | " | `Sympathetic` | **keep** |

### 2f. The target: the fight so far

| Word | Line | Runs when | Hydra reads | Verdict |
|---|---|---|---|---|
| `thp` | `:4336` | the target's health % ≤ N. **Unknown health skips both `thp` and `!thp`** | `hp_percent`, **only when the HP is known** (below) | **keep** (Nisugi's `thp20`) |
| `wounded` | `:4481` | health ≤ 25%, and unknown skips | the same | **merge** → `thp25`, which it is exactly (`creature.rb:607`) |
| `fatalcrit` | `:4483` | a fatal crit landed | `fatal_crit()` | **keep** |
| `smote` | `:4485` | smitten, within the TTL | `smote(now)` | **keep** |
| `ucsdecent` | `:4487` | my unarmed positioning on **this target** is decent | `ucs_position(now)` | **merge** → `position N` (1 decent, 2 good, 3 excellent) |
| `ucsgood` | `:4489` | … good | " | **merge** → `position 2` |
| `ucsexcellent` | `:4491` | … excellent | " | **merge** → `position 3` |
| `ucstierup` | `:4493` | a tier-up attack has been named | `ucs_tierup(now)` | **keep** |
| `tier1` | `:4497` | the global `$bigshot_unarmed_tier` is 1 | — | **merge** → `position 1`. The global is the **last positioning message against any creature**, reset only by `You bolt` (`:2799-2818`). The per-target reading is the correct version of the same fact |
| `tier2` | `:4499` | … 2 | — | **merge** → `position 2` |
| `tier3` | `:4501` | … 3 | — | **merge** → `position 3` |
| `tier` | `:3220` | the global tier ≥ N (`!tier`: ≤ N, overlapping at N) | — | **merge** → `position_at_least N` |

**Unknown health is unknown.** Hydra's `hp_percent()` never reports unknown: `max_hp()`
falls back to `FALLBACK_MAX_HP` (400, `creatures/instance.rs:33`). If that were used
directly, a creature of unknown health would read as full health and `thp20` would skip it
for the wrong reason. Worse, a real 50-HP creature on 30 HP would read as 7%. The guard asks
for **HP that is known** (`hp_is_stated()`, or a bestiary template), and treats anything
else as unknown and skips, as bigshot does.

### 2g. The room and the fight's history

| Word | Line | Runs when | Hydra reads | Verdict |
|---|---|---|---|---|
| `mob` | `:3216` | at least N targetable creatures, not counting animates, untargetables and appendages | `creatures().in_room()` + `valid_target()` | **merge** → `targets_at_least N`; `!mob` → `targets_at_most N` |
| `valid` | `:3224` | at least N valid targets | the same | **merge**, as `mob`. Near-duplicates, with slightly different exclusion lists |
| `once` | `:4505` | this step has not yet been used on this target | the engine's record of what it sent | **keep** |
| `room` | `:4506` | this step has not yet been used on **any** target here | the same | **rename** `once_here` |
| `repeatdelay` | `:4270` | this step was last used N s ago or longer | the same | **rename** `every N` |

### 2h. Not a guard

| Word | Line | What it does | Verdict |
|---|---|---|---|
| `censer` | `:4520` | **casts spell 320 as a side effect** of being checked, when affordable and off cooldown, and never skips (`handle_censer`, `:4530`) | **drop** as a guard. It is a step: a sequence that casts Ethereal Censer, `cooldown`-guarded, before the step it was on. The importer rewrites it and says so |

---

## 3. What the model knows that bigshot could not see: **new**

Proposals, not asks. Each reads a fact the model already holds.

| Word | Runs when | Reads | Why |
|---|---|---|---|
| `injured "<part>" N` | the target's wound on `<part>` is rank ≥ N | `injury(part)` | don't sweep a creature whose legs are gone, and finish one that is bleeding out |
| `stunned_for N` | the target is stunned for ≥ N more seconds | `stunned_for(now)`, the stun estimate | "stunned" says nothing about whether there is time for a two-second attack |
| `helpless` | the target cannot act: dead, webbed, stunned, asleep, immobilized or rooted | `muckled(now)`, Lich's `muckled?` | the one set most profiles spell out a status at a time |
| `coup_ready` | a coup de grace qualifies at my trained rank | `coup_eligible(rank, now)` (`creature.rb:622-629`) | bigshot approximates this with `thp`; the rule is known exactly |

---

## 4. Tally

| Verdict | Words |
|---|---|
| keep | 38 |
| rename | 11 |
| merge | 34 |
| later | 3 |
| drop | 1 |
| **total** | **87** |

The five renamed amount words (`h`, `m`, `s`, `v`, `e`) and `k` count as **rename**. Words
merged two ways (`burst`, `surge`) count once, as merge. MEASURED: the rows of §2, counted
by verdict, then diffed against the `:3197` alternation, with every word once and none missing:

```sh
sed -n '/^## 2\. /,/^## 3\./p' plan/33-guard-vocabulary.md | grep -E '^\| `' \
  | sed -E 's/.*\| \*\*([a-z]+)\*\*.*/\1/' | sort | uniq -c
```

**What survives: 53 words** (38 kept, 11 renamed, and the four new ones), down from 87.
The reduction is mostly the 21 named-effect words becoming four.

---

## 5. Nisugi's six, as they land

| In `ojandhaart.yaml` | Becomes |
|---|---|
| `buff5` on `kweed` | `expiring "Tangleweed Vigor" 5` |
| `thp20` | `thp20`, skipping unknown health (§2f) |
| `empowered30` | `empowered_below 30` |
| `frozen` | `!immobilized` |
| `!frozen` | `immobilized` |
| `hidden` / `!hidden` | unchanged |

---

## 6. Questions for the author

1. **Polarity (§1, 2).** Every guard names when its step **runs**, and the importer flips
   `frozen`, `prone`, `rooted` and `voidweaver`. Agreed? The alternative, keeping bigshot's
   readings, keeps the trap for anyone writing a profile by hand.
2. **The 21 effect words (§2c).** Merge them into `buff "<name>"`? The cost is a longer
   line, `buff "Slashing Strikes"` instead of `flurry`. The gain is that a new ability needs
   no new word.
3. **Effect names.** Whole-name matching with a trailing `…` meaning "starts with", or keep
   bigshot's regex?
4. **`prone` → `down`**, and **`pcs` → `alone`**: good names, or better ones?
5. **The four new words (§3):** worth building now, or when a profile asks?
6. **`expiring`'s polarity (§2c, §5).** As built (2026-09-24), `expiring "<name>" N` names
   the window and the step carries the `!`: `kweed (!expiring "Tangleweed Vigor" 5)`. §5
   spelled it `expiring "Tangleweed Vigor" 5` with "runs when not expiring" inside the
   word, the one exception to question 1's rule. Keep the rule, or rename the word?

---

## 7. What follows the review

When the verdicts are agreed, the profile format, the inheritance chain and the importer are
designed from them (`plan/30` §5, §7 step 4). The importer's contract is already settled: a
kept or renamed word translates, a merged word maps to its survivor, and a dropped or later
word imports **with the step held and named**, never with the guard silently lost.

> **BUILT AHEAD OF THE REVIEW, 2026-09-24.** The format, the chain and the importer exist
> (`plan/30` §7 step 4's note), with Nisugi's six words built and the other 81 recognised
> and held. Because every unbuilt word holds its step, no verdict here changes what was
> built; a verdict adds a word to `cena-behavior/src/hunt/guard.rs` and a translation to
> `import.rs`, and a rename changes a string.
