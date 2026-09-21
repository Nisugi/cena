# Vocabulary proposed by round two of the long tail

**Status: PROPOSALS, 2026-09-21.** Collected from the reports of the agents that ported
slices F, G and H (slice E to be added). Each is what an agent found it could *not* say with
the vocabulary as it stood, and what it would add. Nothing here is decided. Evidence, not
instruction (`CLAUDE.md`): the main session and the author choose; shape ids are the
converter's (`report/unported_crossings.tsv`, `unported_costs.tsv`).

Rule the author has set: **everything is ported**, puzzles included. So each row ends as a
primitive, a condition, or a named routine — never as "left unported".

## Costs (37 left)

| # | proposal | exits | notes |
|---|---|---|---|
| C1 | **Pass the room's title and location to the cost arms**, as `climate` already is. No new vocabulary | 4 | Hinterwilds (`hinterwilds_location == 'EN' and Map.current.location =~ /the Hinterwilds/ ? 240 : nil`, and `'IM'`), on room 29865; Red Forest (`checkroom =~ /^\[Red Forest/ and redforest_location == 'EN'`, and `'WL'`), on room 24675. The room half is static and the converter settles it; the walker's half is `Remembered` |
| C2 | `Cond::Wearing(String)` — wears or carries at top level an item with exactly this name; `Walker.worn: Option<HashSet<String>>` | 8 | `cord-strung delicate brass key`, shape 3fa78d37. The crossings F and G found (keys in a sack, P10 below) want the same fact |
| C3 | `Cond::SkillCarriesLoad(skill, a, b)` — `b × ranks >= a × encumbrance%`; `("climbing", 4, 5)` is upstream's `climbing >= encumbrance / 1.25`. Integers, because `Cond` is `Eq` | 1 + a rung of C4 | shape f12391fa |
| C4 | `Cost::Ladder { rungs: Vec<(Cond, f64)>, otherwise }` — the first rung that holds sets the price; an unanswerable rung is passed over, so a ladder with an `otherwise` refuses nobody | 2 | the Graveyard wall 4140↔4141: any of Unlock/Consecrate/Bless Item/Force Projection known → 0.6; (one way only) the C3 climb → 120; Warrior of 15+ → 5.2; else 30. Upstream orders 120 before 5.2; port as written |
| C5 | `Cost::Hasted { wait, step }` — a timed wait Haste shortens: `floor(wait × max((80 − min(major elemental ranks, level)/5 − elemental lore air/5)/100, 0.4)) + step`, divisions by 5 whole; without Haste `wait + step`. Both pass, so unknown pays the full wait | 16 | one script, 16 exits. Needs `Walker.spell_ranks`. **OPEN: what are these rooms?** The agent guessed a timed drift; check before naming it. (Its note that spell 506 is Celerity is right and irrelevant: the script says `Spell['Haste']`, which is 535) |
| — | the day pass | 6 | reserved: a pre-flight, not a cost (`plan/21` §4.0) |

## Crossings (251 left)

| # | proposal | exits | notes |
|---|---|---|---|
| X1 | `Action::AwaitGroup` — wait for whoever followed the walker to rejoin it; nothing for a walker who leads nobody | 14 | the core of each script is a plain move. **Author's call**: port the wait, or rule that group waits are the group behaviour's business and drop them, which makes these trivial today |
| X2 | `Action::AwaitEscort` — when escorting a child or an official, wait up to five seconds for them to catch up | 11 | the same question as X1, for bounty NPCs. The minotaur maze and the swim table dropped this wait already, following Vellum |
| X3 | `Action::PutUntil(command, lines)` — send a command that does not change rooms until the game says one of these: a search that finds the hole, a lever that gives | 7–12 | G proposes the same with a `Cond` instead of lines (`PutUntil(search, Sees(gap))`); both forms occur |
| X4 | `Action::CastAt(spell, target)` — cast at something in the room and be carried by it: Phase at a banner. Waits for mana, recasts on Spell Hindrance. **Counts as a move** | 11 | shape ac921411 (10) and be49f96e (1). The cost should gate on `SpellKnown("Phase")` |
| X5 | `Cond::Sees(String)` — the room shows a thing with this noun, among its objects or in its description; `Action::WaitUntil(Cond)` | 7+ | F calls it `InRoom`, G `Sees`; one fact. Upstream sometimes matches the full name (`wooden cab`), so it may need a name as well as a noun |
| X6 | `Action::MoveAnyWhile(commands, Cond)` and `Action::WanderUntil(Cond)`, with `Cond::ExitsAre(Vec<String>)` — a maze of look-alike rooms: a random choice among moves while the question holds | 12–16 | **randomness must be seeded** (`plan/21` §4.0). Two shapes use Lich's `walk`; whether that is a uniform random exit needs reading in Lich |
| X7 | `Action::Moves(Vec<String>)` — several moves as ONE step under one guard, because a guard is asked when its step is reached and the second would be asked in a different room; `Cond::At(u32)` | 9 | a real gap in the flat-steps model, found by an agent: `if checkpaths.include?('ne'); move 'northeast'; move 'east'; end` cannot be two guarded moves |
| X8 | `Action::RoundWhile(commands, Cond)` — these commands in order, again and again, while the question holds; the last is a `TryMove`. A generalisation of `MoveWhile` that stays flat: it holds commands, not steps | 7 | `[search, go fissure]` while there is an exit east |
| X9 | `Action::KeepMovingAny(Vec<String>)` — `KeepMoving`, taking turns through several commands | 4 | shape 206005f6: `south`, `southwest`, `south`… until the room changes |
| X10 | `Cond::TryFailed` — the last `TryMove` did not move the walker; unlike `StillHere` it stays true while later steps move it | 5 | Maaghara roots: try the root, else take this room's detour and replan |
| X11 | `Action::Stance(String)` / `RestoreStance` — take a stance, remembering the one held, as `EmptyHands` does hands | 3 + round one's 9 | the model already bands the six stances |
| X12 | `Action::OrderByName(String)` — `inquire`, find the numbered line naming this destination, `order` that number: the number moves between visits, the name does not | 4 | two of the four scripts refuse a walker without silver (a cost gate), and all stop on bandits |
| X13 | A `…` wildcard in `Await` lines | 2 | the River's Rest ferry: `/…gangplank\.|River's Rest\.\.\..+ ashore!/` |
| X14 | `Action::TakeOut(item)` / `PutBack`, and **`{setting:name}` placeholders in commands**, filled from the travel profile (`get my {setting:key} from my {setting:key_sack}`) | 10 | private property: the key and its sack are profile settings. With C2 for "already wearing it" |
| X15 | `Action::MovesFromSetting(name)` — send the commands the profile lists under this setting, the last of which moves | 4 | `UserVars.X.each { |c| fput c }`; cost gated on `SettingIsSet` |
| X16 | `Action::MoveAsTold(pattern)` — move in the direction the game names in a line | 1 | `matchfindword` |
| X17 | `Action::AwaitCond(Cond)` for a stun — or the author's word that `Move`/`Put` wait out a stun already | ~6 | several ports sent `stand` unguarded after a fall; exact would wait out the stun first |

## Routines, one each

Puzzles with their own state. Named in the map, written in Rust in the Travel behaviour.

| routine | exits | what it does |
|---|---|---|
| `SpikeTrap` | 5 | room 7046: search and turn the spike until the exits are no longer `[out]`, be knocked down, stand; the landing is random, so it ends in `Replan`. Upstream hard-codes a walk from each landing instead |
| `BronzeGate` | 2 | `go gate` until through; between tries cast Unlock/Consecrate/Bless Item/Force Projection at it if known, else a Warrior of 15+ batters it with empty hands, else push |
| `RolarenGate` | 2 | open; if locked, climb it (a skill-bonus formula scaled by encumbrance, with Sigil of Resolve), or Phase through, or unlock. **The script refuses a walker who can do none** — a cost gate |
| `CrownDoor` | 1 | push the tine until aligned, burn 100 mana of spells at the crown, touch it, say the word |
| `FamiliarDoors` | 1 | the familiar-and-rings puzzle; cost gated on Call Familiar |
| `AltarLevers` | 1 | read the grid, then pull each coloured lever until it is at the position read |
| `RuneStaircase` | 1 | |
| `RingWedges` | 1 | turn the ring until each wedge shows its sigil, then push |
| `Mirror` | 1 | turn right to the stop, then tilt, with a flip recovery |
| `ThreePillars` | 1 | look at each pillar, cast the 70x spell its symbol names. Sorcerers only: a cost gate |
| `LabyrinthEntry` | 1 | touch the leaves (waiting while another player is at it), recite, then arrive within 20 s |
| `MuralOfDeities` | 1 | touch it, map each verse to a deity, answer each. Upstream has a `Char.name ==` special case that is dropped |

## Reserved for the main session

The three River's Rest errands and one sword-in-container errand (5.6 KB) start a second
trip to fetch something: a **stack of trips** in the Travel behaviour, the same mechanism
the silver detour needs (`plan/21` §4.7). The day pass is a pre-flight.

## Doubts the agents named, to check against the game

- `put 'go field'; put 'go field'` — which send moves the walker? Ported as `TryMove`, then
  `Move` when `StillHere`, right either way.
- `pull splinter` (36166→36167) assumed to be the room change.
- `go raft` assumed *not* to change rooms (upstream waits for the room to differ afterwards).
- Locked doors: upstream reads the game's answer; the port asks "did `go door` move me".
  Same result, different mechanism.
- `empty_hand` / `fill_hand` (one hand) became `EmptyHands` / `FillHands` (both).
- `Cond::StillHere` for a crossing that leaves and comes back: "has not moved yet", or "is in
  the origin room"? `TryFailed` (X10) is the first of those.
