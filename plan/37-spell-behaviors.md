# 37 — Spells: spellactive, ewaggle and spellcaster, measured and ordered

**Status: PROPOSED 2026-09-25, stages built in order.** The author, 2026-09-25: *"Then do
ewaggle, spellactive, and spellcaster."* Three scripts that cast spells for the player, each
a different shape: one keeps a list of spells up forever, one buffs a list of people once,
one turns a typed spell number into the right casting command. They share everything below
the decision of *what* to cast, so the shared part is built first.

## 0. Measured

| Script | Where | Lines | Version |
|---|---|---|---|
| `spellactive.lic` | `reference/scripts/scripts/` | 244 | 1.3.3 |
| `ewaggle.lic` | `reference/scripts/scripts/` | 1,796 (GTK window 200-635) | 2.1.6 |
| `spellcaster.lic` | `reference/lich_repo_mirror/lib/` (the old Lich repository's; not in `scripts`) | 418 | 0.9.6 |

What all three lean on is Lich's `Spell` object: whether a spell is known, affordable,
active and for how long; its cost; its duration for this caster, on self or on another
(`time_per`); whether a cast **stacks**, **refreshes** or neither; whether it can be
multicast; whether it wants a stance or can be channeled. That object is built from
`data/effect-list.xml`, and **Hydra's spell table dropped most of what these scripts use**
(`cena-model/data/spells.tsv`, cut by a `tools/extract_spells.rb` that is not in the
repository). MEASURED against the live install's `effect-list.xml` (mtime 2026-09-13):

| Dropped | Count | Used by |
|---|---|---|
| `<duration span=>` (`stackable` 116, `refreshable` 57, one written with double quotes) | 173 | ewaggle's three casting passes |
| `<duration multicastable=>` | 100 | ewaggle's multicast |
| `<duration max=>`, `persist-on-death=`, `real-time=` | 19, 34, 10 | the stop-at ceiling, death |
| `<spell incant=>`, `stance=`, `channel=` | 17, 16, 15 | spellcaster's verb and stance |
| `<cast-proc>` (Lich's per-spell casting code) | 109 | how some spells must be cast |
| `<cost>` that is an expression, not a number | several | affordability (the table parses a number or nothing) |

`grep -o "<duration[^>]*>" effect-list.xml \| grep -o " [a-z_-]*=" \| sort \| uniq -c`, and
the same for `<spell` and `<cost`.

**Durations are Ruby.** 120 are `Derived` expressions the table keeps unevaluated; their
vocabulary, MEASURED by the same file: circle ranks (`Spells.minorspiritual` 16,
`Spells.ranger` 14, ...), `Stats.level` 15, `Spellsong.timeleft` 12, `Society.rank` 5,
`Spell[N].known?` in a ternary, `.to_i`, `[a, b].min/max`. A few scrape the scrollback for a
CS/TD line or read `CMan`; those stay unevaluated.

## 1. Stages

### Stage 1 — the spell data whole

**BUILT 2026-09-25.** `tools/extract_spell_extras.rb` → `cena-model/data/spell_extras.tsv`
(514 rows); `cena-model/src/spells/extras.rs` (`Extras`, `Shape`, `Span`, `Cost`, on
`Spell::extras`). Tested in `cena-model/tests/spell_extras.rs`, every count pinned to the
source. A first count of 56 refreshable spans missed one written with double quotes; the
extractor did not, and its test caught the difference.

A companion extractor, `tools/extract_spell_extras.rb`, reads what the table dropped into
`cena-model/data/spell_extras.tsv`, joined to the table by number: every attribute of
`<spell>`, of each `<duration>` in order, of each `<cost>` with its text as written, and the
`<cast-proc>`. It fails on an attribute or element it does not know. The table's own
extractor is not rebuilt: the table it made is committed and correct for what it holds.

### Stage 2 — durations and costs, evaluated for a character

**BUILT 2026-09-25.** `cena-model/src/spells/expr.rs` (the evaluator, names resolved by the
caller) and `cena-model/src/state/spell_time.rs` (`GameState::spell_minutes`,
`spell_cost`, `evaluate`: circle ranks by printed name, skills by Lich's key or short form,
the level from its label, society rank, `Spell[N].known?`, `.active?`, `.timeleft`).
MEASURED: all 120 derived durations evaluate; the table's 40 unknown ones (Spellsong,
`CMan`, scrollback) stay `None`. Tested in `cena-model/tests/spell_expr.rs`.

A small evaluator over the measured vocabulary: numbers, `+ - * /`, parentheses, comparison,
`?:`, `if/elsif/else` of one line, `.to_i`/`.to_f`/`.min`/`.max`, and the facts the model has
(circle ranks, skills, level, society rank, known spells, what is active). `None` for anything
else, never a guess.

### Stage 3 — the casting step

One casting primitive for the three: the command (`incant N`, `prepare N` + `cast <target>`,
`channel`, `evoke`, a multicast count, a `cast-proc`'s verb where it only differs in words),
the answers as a closed set (cast, fizzled, hindered, no mana, bad target, not here), and
whether a spell can be cast now (known, affordable, no cast roundtime, not prepared, hands).

### Stage 4 — spellactive: keep these spells up

A list of spells kept active, the `nocast` rooms, Sigil of Power when 25 short, spellactive's
special cases (the Barkskin cooldown, the sign stagger, 606/640 by way of 625, Beacon of
Courage by 1608). Run as `;keep` by the hunt's desk with a machine that keeps and never
ends, as `;heal` runs with one that heals once; and as the hunt's own Maintain when hunting.

### Stage 5 — ewaggle: buff these people

For each target -- yourself, or anyone by name -- what they have up (`spell active <name>`,
parsed, or the effects list for yourself), then ewaggle's three passes: stackable spells up to
`stop_at` minutes, multicast as far as the ranks allow; refreshable spells when under
`refreshable_min`; the rest when absent. Mana waits or bails as the profile says. `;waggle
[name ...]`.

### Stage 6 — spellcaster: the typed spell

`;sc <number|alias> [target] [count]`: the spell cast with the verb, stance and channel the
player set for it, stance back to guarded after an aimed one; the `conserve` and `safety`
checks. **Open for the author:** spellcaster's point is that a bare `401` typed at the prompt
is caught before it reaches the game. Hydra's command line only claims lines with the command
symbol, so a bare number going to the game is today's behavior; catching it is a change to
what the player's own lines do, and is the author's call.

## 2. Not ported, named

ewaggle's GTK window, armor specializations (`Armor.use`), sonic gear, Sign of Wracking and
Symbol of Mana as mana sources, mana bread (203), Retribution chants; spellcaster's `mantle`
list (unfinished in the script itself: its `remove` branch is a comment).
