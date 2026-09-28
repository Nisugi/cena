# 14 — What runs: the checker over both script collections

The answer to *"does my script run under Hydra?"*, script by script, for both collections
`inventory/13` measured, made by Hydra's own checker (`plan/46` §1, §11 step 5; the author, §10
question 10: *"yes"*, ship it). The lists are the two TSVs beside this file; the tables below come
from them. Made 2026-09-27.

**What it measures is the runner as built**, through step 4: Lich's engine and Hydra's edges,
exactly as a runner loads them. `inventory/13` §3.3 asked a different question, which scripts
*could* run once Hydra answers what they need, and counted the model half of Lich's API as
answered because Hydra's model holds it. This counts it as answered only when a script can
read it through Lich's names. The difference between the two is the runner's to-do list, and
the table in *What Hydra has to answer* orders it.

## How it was made

MEASURED. Collection A is `reference/scripts` (elanthia-online, `df11fdd`, 2026-09-25, 236
`.lic`); B is `reference/lich_repo_mirror/lib` (the old Lich repository, `d326ee9`, 2026-09-22,
2,130 `.lic`), both pulled before the run. Ruby 4.0.3 with Prism 1.9.0, on the author's machine,
with Lich's installer's gems.

```sh
ruby bridges/ruby/hydra/check.rb reference/scripts/scripts > A.json         # 9 s
ruby bridges/ruby/hydra/check.rb reference/lich_repo_mirror/lib > B.json    # 24 s
ruby inventory/14-what-runs/make.rb elanthia-online=A.json lich-repo=B.json
```

**The checker** (`bridges/ruby/hydra/check.rb`) reads each script with Ruby's own parser, as
Lich runs it (cut at each line that is a label alone, `script` in scope), and judges every name
it uses against the runner itself, loaded as `bridges/ruby/hydra/engine.rb` loads it. Each
finding has a kind, and a script's verdict is its worst:

| Verdict | Means |
|---|---|
| `stops` | a line raises under Hydra: a name not defined, a method not answered, a library missing, or Ruby cannot read the file. The script stops there, if it gets there |
| `windows` | a line opens one of Lich's windows (Gtk): a window of its own while Hydra lets the runner load the gtk3 gem and the player has it, as it does for now; without it the script stops there, often at only a settings window |
| `markup` | a line reads the game's markup, which Hydra does not give scripts (`plan/46` §6.2): it finds none |
| `differs` | it runs, and does not do what it did under Lich: a hook whose pattern is markup, Lich's own state read as nil |
| `runs` | nothing found |

Each finding also says whether it is **Hydra's to answer** or **the script's own**: a name
defined nowhere, a method Ruby 4.0 dropped (`File.exists?`), a library not installed, a file Ruby
cannot read. Those stop the script under Lich 5.21 as well, which also runs on Ruby 4.0.

## Verdicts

| Verdict | A files | A lines | B files | B lines |
|---|---:|---:|---:|---:|
| runs | 82 (34.7%) | 34477 (18.8%) | 1254 (58.9%) | 201732 (23.7%) |
| differs | 1 (0.4%) | 66 (0.0%) | 10 (0.5%) | 4679 (0.6%) |
| markup | 12 (5.1%) | 4893 (2.7%) | 103 (4.8%) | 41750 (4.9%) |
| windows | 16 (6.8%) | 11762 (6.4%) | 55 (2.6%) | 56139 (6.6%) |
| stops | 125 (53.0%) | 131989 (72.1%) | 708 (33.2%) | 546248 (64.2%) |

So **35% of A and 59% of B run today**, 1 and 10 more with a difference. `inventory/13`
§3.3's 76% and 87% were the ceiling once Hydra answers everything it plans to; this is the floor
as built. As there, the big scripts are the blocked ones: by lines, 19% of A runs.

## What Hydra has to answer, in the order that runs the most scripts

Of the scripts that do not run (A 153, B 866), those whose every finding is Hydra's to answer
(A 130, B 654), by what each needs. Greedy: at each step the gap whose answer lets the most scripts run, then the
one most needed; the percentages count every script, those already running included.

| # | Gap | A: needing it | A: running after | B: needing it | B: running after |
|---:|---|---:|---:|---:|---:|
| 1 | the game's markup | 39 | 95 (40.3%) | 196 | 1362 (63.9%) |
| 2 | Lich's windows (Gtk) | 35 | 111 (47.0%) | 122 | 1422 (66.8%) |
| 3 | Spell#cast | 24 | 117 (49.6%) | 156 | 1501 (70.5%) |
| 4 | Lich::Util | 34 | 136 (57.6%) | 75 | 1544 (72.5%) |
| 5 | Stats | 19 | 140 (59.3%) | 90 | 1562 (73.3%) |
| 6 | Skills | 22 | 146 (61.9%) | 102 | 1589 (74.6%) |
| 7 | Spells | 6 | 147 (62.3%) | 57 | 1626 (76.3%) |
| 8 | Spell#active | 0 | 147 (62.3%) | 20 | 1643 (77.1%) |
| 9 | Map.dijkstra | 7 | 147 (62.3%) | 33 | 1656 (77.7%) |
| 10 | Map.list | 25 | 150 (63.6%) | 53 | 1665 (78.2%) |
| 11 | Room#find_nearest | 13 | 151 (64.0%) | 47 | 1679 (78.8%) |
| 12 | StringProc | 7 | 153 (64.8%) | 29 | 1690 (79.3%) |
| 13 | Wounds | 9 | 154 (65.3%) | 53 | 1701 (79.9%) |
| 14 | Scars | 8 | 156 (66.1%) | 41 | 1730 (81.2%) |
| 15 | Room#find_nearest_by_tag | 8 | 156 (66.1%) | 26 | 1742 (81.8%) |
| 16 | XMLData.next_level_text | 2 | 158 (66.9%) | 24 | 1752 (82.3%) |
| 17 | Effects | 7 | 159 (67.4%) | 15 | 1763 (82.8%) |
| 18 | Society | 8 | 161 (68.2%) | 24 | 1771 (83.1%) |
| 19 | XMLData.level | 3 | 161 (68.2%) | 17 | 1780 (83.6%) |
| 20 | Map.ids_from_uid | 15 | 168 (71.2%) | 3 | 1782 (83.7%) |

The first two are decisions. **The game's markup** is `plan/46` §6.2's, taken: labelled data
first, a copy of the game's bytes if that is not enough, never faked tags. **Lich's windows are loaded
for now** (the author, 2026-09-27: *"sure for now we will load gtk, but we will probably not use
gtk on release"*): the runner loads the gtk3 gem when Hydra lets it and the player has it, and
these scripts open their windows. They are still counted here, since a release may not load it,
and it is not cheap (`plan/46` §9). The rest are reads Hydra's model already holds
(`inventory/13` §2.5: `Stats`, `Skills`, `Spells`, `Wounds`, `Scars`, `Effects`, `Society`,
`XMLData`'s fields) or acts it already performs (`Spell#cast` is the casting step,
`plan/37` Stage 3; `Map.dijkstra` and `find_nearest` are travel's), not yet given Lich's names in
the runner.

## What stops scripts that is their own

INFERRED from the reason each finding gives: these stop the script under Lich 5.21 too.

| | A | B |
|---|---:|---:|
| scripts with one | 23 | 212 |
| a name defined nowhere: a typo (`elisf`, `eixt`), a local used out of its scope, a Lich 4 function, or a constant Ruby dropped (`Fixnum`, `TRUE`, `FALSE`: A 4, B 11) | 18 | 146 |
| a method Ruby 4.0 dropped (`File.exists?` in 29 of B's, `URI.encode`) | 1 | 30 |
| Ruby cannot read it (among them Lich's own label cut: a comment line `USAGE:` alone is a label to Lich, which cuts a `=begin` block in two) | 0 | 29 |
| a library not installed here (`Olib`, `nokogiri`, `discordrb`, `win32/clipboard`) | 4 | 13 |

A script can have more than one, so the rows add up to more than the first.

## Hand check

- **Stops**: 30 findings drawn at random from B (`srand(46)`), each read at its line: all 30
  name something the runner does not define (`Scars`, `Wounds`, `Stats.prof`, `Spell#cast`,
  `Map.dijkstra`, `XMLData.society_task`...). Two were `LNet`, counted then as the script's
  own; it is lnet.lic's, defined when that runs beside it in the same runner, and a constant
  named after a script in the same folder is now left to that script.
- **Runs**: 18 of B's scripts that run mention a blocking name in their text (`Stats.`,
  `.cast`, `Lich::Util`, `status_tags`...). Each read: in a comment, behind `defined?` or
  `respond_to?`, under a `rescue` modifier, after `__END__` (`herbheal`, whose code moved to
  eherbs), another script's library (`AlastirLib`), the script's own `Stats`, or **a spell held
  in a variable or a constant** (`sign.cast`, `PROVOKE.cast`: 4 scripts), which the checker
  cannot place and so misses.
- **Corrected in building it**, each found by reading findings: a script's own class named
  like Ruby's (`Data`); Sequel's table blocks (`primary_key :id`); guards; a name in a
  parameter's default; a mixin's methods (`extend Forwardable`); and the reverse, a
  `begin ... rescue` taken for handled when most report the error and end (`hexmal`).

## Limits

- **A lower bound on what stops.** A method of a receiver the checker cannot place (`spell.cast`
  on a variable) is not judged, nor names that may come from a gem, a mixin, another script the
  script starts, or a script beside it named like the name. A finding in dead code counts.
- **`markup` counts reading, not needing.** `wander` reads markup only to recover from a maze
  that moved, and ran unchanged in step 2.
- **The machine matters for libraries**: a gem installed here but not on a player's is counted as
  there.
- A has no DragonRealms scripts and B one (`inventory/13` §3); they are listed as any other.

## Files

`14-what-runs/elanthia-online.tsv` (A) and `14-what-runs/lich-repo.tsv` (B), a row a script:
`script`, `lines`, `verdict`, `builtin` (`yes` when Hydra has it built in and runs its own in its
place: `go2`), `hydra_answers` (what Hydra has to answer for it, as the gaps above name them) and
`its_own`. `14-what-runs/make.rb` makes them, and the tables, from the checker's answers.
