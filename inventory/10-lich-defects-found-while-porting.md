# Defects found in Lich-5 while porting — triaged

**Scope and date.** Written 2026-09-19 against the working copy at
`E:/Cena/reference/lich-5`, during M3 (the typed character model). Every entry
below was surfaced by the seven `lib/` inventory agents and then **re-verified by
hand against the source** before being written here. Line numbers are from that
working copy.

This file exists because a port that faithfully reproduces its source reproduces
its source's bugs. `plan/13` §4a says to *"port aggressively where knowledge lives
in code"* — this is the list of places where the code is the wrong thing to copy.

**Status vocabulary.** **PORT-BLOCKER** = do not reproduce; the typed design must
differ. **FIX-ON-PORT** = reproduce the behaviour, correct the defect.
**LATENT** = not currently wrong, but fragile in a way the port should not inherit.
**BENIGN** = looks like a defect, is not; recorded so it is not "found" again.
**RETRACTED** = claimed here as a defect, since disproved; kept with the disproof,
because a list that silently drops its errors teaches nothing about how they got in.

---

## 1. `ignorable_cooldown` is written as a String and read as a Symbol — **FIX-ON-PORT**

Three combat maneuvers declare the flag with a **string** key:

| Maneuver | Site |
|---|---|
| `burst_of_swiftness` | `lib/gemstone/psms/cman.rb:66` |
| `surge_of_strength` | `lib/gemstone/psms/cman.rb:579` |
| `swiftkick` | `lib/gemstone/psms/cman.rb:595` |

```ruby
"ignorable_cooldown" => true
```

`CMan.available?` reads it with a **symbol** (`cman.rb:754`):

```ruby
if @@combat_mans.fetch(PSMS.find_name(name, "CMan")[:long_name])[:ignorable_cooldown] && ignore_cooldown
```

Ruby hash keys are typed: `h["x"]` and `h[:x]` are different slots.
`[:ignorable_cooldown]` returns `nil`, the guard is always false, and the `else`
branch runs unconditionally — so **`CMan.available?(name, ignore_cooldown: true)`
silently ignores the caller's request** for all three maneuvers that declare it. The
keyword argument is accepted, documented (`cman.rb:745-750`), and discarded.

**For the port:** a typed struct field cannot have two spellings. This defect is
structurally impossible once `ignorable_cooldown` is a `bool` on a Rust struct,
which is the same argument C21 makes for typed named fields generally.

---

## 2. `percent_health` and `percent_spirit` lack the zero-divisor guard their siblings have — **FIX-ON-PORT**

`lib/attributes/char.rb:83-105`. Four parallel methods, two of which guard:

| Method | Site | Guards `max == 0`? |
|---|---|---|
| `percent_health` | `:83` | **no** |
| `percent_mana` | `:87` | yes — returns `100` |
| `percent_spirit` | `:95` | **no** |
| `percent_stamina` | `:99` | yes — returns `100` |

```ruby
def Char.percent_health
  ((XMLData.health.to_f / XMLData.max_health.to_f) * 100).to_i
end
```

In Ruby `0.0 / 0.0` is `NaN` and `NaN.to_i` is `0`. Before the first vitals frame
arrives — a fresh login, or any moment after a reconnect and before the burst —
`percent_health` returns **0**, which reads as *"dead"*, while `percent_mana`
returns **100** on identical evidence.

**This is the exact failure `plan/12` §5.2 exists to prevent**: reporting a definite
value on no evidence. The asymmetry between the four siblings is itself the proof it
is an oversight rather than a decision.

**For the port:** `Option<u8>`, and `None` before the first observation. The guard
disappears because the type carries the distinction the guard was approximating.

---

## 3. `Spellsong.duration_base_level` returns a wrong number for level > 100 — **FIX-ON-PORT**

`lib/attributes/spellsong.rb:43-58`:

```ruby
def self.duration_base_level(level = Stats.level)
  total = 120
  case level
  when (0..25)   then total += level * 4
  when (26..50)  then total += 100 + (level - 25) * 3
  when (51..75)  then total += 175 + (level - 50) * 2
  when (76..100) then total += 225 + (level - 75)
  else
    Lich.log("unhandled case in Spellsong.duration level=#{level}")
  end
  return total
end
```

The `else` branch logs and **falls through to `return total`**, still holding its
initial `120`. A level-101 bard therefore gets a shorter base duration (120) than a
level-25 one (220) — a discontinuous cliff at the level cap, not a plateau.

GemStone's cap has been above 100 for years, so this is reachable in normal play.
The `Lich.log` call is evidence the author knew the case was unhandled; the defect
is that it returns a **plausible wrong number** instead of signalling.

**For the port:** return `Option`, or saturate at the 76..100 formula. Do not
reproduce a silent fallthrough to a sentinel that looks like data.

---

## 4. `Spells.get_circle_name(90)` returns `'Micellaneous'` — **FIX-ON-PORT**

`lib/attributes/spells.rb:24`:

```ruby
when '90' then 'Micellaneous'
```

Missing the second `s`. VERIFIED this is the **only** occurrence in `lib/`:

```sh
grep -rn "Micellaneous\|Miscellaneous" lib/    # 1 hit, the typo
```

That it is the only spelling anywhere means nothing in Lich compares against the
correct word — so the typo is self-consistent *within* Lich and breaks only at the
boundary, for a script or a port that spells it correctly.

**For the port:** spell it correctly, and note that any ported script comparing
circle names by string needs the same correction. This is an argument for the circle
being an **enum**, not a `String`, which is what M3's design already says.

---

## 5. `feat.wps` — **RETRACTED.** They are aliases, not a collision. One real gap: `sighting` is missing — **FIX-ON-PORT**

**This entry first claimed two feats shared one Infomon key, as a PORT-BLOCKER. That
was wrong, and the author corrected it by pointing at the wiki page.**

`lib/gemstone/psms/feat.rb`:

```ruby
"weighting" => { :short_name => "wps", ... }   # :258
"padding"   => { :short_name => "wps", ... }   # :265
```

`PSMS.find_name` (`lib/gemstone/psms.rb:72`) resolves with `.find`, returning the
**first** match, and `psms.rb:123` builds the Infomon key from that `short_name` —
so both entries do resolve to the single key `feat.wps`. That much is accurate.

**What is wrong is the inference.** `reference/wiki_clean/Weighting_ Padding_
Sighting.txt:1-3` is one page for one feat:

> **Weighting, Padding, Sighting — Mnemonic `[wps]` — Type Passive — Available To
> Warriors**

and the command takes the variant as an **argument**, not as a separate feat
(`:96`):

```text
FEAT WPS ASSESS (DAMAGE or CRITICAL or SIGHTING) (ITEM)
```

Weighting, padding and sighting are three services of **one** passive feat with one
rank. `feat.wps` holding one value is therefore **correct**, and Lich's two entries
are **aliases** — two names a user might type for the same thing.

**Lich's own table was evidence against the collision reading and I misread it.**
Both entries carry identical `:type`, `:cost`, `:usage => nil` and an identical
`:regex` of `/USAGE\: FEAT WPS \{options\} \[args\]/` — the shared usage banner.
Byte-identical values on two keys is the signature of an alias; genuinely distinct
feats would differ somewhere. I read duplication and stopped, without asking what
the feat *is*.

### CONFIRMED ON THE WIRE, 2026-09-19

The retraction above was written from the wiki page. It now has wire evidence:
a `--psm` capture the author ran prints the feat table with **32 rows** where
Lich's table has 33, and the one row reads

```text
   Weighting, Padding,  wps             0/1   Passive
```

One feat, one mnemonic, max rank **1**, Skill column truncated at 20
characters -- cutting "Sighting" off the feat's full name. Two Lich entries,
one game row: aliases, as the author said. (`plan/15` §2b.3.)

### The one real defect here

**`"sighting"` is absent from the table entirely.** VERIFIED:

```sh
grep -n "sighting\|weighting\|padding" lib/gemstone/psms/feat.rb
# 258: "weighting"
# 265: "padding"
```

Two of the mnemonic's three names resolve; the third does not. `Feat.known?("sighting")`
returns false for a warrior who has the feat, because `find_name` cannot match a name
nobody wrote down.

**For the port:** model this as one feat with an alias set — `{weighting, padding,
sighting}` → `wps` — which makes the missing third name a data-completeness question
a test can ask, rather than a lookup that silently fails. Do **not** key the store on
the long name, which was this entry's original recommendation: it would split one
feat's rank across three slots and invent the very bug this entry wrongly alleged.

### Why this was worth keeping rather than deleting

The failure mode is the one `plan/05` §−2 exists to catch, in a form the rule does
not name: every *fact* I cited was verified — the line numbers, the `.find`, the key
construction. The **inference** on top of them was not, and a chain of verified facts
lent it unearned weight. Checking what the feat was would have taken one page read.
`reference/wiki_clean/` was sitting there.

---

## 6. The same `short_name` **across** categories is safe — **BENIGN**

`blockspec` appears in `cman.rb:46` and `shield.rb:30`; `spikemastery` in
`armor.rb:43` and `shield.rb:165`. `psms.rb:123` namespaces the key by category:

```ruby
Infomon.get("#{type.downcase}.#{seek_psm[:short_name]}")
#            ^^^^^^^^^^^^^^^^^
```

They land as `cman.blockspec` and `shield.blockspec` — genuinely different maneuvers
that happen to share a display name.

**With #5 retracted, this entry and that one now say the same thing from two
directions:** a repeated `short_name` is not by itself evidence of anything. Across
categories the namespacing separates genuinely different maneuvers; within a category
(#5) the repetition marks aliases for one feat. In neither case is it a collision,
and in both cases I initially read it as one.

A port that keys on `(category, short_name)` and carries an explicit alias set is
correct for both.

---

## 7. `Skills`' shorthand gsub-chain is order-dependent — **LATENT**

`lib/attributes/skills.rb:48-63` defines 36 backwards-compatible shorthand methods
by squashing each long name and comparing:

```ruby
method.to_s.gsub(/_/, '')
      .gsub(/elementallore/, 'el')
      .gsub(/spirituallore/, 'sl')
      .gsub(/sorcerouslore/, 'sl')      # <-- same output as the line above
      .gsub(/mentallore/,    'ml')
      ...
      .eql?(shorthand.to_s)
```

Two different inputs map to the same prefix `sl`. VERIFIED by executing the chain
that this is currently **harmless** — all five lore skills still produce distinct
results (`sldemonology`, `slnecromancy`, `slblessings`, `slreligion`,
`slsummoning`), because the suffixes differ.

It is listed anyway because of how it fails if that ever stops being true. The
lookup is a `.find`, which returns `nil` on no match, and the `nil` is then
**captured by a closure**:

```ruby
self.define_singleton_method(shorthand) do
  Skills.send(long_hand)     # long_hand may be nil, captured at definition time
end
```

An unresolved shorthand therefore does **not** raise at load. It defines a method
that raises `NoMethodError` on `nil` the first time a script calls it — moving the
failure from startup, where it would be obvious, to whenever some script first
touches that one skill.

**For the port:** the 46 skills are a static table (`enhancive.rb:42-89`'s
`SKILL_NAME_MAP` is the only place the wire strings are written down). Generate the
aliases from an explicit pair list, and make a missing entry a compile error. Do not
port a chain whose correctness depends on suffixes staying distinct.

---

## 9. `Injured`'s rules disagree with a second implementation and with the wiki

Found 2026-09-20 while porting `lib/gemstone/injured.rb`. Three separate
issues, none a defect in the usual sense -- these are places where two
community implementations of one game rule do not agree.

### 9a. `eherbs.lic` has its own `able_to_cast` and it is a different rule

`reference/scripts/scripts/eherbs.lic:2885-2914` reimplements the predicate:

```ruby
stacked_left_scar += h['scar']
stacked_left_wound += h['wound']
if (stacked_left_scar > 1 || stacked_left_wound > 1)
```

| | `injured.rb` | `eherbs.lic` |
|---|---|---|
| arm + hand | `max(arm, hand)` per side, wound and scar merged | sums wound and scar **separately** across the pair |
| rank-1 scars | discarded | counted in the sum |
| Sigil of Determination | bypasses rank ≤ 2 | not consulted at all |

**AUTHOR, 2026-09-20: *"I would say injured.rb implementation is the right
one."*** Ported accordingly. Recorded because a reader comparing the two will
find the disagreement and should not have to re-derive which won.

The wiki supports that call: it discards rank-1 scars (*"Rank 1 scars never
have any mechanical penalties"*) and describes Sigil explicitly, both of which
`eherbs.lic` ignores.

### 9b. RESOLVED: rank-2 nerves DO block ranged, and the wiki is incomplete

`injured.rb:187` includes `nsys` in the rank-2 block for
`able_to_use_ranged?`. `reference/wiki_clean/Wound.txt`'s penalty table lists
only *"nervous system | Rank 2: prevents spellcasting, searching"*, so this was
first recorded as a disagreement and marked UNVERIFIED.

**AUTHOR, 2026-09-20: *"I personally tested ranged for injured.rb."*** So
`injured.rb` is right and the wiki's table is **incomplete**, not wrong --
which is consistent with it also omitting the cumulative rules
(`injured.rb:128-133`) and the per-side arm/hand merge that the same file
implements.

**The lesson is about the oracle, not the rule.** The wiki was promoted to
oracle for these tests precisely because it is a primary source, and it earned
that once by catching a wrong assertion (a rank-2 arm does not block casting).
It does not follow that its silence is evidence: a table that omits three
known rules cannot be read as an exhaustive list, and treating absence there
as a contradiction was the same mistake as reading absence from the login
burst as invalidation (`plan/15` §2a.4a.3a).

### 9c. Rank-3 legs are called "NOT critical" while behaving critically

`injured.rb:143`:

> *"Rank 3 leg/foot injuries always prevent sneaking, but these are NOT
> critical (Sigil cannot bypass, but they're not in the critical list for
> other actions)"*

The parenthesis describes Lich's own data structures, not a game rule. A
rank-3 leg blocks sneaking and Sigil does not help -- which is exactly what
"critical" means for every other action -- so the distinction names nothing
observable.

**AUTHOR, 2026-09-20: "Lich quirk -- flag it."** Ported as a critical refusal,
because that is the behaviour; only the label differs.

---

## 8. Already recorded elsewhere

These were found earlier in M3 and are carried here so the list is in one place.
Each is VERIFIED in the source; see M3's plan for the porting decision.

| Defect | Site | Status |
|---|---|---|
| `SocietyJoin`'s `'Lodge'` branch is unreachable — the scan is `/Order\|Council\|Guardians/`, which cannot produce `'Lodge'`, so **joining Council of Light records nothing** | `infomon/parser.rb:415-425` | FIX-ON-PORT |
| `warcry.` has two key spellings — presence writes `warcry.bellow`, absence zeroes `warcry.bertrandts_bellow`, so **a learned warcry is never un-learned** | `parser.rb:379` vs `:369-374` | PORT-BLOCKER |
| `SocietyStep` does `get + 1` on a possibly-`nil` | `parser.rb:428` | FIX-ON-PORT |
| Platinum collapses into Premium, unrecoverably | `parser.rb:579-580` | PORT-BLOCKER |
| Unrecognized enhancive names are silently dropped (Rule 2.2 violation) | `parser.rb:715` et al. | FIX-ON-PORT |

---

## What this list is evidence for

Four of the twelve entries (#1, #4, and two in §8) are **key-spelling or
string-comparison defects** — a value written under one name and read under another.
Every one is structurally impossible in a typed model with named fields, which is the
case `research/04-inherited-decisions.md:1878` (C21) makes on other grounds. In this
specific class of defect the type system does work Lich has to do by convention, and
does not always get right.

That is worth stating precisely, because the opposite claim is easy to make and
wrong: Lich is a mature, working client whose protocol knowledge is the reason this
project can exist at all. These defects are the residue of ten years of accreted
Ruby, found by reading all 327 logic files. They are not a reason to trust it less —
they are the specific places where "port it faithfully" is the wrong instruction.

## A methodology note, earned twice

**Three of the seven originally-flagged items did not survive re-verification**: the
`Skills` gsub chain (#7, downgraded to LATENT), the cross-category `short_name`
repetition (#6, BENIGN), and `feat.wps` (#5, retracted outright). All three were
"this looks duplicated, therefore it is broken" — pattern-matching on shape without
checking meaning.

The first two I caught myself by running the code. **#5 I did not**, and it shipped
as the list's only PORT-BLOCKER until the author pointed at
`reference/wiki_clean/Weighting_ Padding_ Sighting.txt`. The difference is instructive:
executing a gsub chain is cheap and I did it; reading a wiki page about what a feat
*is* is equally cheap and I did not, because the source-code evidence felt sufficient.

**Lich's table is not a specification of the game.** It is one client's model of the
game, and where that model looks strange the game is the tiebreaker —
`reference/wiki_clean/` first, then the corpus. `CLAUDE.md` already ranks these
sources for the *protocol*; the same ranking applies to game mechanics, and this
entry is why it is now written down.
