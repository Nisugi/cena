# Brief: survey one pile of lich_repo_mirror scripts against Cena

You are one of nine reviewers. Each has a pile of Lich scripts. Your pile is named in your
prompt. Everything below applies to all nine.

## What the author asked

> "You want to go through the scripts in that new repo looking for things we could use?
> Anything that involves data, capturing or using should be compared against what we capture
> and our defs at the very least."

The answer they want: **what in these scripts Cena could use**. For every script that
**captures** game text (regexes, waits, hooks), **holds** game data (tables of creatures,
items, spells, rooms, prices, messages...), or **uses** character/game data (Lich APIs such as
`GameObj`, `Spell[]`, `Effects`, `Char`, `Skills`, `Bounty`, `Wounds`, `XMLData`), compare it
against what Cena captures and defines. Report what Cena has, what it has partly, what it lacks,
and where the two disagree.

## Cena, briefly

Cena (product name **Hydra**) is a Rust client for GemStone IV. It replaces Lich and a
frontend in **one binary** and runs 3-25 characters in one process. Repo: `E:\Cena`.
- **One parser, N classifiers.** One parser turns the wire into frames. Each recognised game
  fact is a **stateless classifier** in `crates/cena-model/src/state/`, over those frames. A
  fact that needs memory across lines is a **consumer** (e.g. `crates/cena-session/src/combat_recorder/`,
  `crates/cena-session/src/ledger/`). So a regex in a script usually corresponds to a
  classifier in cena-model, and its game text shows up there as a literal or a pattern.
- **Game data ships as TSV** in `crates/cena-model/data/`, extracted from Lich (creatures from
  Lich's creature templates, crits, spells from `effect-list.xml`, weapons, armor, gameobj-data...).
  The rule is **port data whole**: every field, and "nothing reads it yet" is not a reason to drop it.
- **Automation is curated Rust behaviors** in `crates/cena-behavior/src/` (hunt, loot, town,
  travel), configured by data profiles. No embedded scripting.
- Roadmap, to rank value: **M6 (now)**, the hunt: a bigshot port (`plan/30-m6-hunt.md`,
  guard words in `plan/33-guard-vocabulary.md`), looting and the town selling round (eloot port,
  `plan/31-eloot-port.md`, including the locksmith pool), the loot ledger (`plan/34-loot-ledger.md`),
  and healing next (M6d; no heal behavior exists yet). **M7**: an agent protocol (`plan/35-m7-agent.md`).
  **M8**: the remaining behaviors (**Bounty**) and the customization surface (**highlights first**,
  then keybinds, macros, layouts). **M10**: an egui GUI (`plan/28-gui-inventory.md`).
  Multi-session is `plan/29-m5-multi-session.md` (`crates/cena-host`). The map is `plan/21-mapdb.md`
  (`crates/cena-map`).

**Start by reading** `survey/cena-baseline.md` in the scratchpad (the path is in your prompt).
It lists every Cena model module, data table, session consumer and behavior with its first doc
line. Then open the modules your prompt points at.

## Method

1. Your pile is `survey/pile-<name>.tsv`: script, lines, capture, data, api. It is sorted
   richest first. The counts are heuristics for ordering, not findings.
2. **Substantive scripts** (capture + data >= 5): read the header (purpose, author, version,
   date, license line if any). Extract what it captures and holds, e.g.
   `grep -nE "=~ */|waitforre|matchtimeout|matchwait|DownstreamHook|when +/|Regexp.new|%r\{" X.lic`,
   and read the data literals. Read the top of the pile deeply. For large scripts, work from
   the capture lines and data blocks rather than every line.
3. **Families**: `tpick`, `tpick2`, `old-tpick` and `test-tpick`, or `heal`, `heal2` and
   `healbot2025`, are versions of one script. Read the newest (header version or date) in full.
   Note the others as variants, and `diff` them if the difference might matter.
4. **Compare each captured fact or data table against Cena.** Grep Cena for a distinctive
   fragment of the game text, e.g. `grep -rn "fragment" E:/Cena/crates --include=*.rs --include=*.tsv`.
   Also grep for the concept (module names, type names). If `E:/Cena/reference/lich-5/lib`
   already holds the same pattern or table, say so: Cena may already port it from there.
5. Give each finding a verdict:
   - **HAVE**: Cena captures or holds it. Cite `file:line`.
   - **PARTIAL**: Cena has the fact but misses a variant, field, or message. Say which.
   - **GAP**: Cena does not capture or hold it.
   - **CONFLICT**: their text or data differs from Cena's. Quote both, cite both.
   - **N/A**: not game data (pure UI plumbing, DragonRealms, a one-line macro).
   Give each GAP, PARTIAL and CONFLICT a **value**: HIGH (needed for M6 to M8 as above),
   MEDIUM (useful, no milestone yet), or LOW. Name the milestone.
6. **Non-substantive tail**: a name scan is enough. Group them in one line per kind, and open
   any whose name suggests game data.

## Evidence rules (the project's, binding)

- Cite `script.lic:line` and `crates/...rs:line` for every claim. Give the command behind any
  count. Label a claim you did not verify **INFERRED** or **UNVERIFIED**. Never write a number
  from memory.
- "Cena lacks X" is a claim about absence: grep for it more than one way before saying so,
  and give the searches you ran. A search that finds nothing because the path or the word was
  wrong manufactures a false GAP.
- The scripts belong to their authors. The mirror is Apache-2.0 for its tooling, not for their
  content. Recommend porting **facts and data** (message texts, tables of game facts), not
  code. Note the author, and the license line if one exists, for HIGH items.

## Do not

- Do not edit anything under `E:\Cena` (crates, plan, data, CLAUDE.md). Write only to your own
  output file, and to `survey/tmp-<pile>/` for scratch. Other reviewers share the survey dir.
- Do not run git commands that change state. Do not run `cargo run` or the `cena` binary: it
  logs into the live game.
- Do not search `E:\Gemstone\data\log archive` (49 GB; the author must be asked first).
- No network access. The mirror is already cloned at `E:\Cena\reference\lich_repo_mirror\lib`.

## Output: `survey/<pile>.md`, written as you go

- Create the file with the **Write** tool **first**, as a skeleton with the headings below.
  Then append with **Edit** as you finish each group of about 10 scripts, so partial work
  survives if the session ends. Keep each Write or Edit under about 15,000 characters. Never
  write a file through a Bash heredoc: long ones are silently truncated.
- Python reads files as cp1252 here. Use `encoding="utf-8"`, or `read_bytes().decode("utf-8", "replace")`.

```
# <pile>: lich_repo_mirror survey

Scope: N scripts, M substantive; K read deeply, J at capture-line depth; Cena HEAD <sha>
(`git -C E:/Cena rev-parse --short HEAD`), date.

## Top findings
(At most 12, ranked by value. Each gives what it is, script:line, the Cena counterpart
file:line or the searches that found none, the verdict, the value and the milestone.)

## Data tables Cena lacks or differs on
| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |

## Captures Cena lacks or differs on
| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |

## Script by script
| script | lines | purpose | captures / data / uses | verdict summary |

## Tail
(Non-substantive scripts, grouped by kind in one line each.)

## Method
(The commands you ran for counts and for the absence claims.)
```

## When done

Reply in 300 words or fewer: the top findings, the output file's path, and your coverage
(how many scripts read deeply, how many at capture-line depth, and how many in the tail).
