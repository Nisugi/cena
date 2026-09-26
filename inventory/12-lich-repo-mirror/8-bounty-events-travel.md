# 8-bounty-events-travel: lich_repo_mirror survey

Scope: 364 scripts, 174 substantive; 18 read deeply, 156 at capture-line depth, 190 in the tail
by name. Cena HEAD 3566f8d (`git -C E:/Cena rev-parse --short HEAD`), 2026-09-25. Of the top 40,
the ones not read deeply are copies or tooling, diffed instead: the three sbounty forks against the
baseline, `ride2`/`go2enhanced` against go2, `ms`/`t`/`minibar`/`uberbounty` against each other,
`osacrew_avalon_wizard` against `osacrew`, `saferepo`/`mapmap` (tooling).

## Top findings

1. **Cena's bounty task parser holds up against real text.** `bounty.rs`'s 23 patterns, run in
   Python over the 80 bounty lines quoted in comments across the mirror, classify every complete
   sample (67) correctly; the misses are truncated comments, hand-typed test strings, and one
   single-space heirloom variant (B6, UNVERIFIED). What Cena lacks is **everything around the task**.
   HAVE; the gaps below are all M8.
2. **CONFLICT: `Come back in about a minute` is dropped.** Five scripts handle that line
   (`gemologist.lic:528`, `rrbountyspool.lic:115`, ...), all mapping it to one minute. `bounty_status.rs:157-161` parses the word
   `a` as a number, fails, and returns `None`; its test uses `about 1 minute`. HIGH, M8 (B1).
3. **CONFLICT: the removal line is documented but not read.** `bounty_status.rs:24` lists `I have
   removed you from your current assignment`; `classify_refusal` has no arm for it. The pile also
   shows the two-step removal (`You want to be removed ...` then `Very well`). HIGH, M8 (B2).
4. **GAP: the group-bounty assignment.** `You are to report to one of the <npc> ... in order to help
   <leader> take care of a bandit problem` (`go2bandits.lic:57`, `minibounty.lic:248`) matches no Cena
   pattern and is not in Lich's `bounty/parser.rb` either, so porting Lich will not bring it. The
   group flow around it is also absent: `You have completed this portion of your Adventurer's Guild
   task.`, the sign-up and confer lines (B5, B9, B10). HIGH, M8.
5. **PARTIAL: the `bounty` report's own timer is dropped.** `You are not currently assigned a task.
   You will be eligible for new task assignment in <time>.` (`parsebounty.lic:31`) matches `None`'s
   prefix and the wait is discarded (B20). Same report: unspent points, lifetime points, per-kind
   success counts (B15). HIGH/MEDIUM, M8.
6. **INFERRED, needs one corpus query: which stream Cena reads the task from.** Lich and every HUD
   here read the `bounty` stream (`xmlparser.rb:1177`); Cena classifies only main-stream chunk lines
   (`chunks.rs:327`, `streams.rs:173`). Safe only if every `bounty` push has a main copy, including
   unasked pushes (kill counts, completion). HIGH, M8.
7. **Guild answers and in-task events Cena lacks** (B3, B4, B8, B11-B14, B16, B18, B21): expedite
   results, four more refusals (`kind of busy`, `no tasks suitable for you`, `want to head to
   another`, `I don't seem to have`), `which looks like the heirloom that you are searching for!`,
   escort ambush and trap lines, escort follow dialogue, NPC hand-offs, bounty boosts, the undefined
   `It is your duty to oppose the ...`. HIGH/MEDIUM, M8.
8. **Escort data is in the baseline, not in Cena.** `ego2.lic` (ancestor of baseline
   `escortgo2.lic`, 66% shared lines) has the pickup-phrase -> room table, drop-off rooms, and 96
   per-edge escort crossings (87 in escortgo2). `plan/21` §2d counts only ~11 escort-aware edges,
   those inside the mapdb; these 87 are outside it. HIGH/MEDIUM, M8.
9. **Travel: `movement.rs` already has every move-failure line the pile's travel scripts match**;
   the go2 forks add none. Two real gaps: the **elven day pass** (Ta'Illistim <-> Ta'Vaalor,
   `chrono.lic:85`) is in neither the mapdb (1276 and 13779 have no edge between them) nor
   `day_pass.rs` (3 towns); and `day_pass.rs:582` never reads the `raise` answer (`not valid for
   departures`, `pass is expired`). MEDIUM (T1, T2).
10. **OSA (ask 2) is GemStone's player-ship system**: crews, cannons, boarding, Sea Hag's Roost
    tasks, the kraken's Tenebrous Cauldron. The `osa*` family is a captain/crew pair talking over
    LNet. Its asset is an **ocean map the mapdb lacks entirely**: 1,252 rooms in `osa-map-plugin`,
    1,283 in `ocean-go2` (the 37k lines are this table), a third copy on GitHub. LOW, no milestone.
11. **Events (ask 4): Cena has the balances and item types, not the events.** Every scrip is read
    (`currency.rs`) and Lich's event item types are ported (`gameobj-data.tsv:18,85,100,102`); no
    capture for fox/pixie hunts, raffles, Delirium Manor passes, arena winnings, `event entries`.
    One data set worth MEDIUM: 70 **GemStone jewel property** names by rarity
    (`generate-gemstone.lic:21-27`), absent from Cena and from Lich core. LOW otherwise.
12. **Guilds (ask 5): the `gld` report is a GAP.** Skill ranks, the Training Administrator's task,
    reps left, promotion due, check-in due (`grguild.lic:2421-2466`, `rogue-gambits.lic:64-86`);
    plus a rogue task catalogue and guild dues for 11 professions (`checkin.lic:28-40`). Guild entry
    puzzles are HAVE (`three_pillars.rs:25-30`, `guild_password.rs`). MEDIUM, no milestone.

## Data tables Cena lacks or differs on
| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| ego2.lic:165-172 (Tillmen, v0.6, 2020) = baseline `escortgo2.lic:187-194` | escort pickup phrase -> town, room | 6 | the `Go to <phrase>` text of an escort task, town, Lich room id (e.g. `area just inside the Sapphire Gate` -> Ta'Illistim 34) | none; `bounty.rs:380` captures `start` as free text only | GAP (in the baseline too, so port from `escortgo2`) | HIGH, M8 |
| ego2.lic:174-181 = `escortgo2.lic:196-203` | escort drop-off rooms by town | 6 (Zul Logoth has 2) | town -> room ids | none | GAP | HIGH, M8 |
| ego2.lic:187-230 | return-trip silver by town pair | 6x5 | from, to, silver (0 / 20 / 2000 / 2020 / 4020) | the travel pre-flight prices trips from the map (`travel/drive/preflight.rs`) | superseded: the map prices it | LOW |
| ego2.lic:162-163 | escort nouns; NPCs an escort may ignore | 6; 7 | `traveller magistrate merchant scribe dignitary official`; `kobold rolton urgh ridge orc hobgoblin velnalin fire ant` | nouns HAVE: `gameobj-data.tsv:19` (type `escort`, by race + noun) and `:47` (passive npc nouns); the ignore list none | HAVE / GAP (ignore list) | LOW, M8 |
| ego2.lic:96 edge overrides vs escortgo2's 87 (`comm` of the two lists) | escort-aware crossings | 96 | map edge -> a proc that waits for the escort | `plan/21` §2d counts "Escort-aware moves ~11 edges, 3 families" in the mapdb itself; the 87 baseline overrides are outside the mapdb | GAP (plan/21 has not counted these 87) | MEDIUM, M8 |
| bountyhunter.lic:1970-1982 (Alastir) | bandit areas -> hunting rooms | 7 | area (Gyldemar Road, Sylvarraend Road, Widowmaker's Road, Muddy Village, Black Weald, Cliffwalk, Whistler's Pass) -> 4 Lich room ids | none; `creature_areas.tsv` holds creature uid ranges, not bandit areas | GAP | MEDIUM, M8 |
| go2bandits.lic:23-24 (Xanlin, v6) | rooms whose `location` lies | 17 | Whistler's Pass 37-41 are in town; 12 Widowmaker's Road rooms 30609-30817 | none | GAP (map quirks for a location-based search) | MEDIUM, M8 |
| swapbounty2.lic:46-52, bountyhunter.lic:1353-1768, ebounty baseline `:962-990` | bounty NPC names | ~25 | guard: `Luthrek Witlass tavernkeeper Bumbleberry Rex Nessy Sleetwood Smiley Halfwhistle Belle purser`; herbalist rooms `3824 1851 10396 640 5722 2406 11002 9505`; Illistim gem/fur NPC `Cysaegir`; room 10327 `areacne` for both | none. The baseline ebounty has `areacne` (`:972`) and `Halfwhistle` (`:2639`, by room) but not `Luthrek Witlass Bumbleberry Rex Nessy Sleetwood Smiley` | GAP (partly beyond baseline) | MEDIUM, M8 |
| StartVaalor.lic:12-27 | Ta'Vaalor guards by name -> room | ~10 | `Vontrilaias 5907`, `Rethustril 5827`, ... (5827 is ebounty's `advguard2`, `bountyhunter.lic:10`) | none | GAP | LOW, M8 |
| chrono.lic:106-128, 245-248 | Chronomage stations | 5 towns | town -> clerk room, portal room, clerk noun (`clerk halfling agent attendant`) | mapdb tags `chronomage` on 8634, 8916, 14358, 13169 (Python over the JSON); `day_pass.rs` `TOWNS` has 3 | PARTIAL (elven pair missing, see T2) | MEDIUM |
| checkin.lic:28-40 | profession guild dues | 11 | profession, initiation fee, monthly dues | none | GAP | LOW |
| grguild.lic:2566-2695 | rogue guild tasks | ~30 | task phrase -> handler | none | GAP | LOW |
| volnstep.lic:194-798 (Jennora, v2.1 BETA) | Voln step tasks | ~40 | `XMLData.society_task` phrase -> step (e.g. `take a bleeding heart rose to the grave marker of the lovers`) + rooms, Landing and Vaalor | `societies/membership.rs` reads the step number only; the task text is not read (`grep -rn "next step" crates/cena-model` = 0 society hits) | GAP | LOW |
| col.lic:18-40 (spiffyjr, v1.0) | Council of Light tasks | 2 Q&A, 13 items, 3 rooms | question -> answer; item -> shop room ids; poohbah and entrance rooms | `societies/membership.rs:119-120` has the High Taskmaster lines; tasks none | GAP | LOW |
| generate-gemstone.lic:21-27 | GemStone jewel properties | 70 | name by rarity (25/14/15/16) | none | GAP | MEDIUM (loot) |
| osa-map-plugin.lic:72-8880; ocean-go2.lic:201-36265 | OSA ocean map | 1,252; 1,283 | room, sea, exits by direction, map hash or ASCII, grid coords, uid, port tags | none; the mapdb has no ocean rooms | GAP | LOW |
| node.lic:25-130 | super nodes by town | ~60 | name, room id | mapdb tags 91 rooms `supernode`; 12 of 14 sampled Landing/Icemule ids carry it (13500, 19290 do not) | PARTIAL | LOW |
| decryption.lic:1-50 | a quest lexicon | ~90 words | invented word -> English (`umi` one, `krio` bread, `mubzh` north ...) | none | GAP (quest-only) | LOW |
| event_support.lic:47-49, ci_games.lic:105-126, duskruin_adventurer.lic:5 | event prize lists | 3 lists; 5 tiers | item names by disposition | `gameobj-data.tsv:85,100,102` types | PARTIAL | LOW |

## Captures Cena lacks or differs on

### Bounty (special ask 1). Cena: `crates/cena-model/src/state/bounty.rs` (task text, ported from Lich `bounty/parser.rb`), `bounty_status.rs` (guild answers, from `ebounty.lic`), `ledger/hunt.rs:58` (the reward line)

**The task descriptions themselves are covered.** I transcribed `bounty.rs`'s 23 matchers into Python
(`tools/8-bounty-events-travel/cena_bounty.py`) and ran them over every real bounty line quoted in a
comment anywhere in the mirror (80 lines, 13 scripts; `samples_raw.txt`, `samples_result.tsv`).
67 classify to the kind the comment says, captures sane (creature, area, town, number, npc, gem, herb,
skin, quality, start/destination). The 13 misses are: 6 comments truncated mid-sentence, 3 hand-typed
test strings in `gembuyer.lic` with one space after a period, and **one real-looking variant copied into
four scripts** (row B6). No sample of any task kind failed for a reason in Cena's pattern except B6.

| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| B1 gemologist.lic:528, bandit_hunter_follower_support.lic:42, rrbountyspool.lic:115, bountyhunter.lic:1335, sbountybeta.lic:1235 | guild wait, one minute | `Come back in about a minute if you want another task.` (a word, not a digit; every script that handles it maps it to 1) | `bounty_status.rs:157-161` strips `Come back in about `, splits at the space, and `"a".parse()` fails: returns `None`. The test `tests/bounty_status.rs` uses `about 1 minute`, a form no pile script shows | **CONFLICT** (the line is dropped) | HIGH, M8 |
| B2 bountyhunter.lic:1293-1297, sbountybeta.lic:593-596, rrbountyspool.lic:83 | removal, two-step | `ask taskmaster about remove` -> `^<npc>.*?want to be removed` / `You want to be removed from your current task assignment`; ask again -> `Very well` / `^<npc>.*?have removed you` | `bounty_status.rs:24` **documents** `I have removed you from your current assignment` as read; `classify_refusal` (`:146-162`) has no arm for it and nothing else matches `removed` (`grep -n removed bounty_status.rs` hits only the doc line) | **CONFLICT** (doc says read, code does not) | HIGH, M8 |
| B3 gemologist.lic:507-519, bountyhunter.lic:1317 | expedite answers | `Very well, <name>.  I'll expedite your task reassignment.` / `You don't seem to have any expedited task reassignment vouchers` / `I can't expedite this task reassignment, <name>.  I still need to complete the paperwork on the task you just finished` | only the voucher COUNT line, `bounty_status.rs:138-142` | PARTIAL | HIGH, M8 |
| B4 undead_bounty.lic:280-281, bountyhunter.lic:1331, sbountybeta.lic:1232 | taskmaster refusals besides the three Cena has | `I'm kind of busy right now`; `... don't seem to have any tasks that are suitable for you` (typed request `ask taskmaster for bounty <type>` too hard/easy); `but I don't seem to have` (scripts park 9999 min); `want to head to another` (sbountybeta parks 60 min) | `bounty_status.rs:146-153`: `already been assigned`, `I don't have any tasks for you right now`, `Come back in about` | PARTIAL (4 refusals absent) | HIGH, M8 |
| B5 go2bandits.lic:57 (Xanlin, v6), minibounty.lic:248 (Demandred, v1.2) | **group-bounty assignment** | `You are to report to one of the <npc> ... in order to help <leader> take care of a bandit problem` | `bounty.rs:230-237` `BanditAssignment` needs `It appears they have a bandit problem`; not in Lich `bounty/parser.rb` either (`grep -n "in order to help" reference/lich-5/lib/gemstone/bounty/` = 0) | **GAP** (`classify` returns `None`) | HIGH, M8 |
| B6 minibar.lic:183, ms.lic:216, t.lic:206, uberbounty.lic:160 | heirloom, one space | `...initials SM engraved upon it. Hunt down the creature and LOOT the item from its corpse.` (one space after `it.`; the same scripts' other samples have two) | `bounty.rs:390` requires `upon it\.  ` (two spaces) | UNVERIFIED variant: one copied comment; a corpus grep would settle it | MEDIUM, M8 |
| B7 minibounty.lic:215-233, bountyhunter.lic:2280 | heirloom **initials** | `The heirloom can be identified by the initials (?'initials'.*?) engraved`; the found item is confirmed by `look` -> `^Engraved .* initials` | `bounty.rs:390` matches `initials \w+` but does not capture them. Port-data-whole says keep them: they are how the found item is identified | PARTIAL | MEDIUM, M8 |
| B8 sbountybeta.lic:1095 | heirloom seen while searching | `which looks like the heirloom that you are searching for!` | none (`grep -rn "heirloom that you" crates` = 0) | GAP | HIGH, M8 |
| B9 bandit_hunter_follower_support.lic:31,241, bandit_hunter_support.lic:68, bandit_hunter_follower.lic:37,106 | partial credit, group task | `You have completed this portion of your Adventurer's Guild task.` | none (`grep -rn "completed this portion" crates` = 0) | GAP | HIGH, M8 |
| B10 bandit_hunter_follower_support.lic:162,185,198, bandit_hunter_support.lic:96-99, hw_hunter_follower_support.lic:61 | group sign-up | `<leader> has signed you up to join him on his task.`; `<leader> confers quietly with Taskmaster Lucrecious.`/`...with Luthrek.`/`...with the guardsman.`; `Luthrek gives <leader> a strange look.`; `<x> nods to the city guardsman.` | `data/spells.tsv:429` (`9056 Next Group Bounty`) holds the sign-up line as a **cooldown start message** only; no bounty-state reader | PARTIAL | MEDIUM, M8 |
| B11 bountyhunter.lic:1360-1361,1406-1407,1621-1622,1680-1681,1805, nesbounty.lic:1267,1364 | the NPC hand-off | herbalist/furrier/gem dealer: `Yes, I do have a task for you` + `I've recently received an order for N <thing>.` / gem dealer `interested in purchasing an? <gem>. ... go round up N of them`; guard: `Yes, we do have a task for you` / `Ah, so you have returned` | none (`grep -rn "Yes, I do have" crates` = 0). Cena reads the task once `bounty` restates it, so this is only the answer that says the ASK worked | GAP | MEDIUM, M8 |
| B12 bountyhunter.lic:1382, 1648, 1747; sbountybeta.lic:433 | turn-in answers | herb give: `This looks perfect` / `That looks like it has been partially used up`; `You've collected all the samples I asked for`; `The furrier takes` | ebounty (baseline) has the first two at `:3305`; Cena none (`grep -rn "This looks perfect" crates` = 0) | GAP | MEDIUM, M8 |
| B13 undead_bounty.lic:112,325-326, rrbountyspool.lic:119, bounty_instant.lic:34, bountyhunter.lic:2316 | guard and taskmaster turn-in | guard asleep: `is in no condition to talk to you`; guard done: `successfully completed your task`; taskmaster: `<npc> says, "All done with that assignment?  Good job, <name>!"` | the reward line only, `ledger/hunt.rs:58` | PARTIAL | MEDIUM, M8 |
| B14 undead_bounty.lic:255-258, rrbountyspool.lic:91-96 | bounty boosts | `boost bounty` -> `You have activated a Bounty Boost.` / `You do not have any Bounty Boosts to redeem.`; guild boost -> `You have been awarded 1 Bounty Boost` / `You do not have any Guild Boosts to redeem` | none; ebounty reads the balance from `<d cmd='boost bounty'>Bounty Boosts</d>: N` (`ebounty.lic:2422`), also absent (`grep -rn "Bounty Boost" crates` = 0; the 20 `Boost` hits are other boosts, e.g. `ledger/hunt.rs:50` Long-Term Experience Boost, `spells.tsv:504-512`) | GAP | MEDIUM, M8 |
| B15 prettybounty.lic:19,43, star-wealth.lic:135-137 | the `bounty` report's history lines | `^You have succeeded at the <kind> task N times( and failed N times)?.`; `^You have accumulated a total of N lifetime bounty points.`; `N unspent bounty points` | none (`grep -rni "lifetime bounty\|unspent" crates --include=*.rs` = 0 game hits) | GAP | MEDIUM, M8 |
| B16 ego2.lic:298 (= baseline `escortgo2.lic:301`) | escort and bandit ambush | `<escort> fearfully exclaims, "It's an ambush!"`; compass-line `quickly approaches / suddenly leaps from / leaps out of / suddenly jumps out of the shadows`; traps `carefully concealed metal jaws`, `nearly invisible length of razor wire`, `carefully concealed inflated pouch`, `looped rope`, `net`, `pit`, `Suddenly, the ground gives out from under you as you fall into a shallow pit filled with tiny spikes!` | Cena has two bandit reveal lines as hiding prose (`overwatch.rs:142-143`), nothing for the escort, the traps or the ambush | PARTIAL | HIGH, M8 |
| B17 azbandit.lic:288 | bandit ambush openers | `^You hear a voice shout, "Die`; `^A plain wooden arrow flies out of the shadows`; plus the two Cena has | `overwatch.rs:140-146` (`flies out of the shadows toward you!` generic, silvery light, jet black crystal) | PARTIAL (`voice shout, "Die` absent) | MEDIUM, M8 |
| B18 ego2.lic:1698-1733 | escort NPC follow | `tell <npc> to follow` -> `<noun> nods and says to you` / `<noun> says to you, "But I already am!"` / `"The guild didn't hire you to guide me."` / `The merchant gives you a strange look.`; `wait` -> `Time drags on by...`; escort nouns `traveller\|magistrate\|merchant\|scribe\|dignitary\|official` (`:162`) | the escort NPC is typed (`gameobj-data.tsv:19` type `escort`, `:47` nouns); the dialogue none (`grep -rn "Time drags on\|already am" crates` = 0; `grep -rni escort crates --include=*.rs` hits only a doc comment, `confluence.rs:28`) | PARTIAL | HIGH, M8 |
| B19 rescue-watch.lic:30-70 | death announcements, by town | `* X just bit the dust!` (Landing), `was just put on ice!` (Icemule), `just took a long walk off of a short pier!` (Solhaven), `just punched a one-way ticket!` (KD), `is six hundred feet under!` (Zul), `has gone to feed the fishes!` (RR), `just turned her last page!` (Illistim), `is going home on his shield!` (Vaalor). The last two hard-code one gender, so the real line varies | none (`grep -rn "bit the dust\|put on ice" crates` = 0); Despana merges the `death` stream as text only | GAP | LOW (hub display; no behavior) |
| B20 parsebounty.lic:28-31 | the `bounty` report's timer | `You are not currently assigned a task.  You will be eligible for new task assignment in <time>.` / `...  You are eligible for new task assignment.`; header `your Adventurer's Guild information is as follows`; `You currently have N unspent bounty points.` | `bounty.rs:230` matches only the prefix `^You are not currently assigned a task` -> `TaskKind::None`; the eligibility wait in the same sentence is dropped. `bounty_status.rs` gets a wait only from the taskmaster's `Come back in about`. A second source exists: the `Cooldowns` dialog's `Next Bounty` row, which `effects.rs:64` keeps by dialog generically (INFERRED; it is what ebounty reads, `ebounty.lic:2050`) | PARTIAL | HIGH, M8 |
| B21 nextbounty.lic:50, swapbounty.lic:117, swapbounty2.lic:117,135 | a bounty text the scripts treat as "no task" | `It is your duty to oppose the ...` (they `ask taskmaster about bounty` on it) | `classify` returns `None` (`grep -rn "duty to oppose" crates` = 0). Meaning UNVERIFIED: the scripts show only the prefix | GAP | MEDIUM, M8 (a corpus grep would name it) |
| B22 bandit_hunter_follower.lic:101 | the guard's hand-off, cull | `Killing N of them should do nicely.  Report back to me when you are done.` | none | GAP | LOW, M8 |
| B23 shat_bounty_parser.lic:47,64,76 (Tykus, GSF only) | Shattered guard phrasing | `...should bring it back to the .* of .*\.`, `...back alive to the .* of .*.`, `...report back to the .* of .*\.` | `bounty.rs:214` `GUARD` knows only `the purser of`, `the captain of the` among `the X of Y` forms | UNVERIFIED (Shattered may name guards differently) | LOW |
| B24 swapbounty2.lic:108-113 | Kraken's Fall taskmaster | the guild master there is `Halfwhistle`, room 29866; `ask Halfwhistle to expedite` / `about removal` | `bounty.rs:468` knows Contempt's NPC names only; the baseline ebounty knows Halfwhistle by room (`ebounty.lic:2639`) | PARTIAL (data for the behavior) | LOW, M8 |

**Where the bounty text arrives (INFERRED, worth a corpus check).** Lich reads the task from the
`bounty` stream (`reference/lich-5/lib/common/xmlparser.rb:1177-1178`, `checkbounty` =
`XMLData.bounty_task`, `global_defs.rb:1210`), and every HUD script in this pile does the same
(`minibounty.lic:215`, `uberbounty.lic`, `ms.lic`, `minibar.lic:169`). Cena's `BountyStatus` is fed
only main-stream chunk lines (`chunks.rs:327`, gated at `streams.rs:173`); the `bounty` stream is
buffered (`streams.rs:230`) but not classified. That is safe **if** every `bounty` stream push has a
main-window copy: `stream_windows.rs:39` records `bounty` as `ifClosed=''`, which the wiki says means
a duplicate. What it does not show is whether the game pushes the stream **unasked** (a kill count
ticking down, a task completing) with no main copy. If it does, Cena misses every update Lich sees.
One corpus query (`<pushStream id="bounty"` with no matching main line) settles it.

### Travel (special ask 3). Cena: `cena-model/src/movement.rs` (the whole `move.rb` ladder), `state/movement.rs` (`MoveFailure`, 14 causes), `cena-behavior/src/travel/` and its 29 routines, over the Lich mapdb (`reference/mapdb/map-1789942730.json`)

**Move-failure text: Cena has everything the pile's travel scripts match, and more.** The pile's
go2 forks (`go2enhanced.lic`, `ride2.lic`) add **no** move-response pattern that the baseline go2
lacks (`uniq_caps.py` over both, against `reference/scripts/scripts/go2.lic` and
`reference/mapdb/go2.lic`); their extras are settings and one fog room. Cena's
`movement.rs:93-188` is Lich's full `move.rb` ladder. What the pile adds is **transport-specific**
text, below.

| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| T1 chrono.lic:65-71, speed2.lic:186, jfloo.lic:161-168 | day pass raised: did it work? | `You raise your pass up, presenting it for inspection.`; `...a voice echo in your head, "Your pass is not valid for departures from this realm."`; `pass is expired`; arrival `whirlwind of color subsides` (all six mapdb day-pass procs carry the last three: `grep -o "whirlwind of color" map-1789942730.json` = 6) | `day_pass.rs:582-594`: `raise #id` is a `Put`, the answer is not read; the next state drags the pass back into the sack | PARTIAL (a refused pass is found only by the trip noticing where it stands) | MEDIUM, M6/M8 travel |
| T2 chrono.lic:85, 121-128, 245-248 | the **elven** day pass, Ta'Illistim <-> Ta'Vaalor | pass text `...between the towns of Ta'Vaalor and Ta'Illistim...`; stations TI 13169/1276, TV 5883/13779 | the mapdb has no edge between them: 1276's only exit is `south`, 13779's only exit is `go staircase` (Python over the JSON). `day_pass.rs` `TOWNS` has 3 entries (wl, imt, sol) | GAP in the map and in Cena | MEDIUM, M8 travel |
| T3 chrono.lic:82 | a three-town pass | `...between the towns of Icemule Trace and Wehnimer's Landing and Solhaven...` | `day_pass.rs:249-257` splits at the first ` and `, so `towns` = ("Icemule Trace", "Wehnimer's Landing and Solhaven") and `serves` never matches | UNVERIFIED (one listing, no sample) CONFLICT if real | LOW |
| T4 chrono.lic:225-248, bacon.lic:24 | the `location` command | `You carefully survey your surroundings and guess that your current location is <X> or somewhere close to it.` (X = town, `the Graveyard`, `the Abbey`, `the free port of Solhaven`, `the city of Ta'Vaalor`) | none (`grep -rn "carefully survey" crates` = 0) | GAP | MEDIUM (a "which town am I near" fact for bounty and travel) |
| T5 fly.lic:237, 274-281 | Chronomage departure timer; orb capsule | `The next distant departure will be in less than a minute\|in N minutes`; capsule `setting the active chamber to`, `is ejected from the\|chamber is empty` | none (`grep -rn "distant departure\|active chamber" crates` = 0) | GAP | LOW |
| T6 fly.lic:66-75 | premium teleport list | `premium start` output parsed for `PREMIUM INFORMATION` and destinations (Ornath, Cysaegir, Kraken's Fall ...) | none | GAP | LOW |
| T7 IFW_teleporter.lic:24-123 | Mist Harbor (FWI) trinket | `surrounded.*?light`, `scintillating\|waxing` (charge), `That does not work here.`, `seems to stick when you turn your`, `You can't turn that` | `routines/trinket.rs` (31 exits) exists; its literals do not include these (`grep -n "stick when\|does not work here" trinket.rs` = 0) | PARTIAL | LOW |
| T8 ego2.lic:778-826 | Locksmehr River ferry (edges 1189-1193) | `give ferryman 10` -> `Thank you, friend.  Go ahead and board.` / `Sorry, friend, break time for me.  Not taking tolls.`; docking `The ferryboat bumps up against the dock...`; `^The ferryman.*Everybody out` | mapdb 1190->1191 is a bare `southwest`. The baseline `escortgo2.lic` **dropped** these crossings (`comm` of the two edge lists: ego2 96, escortgo2 87; ego2-only = the six ferry edges, 1139->1140, 1143->1142, and one key ego2 declares twice) | stale: superseded upstream | LOW |
| T9 ego2.lic:1172-1253 | Zul Logoth mining cart | `buy ticket` -> `You already bought a ticket.` / `The dwarf .*You now have passage`; `You hastily enter the mining cart.`; `You hastily exit the cart` | mapdb 1260->1261, 1266->1267 carry `buy ticket` + `waitfor "You hastily exit the cart"`; Cena converts those procs | HAVE (through the map) | - |
| T10 ego2.lic:1266-1341, 1497-1547 | rope crossing, rope ladder | `No one is currently crossing the river on the rope.` / `is currently hanging on the`; `shuffles slowly across the rope and hops off of it`; `You finally reach the shore and you pull yourself to safety...`; `The rope ladder is not being used by anyone at the moment.` / `climbs down the rest of the ladder slowly` | mapdb: `look rope` 0 occurrences; the baseline escortgo2 keeps the ladder wait (escort-only) | GAP (escort-only waits) | LOW, M8 escort |
| T11 sailors_grief_swim_fix.lic:144-255 (Tysong, v1.0.0) | Sailor's Grief swims | a mapdb hotpatch: `somewhere to the <dir> of` / `is just a short swim away` decide the swim; header says "will eventually be included into GSIV Prime mapdb" | mapdb: `short swim away` 0 occurrences | GAP (map data pending upstream) | LOW |
| T12 fastswim.lic:19 | Nelemar swims 12662<->12677 | patch removes `sleep 1` per room when swimming has no RT | a cost, not a fact | N/A | - |
| T13 go2enhanced.lic:1125 | Ithzir fog room | `Room.current.title =~ /Lost in the Fog/` -> `out` | mapdb has the title once; Cena none | GAP | LOW |
| T14 go2enhanced.lic:936-969, ego2.lic:331 | justice | `justice status` -> `There is no justice other than your own out here.` / `You sense that your surroundings are calm enough`; room players `the body of` / `a stunned` | none (`grep -rn "no justice\|calm enough" crates` = 0) | GAP | MEDIUM (dragging, hiding, and escort rules turn on it) |
| T15 gondola.lic:4-18 | Caligos Isle gondola | `look at runes` -> colour -> direction (yellow north, orange south, purple west, green south) | none (`grep -rn Caligos crates` = 0) | GAP | LOW |
| T16 teleport.lic (Ondreian, v1.2.0) | n-setting teleporter | generates mapdb patches for the teleporter's jump points per town (`:171-185`) | none | GAP (map extension) | LOW |
| T17 116locate.lic:31-60, go2locate.lic:20-22 | Locate Person (116) | `Your vision begins to become murky and clouded`, `the distance is too great`, `must be beyond the reach`, `establishing the link and suddenly you see` + the room name | none | GAP | LOW |

**Does travel data here beat the mapdb?** Ferries, carts, the Chronomage, portals: **no**, except
T2 (the elven day pass) and T11 (Sailor's Grief swims). The OSA ocean is the one large data set the
mapdb lacks entirely (next section). Rumor Woods' fox maze is in the mapdb already: all 40 uids
`8208601..8208640` resolve to mapdb rooms (Python over the JSON), which is where `rumor_woods.lic:653`
froze its graph from.

### OSA, Open Sea Adventures (special ask 2)

**What it is.** OSA is GemStone's player-ship system: a character owns a ship (sloop, brigantine,
carrack, galleon, frigate, man o' war; `gangplank.lic:152`), crews it with other characters, sails an
ocean grid between ports, fights enemy ships (Krolvin, pirate, ethereal) with cannons and boarding,
takes tasks from a bird at the **Sea Hag's Roost**, and meets the kraken in the **Tenebrous
Cauldron**. The `osa*` family is two cooperating scripts run by a whole party: `osacommander.lic`
(Peggyanne, v4.1.0, 2026-09-20) on the captain, `osacrew.lic` (v6.1.0, 2026-09-20) on each crewman,
talking over LNet private messages (`[Private]-GSIV:<name>: "Crewman X Reporting For Duty Captain"`).
`osacrew_avalon_wizard.lic` is the same crew script at v5.8.6 for other frontends; `osaunderway`,
`osaanchor`, `osasails`, `osapicker`, `osanav`, `osatask`, `osaorders`, `osacannon_watch`,
`osasail_watch` are its satellites. `osa-commands.lic` (Jymamon) and `osa-map-plugin.lic` (Jymamon)
are a separate lineage.

| script:line | data or capture | Cena |
|---|---|---|
| osa-map-plugin.lic:72-8880 | **the ocean map**: 1,252 rooms (`grep -c ":location=>"`), 25 seas of 50 rooms each, each with `:maphash` (MD5 of the in-game `look map` ASCII), `:wayto` by compass direction, `:map_coords` (row, col), `:roomid` (the game's uid), port tags. Source: a public Google sheet | GAP. The Lich mapdb has none of it: `North Blue` 0 occurrences in `map-1789942730.json` |
| ocean-go2.lic:201-36265 (Dreaven, v10) | the same ocean, 1,283 rooms (`grep -cE "^[0-9]+ => \{"`), 30 seas, each keyed by the **ASCII map itself**; 11 ports; a written list of in-game bugged exits (`:80-195`) | GAP. The 37k lines are almost entirely this table |
| sail2.lic:90-92, 249 | a third copy, downloaded from GitHub `jmbreitfeld/OSA-Ocean-Database` (`oceandb.json`, not in the mirror); 12 ports by uid | GAP |
| osacrew.lic:541-1721, osacommander.lic:1389-1416, osaanchor/osasails/osaunderway | ship handling: `lower sail` -> `...until it is at half mast` / `fully open` / `far as it can go!`; `push capstan` -> `begin to push` / `one final push` / `anchor is already up`; `assess` damage per deck (`Main Deck:  It appears to be`, `[Health of your ship: N/M]`); `get wood`/`fix` repair; `get balls`/`load cannon`/`fire cannon` with `You cannot fire your cannons while boarded` | GAP |
| osacrew.lic:4238-4410, piratehunter.lic:19-61, gunnersmate.lic:24-270 | sea combat: `You notice (a krolvin\|an ethereal\|a dark) X approaching your position`; `A distant thudding of the drums of war...`; `...materializes next to your ship!`; `The sides of the X collide against your Y`; `in boarding range!`; `The (Krolvin\|Pirate\|Ethereal) Captain shouts in rage aboard...`; `Tenebrous Cauldron.  Victory is yours!`; `rapidly descends beneath the cold, dark waters` | GAP |
| osacrew.lic:1767-1787, osatask.lic:98-172 | Sea Hag's Roost tasks: `You do not currently have a task from the Sea Hag's Roost`, `You should return to the Sea Hag's Roost to report your success`, `Abandons your current task`; the task bird in the crow's nest | GAP |
| ocean-go2.lic:37378-37478, osa-map-plugin.lic:9242-9280, stolenvalor.lic:77-129 | navigation: `The <ship> drifts slowly <dir>, its sails fully raised.` / `cuts through the ocean, heading <dir>`; `turn wheel <dir>`; `wheel slowly turns off course`; `Open waters: <sea>`; `You are unable to pinpoint your location under the current weather conditions!` | GAP |
| newenemy.lic:1-345, MapEnemyShip.lic | enemy-ship rooms: extra room descriptions for mapdb rooms 30266-30272 (`[Enemy Ship, *]`, location `The Tenebrous Cauldron`; the mapdb holds 1-6 descriptions each) so room matching works on every enemy hull | PARTIAL: the rooms exist in the mapdb, the variants do not |
| osacommander.lic:2157-2216 | loot on a captured ship: box `loot` -> `...note some interesting treasure but are unable to take any of it` / `There is no loot inside the X` / `...remove Y which you promptly stow away.`; `loot room` -> `In a desperate attempt to pick up and stow...` | HAVE mostly: `cena-behavior/src/loot/outcome.rs:83,149` (`There is no loot`, `up and stow` + `treasure`); the box-loot partial-take line is not distinguished |

**Value: LOW, no milestone.** OSA is a self-contained minigame with its own map; nothing in M6-M8
touches it. If Hydra ever sails, the ocean map is the asset, and there are three independent copies
to reconcile (`osa-map-plugin`, `ocean-go2`, `OSA-Ocean-Database`).

### Events and festivals (special ask 4)

**What Cena has: the balances and the item types, not the events.** `state/character/currency.rs`
reads every event scrip (`Duskruin Arena - N bloodscrip.`, `Ebon Gate - N soul shards.`,
`Rumor Woods - N raikhen.`, `Reim`, `Inquisitor - N aevit.`, `Troubled Waters`), and
`data/gameobj-data.tsv` carries Lich's event item types (`:18` `ebongate`, `:85` `event:duskruin`,
`:100` `simucoin:regular` incl. `Adventurer's Guild voucher pack`, `:102`
`simucoin:deliriummanor` invitational passes). No event behavior, no event capture.

| script:line | event | data or capture | Cena (searches) | verdict | value |
|---|---|---|---|---|---|
| E1 rumor_woods.lic:295-410, 639-816 (Alastir, v2.17.18, 2026-04-16); foxpixiehunt.lic (Arianiss, v2.6, 2026-04-26); fox.lic, fox2020.lic, foxy.lic | Rumor Woods fox and pixie hunts | `observe`/`track` -> `many signs of a (fox\|pixie) in this area` / `no signs of a` / `paw prints of a small animal` / `incredibly tiny footprints`; `A fox trots <dir>!`; `A pixie slips through the foliage, heading <dir>!`; payouts `You exchange X with Eyaeno who hands you Y and N raikhen` (six shapes, `:402-410`); maze graphs frozen from the mapdb (`:653`, `:712`) | raikhen balance HAVE; `grep -rn "signs of a fox\|Eyaeno" crates` = 0 | GAP (captures) | LOW |
| E2 star-wealth.lic:159-167 | event entries | `event entries` -> `Duskruin Entries: N`, `Ebon Gate Entries: N`, `Rumor Woods Entries: N` (rumor_woods.lic also gates on `;event ent`) | `grep -rn "Entries:" crates` = 0 | GAP | LOW-MEDIUM (a balance beside the scrip Cena already reads) |
| E3 duskruin_loot.lic:35-86, event_support.lic:14-60 | Duskruin Arena prizes | `You open an arena winnings package.` / `In the winnings package`; prize nouns by disposition (bin, cloak, lootsack) and three item lists (`items_to_sell`, `adventurer_items`, `junk_items`) at room 25577 | item types HAVE (`gameobj-data.tsv:85`); `grep -rn "winnings package" crates` = 0 | PARTIAL | LOW |
| E4 dm_runner.lic:91-154, dm2.lic:59-124 (Alastir, 2018) | Duskruin Delirium Manor | invitational pass: `It currently has N entries left and has been marked N time(s) as being used.`; search: `You search through some sundries and find X` (tickets, bloodscrip) | pass type HAVE (`gameobj-data.tsv:102`) | PARTIAL | LOW |
| E5 duskruin_archaeologist.lic, duskruin_adventurer.lic (Taleph, v1.2.1), makemoons.lic, duskruin_find_rat.lic | Duskruin side games | keep-lists for the archaeologist and adventurer exchanges; sewer "moon" pieces (11 data) | none | GAP | LOW |
| E6 totbundle.lic:19-40 | Ebon Gate trick-or-treat | `wrapped piece of candy`, `turn my bag` -> `^You turn the.*labeled`, `There are (\d+) treats inside` | soul shards and `ebongate` type HAVE; candy none | GAP | LOW |
| E7 rafflewindow.lic:321-351 (Phocosoen), raffle.lic:59-90, draffle.lic:41-56, xraffle.lic (LostRanger, 2019), citrove.lic:11-18 | raffles, all events | `RAFFLE LIST`: `Raffle #N for "X", Cost: N silver`, `Location: X at [Y]`, `Some raffle tickets for raffle #N can be found on X at [Y] (N)`, `The tickets sold for N silver each`, `Draws at: X (in Y)`, `Drew at:`, `It drew N winners`; ticket: `The drawing will be in N minutes` / `N hours and N minutes`, `The drawing has been held with the following winner`, `The raffle is for X`; token drawer (`citrove`) | `grep -rn "drawing will be\|Raffle #" crates` = 0 | GAP | LOW (a natural highlight or timer at M8; no behavior) |
| E8 ci_games.lic:101-237 (Alastir, 2018), gondola.lic | Caligos Isle | `You find N new seashells`; prize-name lists (5 tiers, `:105-126`); game failure lines (`manage to get nothing more than a handful of slime`) | none | GAP | LOW |
| E9 joustsmart.lic:155-158 (Tovklar, v0.4) | jousting | `A jousting attendant says ... AIM LEFT, AIM CENTER, or AIM RIGHT`; `appears to be aiming.+?to the (right\|left\|center) of your`; `A jousting herald announces.+?(0\|1\|2)`; four knight patterns | none | GAP | LOW |
| E10 inquiconundrum.lic (Kyrandos, v1.28.0, 3375 lines) | Inquisitor, Zephyra Wilds Conundrum ("repair the clockwerke orangutan") | a timed minigame solver keyed on rooms `Mistwarden Temple`/`Zephyra Wilds` | aevit balance HAVE | GAP (minigame) | LOW |
| E11 ubw.lic:69-194 | Undergrowth of Bittermere Woods, the witch's tasks | `...an old witch says, "Yourrr expelled enerrrgies are apprrreciated.  Take this as thanks."  She tosses a X`; break/stow items per task | none | GAP | LOW |
| E12 generate-gemstone.lic:21-27 (Dreaven, v11) | **GemStone jewel properties** | 25 common, 14 regional, 15 rare, 16 legendary property names (Python count over the four arrays); limit-break skills and stats; 9 damage types | `grep -rn "Arcane Intensity\|Arcanist's Blade" crates` = 0; not in Lich core either (`grep -rn "Arcane Intensity" reference/lich-5/lib` = 0) | GAP | MEDIUM (loot: jewels are found while hunting; the ledger records finds, `plan/34`) |
| E13 birdwatching.lic:224-443, bird_fieldguide.lic | Jeepers Creepers bird watching | `peer my spyglass` -> `you notice an? X in this area` / `...X feather hiding in the environment`; per-uid sightings DB | none | GAP | LOW |
| E14 honoramongthieves.lic, bestbeast.lic, grofl-auto.lic, quest_aicrystal.lic, locksmehrquest.lic, spriteWL.lic, startvaalorsprite.lic, StartVaalor.lic, thrak.lic, feeder.lic | player-run games, spirit beasts, Rings of Lumnis, one-off quests, newbie start quests, Raging Thrak quiz | spirit beast grades `extraordinary\|perfect\|robust\|average\|unimpressive specimen`; Vaalor errands `Now, take this to Guardsman X`; sprite quest lines; quiz answers | none | GAP | LOW |

### Guilds (special ask 5)

The pile's guild scripts are **rogue and sorcerer guild** skill training. The one fact they all
share is the **`gld` report**, which Cena does not read at all.

| script:line | fact | sample text or regex | Cena (searches) | verdict | value |
|---|---|---|---|---|---|
| G1 grguild.lic:2421-2466 (Kalros/Gibreficul, v0.4, 2017), rogue-gambits.lic:64-86 (Daedeus) | the `gld` report | `You have N ranks in the <Skill> skill`; `You have.*?the <Skill> skill`; `The Training Administrator told you to <task>.`; `You have N repetitions` (left); `You have earned enough` (training points: promotion due); `You are a Master of <Skill>.`; `not currently training`; `have not (yet) been assigned a current task`; `inactive` (lapsed member); `month` (check-in due); `you nor any training partners` (no credit); `At least N more should have` (wedges); terminator `for additional commands.` | `grep -rn "Training Administrator\|repetition" crates --include=*.rs` = 0; `menu_commands.tsv:214,502-505,663` hold `gld` menu verbs only | GAP | MEDIUM (character state: guild ranks; no milestone) |
| G2 grguild.lic:2566-2695 | rogue guild task catalogue | ~30 task phrases, one per skill step: `practice defending against footstomps`, `work out on the sweep dummies`, `pick some tough boxes from creatures`, `measure some boxes, then pick 'em`, `clean the windows in the guild`, `water the guild plants`, `let a footpad shoot arrows at you`, `play a few rounds of slap hands`, six `practice ... while stunned` tasks, `crush up some garlic`, `ding up a few melons at the subdue mannequins`, `put clasps on some containers`, `make some good locks`, `cut keys for some locks you make`, `customize some lockpicks and keys`, `sweep the guild courtyard`, `get lessons in Footstomp from a master footpad` | none | GAP | LOW |
| G3 grguild.lic:361-1648, rogue-lmas-measure.lic:15-18 | Lock Mastery reps | `LMASTER SENSE`, `LMASTER MEASURE box, then speak the box's difficulty aloud`, `LMASTER CALIBRATE the calipers`; `Measuring carefully, ...`; `You have N repetitions remaining.` | none (the locksmith pool, `town/pool.rs`, is a different thing) | GAP | LOW |
| G4 checkin.lic:28-40 (Xanlin) | **profession guild dues** | 11 professions: initiation fee and monthly dues (Bard 2500/750, Rogue 15000/5000, Warrior 10500/3500, Sorcerer and Wizard 4500/1500, Ranger 0/1000, Empath 0/500, others 0/750); `ask <guildmaster> about checkin` x3 pays 3 months | none | GAP | LOW |
| G5 illusions.lic:103-698 (Kaldonis, v2.20), illisorcguild.lic | Sorcerer guild: Illusions (rose, vortex, maelstrom, void, shadow, demon; tasks learn, audience, speed, teach); guild entry by pillar sigils | illusion failure lines (`the mana seeps away`, `disperses into inky black lines`); completion `training task.]` / `You have completed your training task`; sigil -> spell: blood-drop 701, radiating circle 702, dark cloud 703, hazy black 704, slate grey 705, dark eye 717 | entry HAVE: `travel/routines/three_pillars.rs:25-30`, same six sigils; illusions none | HAVE (entry) / GAP (training) | LOW |
| G6 grguild.lic (rogue door), rogue guild password | guild door | `lean door` + the user's verb sequence | HAVE: `travel/routines/guild_password.rs` | HAVE | - |

## Script by script

Depth: **D** read deeply (header, data blocks, every capture region); **C** header plus capture
lines (`digest.py`, `uniq_caps.py`). Families were diffed rather than re-read: sbounty forks against
the baseline `reference/scripts/scripts/sbounty.lic`, go2 forks against baseline `go2.lic`, `ego2`
against `escortgo2.lic`, the HUDs against each other (`sim.py`: shared-line ratio).

| script | lines | purpose | captures / data / uses | verdict summary |
|---|---|---|---|---|
| grguild.lic (D) | 3024 | rogue guild trainer (Kalros/Gibreficul, v0.4, 2017) | `gld` report hook (`:2421-2466`), ~30 task phrases, Lock Mastery / subdue / stun / cheapshot / sweep rep lines, guild NPCs `Master Footpad`, `Training Administrator` | GAP G1-G3 (MEDIUM/LOW); door HAVE |
| osacrew.lic (D) | 4428 | OSA crewman automation (Peggyanne, v6.1.0, 2026-09-20) | sails, capstan, cannons, damage per deck, Sea Hag tasks, sea-combat announces, LNet crew protocol, gangplank edge patches | GAP, LOW (OSA) |
| osacommander.lic (D) | 4649 | OSA captain automation (v4.1.0) | as osacrew, plus muster, boarding, `listen` -> `[Enemies Left: N]`, box and room looting on a taken ship | GAP LOW; loot lines mostly HAVE |
| ego2.lic (D) | 1853 | escort a bounty client (Tillmen, v0.6, 2020) | escort pickup/dropoff tables, ambush and trap lines, follow dialogue, 96 per-edge escort crossings; ancestor of baseline `escortgo2` (0.66 shared lines) | GAP B16, B18, data rows (HIGH, M8) |
| osacrew_avalon_wizard.lic (C) | 4017 | osacrew v5.8.6 for Avalon/Wizard FE | same captures as osacrew | variant; GAP LOW |
| bountyhunter.lic (D) | 2562 | sbounty fork (Alastir) | sbounty's `bounty_patterns` (identical to baseline but two NPC names), NPC hand-offs, removal/expedite, bandit area -> rooms, herbalist rooms, heirloom `look` for initials | B2-B4, B7, B11-B12, data rows |
| volnstep.lic (C) | 1295 | Voln step tasks (Jennora, v2.1 BETA) | ~40 `society_task` phrases, step rooms, Landing and Vaalor | GAP LOW (society tasks) |
| nesbounty.lic (C) | 2062 | sbounty fork using NesAtlas (Nesmeor, v1.2.2) | patterns identical to baseline sbounty (whitespace diff only) | variant; same as bountyhunter |
| sbounty-2017.lic (C) | 1835 | sbounty fork (Daedeus) | heirloom patterns rewritten for "game now shows heirloom name" (`:26`, task_search/task_heirloom) | variant; Cena's heirloom sample tests pass |
| go2enhanced.lic (D) | 1324 | go2 fork (Ponclast, v1.07) | 0.85 shared with ride2; adds stop-for-dead/stunned players, `--avoid-rooms`, justice check, `Lost in the Fog` | T13, T14 |
| sbountybeta.lic (D) | 1568 | sbounty rewrite (SpiffyJr, 2018) | 26 real task samples in comments, heirloom search line, removal and taskmaster answers, forage results | B2, B4, B8; samples used for the Cena test |
| ride2.lic (C) | 1233 | go2 (Tillmen, v1.25) | 0.70 shared with mapdb go2; no new move text | N/A (superseded by go2) |
| saferepo.lic (C) | 2320 | repository.lic filtered by LNet ignore list | tooling | N/A |
| rumor_woods.lic (D) | 3024 | Rumor Woods fox/pixie hunt (Alastir, v2.17.18, 2026-04-16) | frozen maze graphs, trot lines, sign lines, raikhen payouts | E1 GAP LOW; maze in mapdb |
| osa-commands.lic (C) | 1460 | OSA helpers (Jymamon, 2024.08.01) | multistep ship commands, ship-state lines | GAP LOW |
| gemologist.lic (C) | 660 | fill gem bounties from locker jars (Alastir, v1.0) | locker rummage `You rummage through the (deep chest\|magical item bin\|clothing wardrobe\|weapon rack\|armor stand\|locker) and remove`, jar shake/empty lines, expedite answers, `a minute` | B1, B3; locker/jar lines GAP (`grep -rn "rummage through\|hard shake" crates` = 0) MEDIUM M8 |
| mapmap.lic (C) | 954 | mapdb builder (Tillmen, v0.3) | room capture for mapping | N/A (tooling) |
| foxpixiehunt.lic (C) | 840 | older Rumor Woods hunt (Arianiss, v2.6) | sign lines, `Aethyra has an exchange`, `escorts you through\|You need to redeem` | E1 |
| inquiconundrum.lic (C) | 3375 | Zephyra Wilds conundrum (Kyrandos, v1.28.0) | minigame | E10 LOW |
| newenemy.lic (C) | 345 | enemy-ship room descriptions for the mapdb | 114 description strings for rooms 30266-30271 | OSA, PARTIAL LOW |
| startvaalorsprite.lic (C) | 395 | Ta'Vaalor newbie quest + sprite quest | quest dialogue | E14 LOW |
| bandit_hunter_attack.lic (C) | 407 | bandit group attack (Alastir) | kill-count lines, `shield throw`, KF bandit task texts | B9 context; HAVE for task text |
| ms.lic (C) | 629 | bounty/vitals HUD (JFTActual) | same task regexes as uberbounty (0.61 shared) | HAVE (task); B6 sample |
| t.lic (C) | 612 | copy of ms.lic (0.85 shared) | same | variant |
| honoramongthieves.lic (C) | 629 | a player-run dice/roshambo game host ("HAT") | dice lines `The dice bounce a few times and come to rest on N and N.` | N/A (player game) |
| minibar.lic (C) | 540 | vitals + bounty bar for Stormfront (Nesmeor, v0.1.2) | 10 bounty samples in comments, task regexes | HAVE; B6 sample |
| ubw.lic (C) | 462 | Bittermere witch tasks | witch reward line, task items | E11 LOW |
| rafflewindow.lic (C) | 1036 | raffle window (Phocosoen) | `RAFFLE LIST` format | E7 LOW |
| sail2.lic (C) | 1235 | OSA go2 (Peggyanne, v1.0.4, 2026-05-17) | downloads `jmbreitfeld/OSA-Ocean-Database`; 12 ports | OSA GAP LOW |
| swapbounty2.lic (D) | 290 | reroll bounty (Lunarwulf, v3.4) | KF taskmaster Halfwhistle room 29866, guard names, `It is your duty to oppose the` | B21, B24 |
| StartVaalor.lic (C) | 283 | Vaalor newbie errands to level 5 | guard name -> room (41 data), `Now, take this to Guardsman X` | data row LOW |
| swapbounty.lic (C) | 272 | older swapbounty (0.90 shared with v2) | same | variant |
| ci_games.lic (C) | 267 | Caligos Isle games (Alastir, 2018) | seashell lines, prize lists | E8 LOW |
| col.lic (C) | 282 | Council of Light tasks (spiffyjr, v1.0) | Q&A, items -> rooms, `Report to the High Taskmaster` | data row LOW; High Taskmaster lines HAVE |
| jfloo.lic (C) | 338 | Chronomage day-pass hopper (Jara) | pass raise answers, bank, `your account is free` | T1 |
| ocean-go2.lic (D) | 37482 | OSA ocean go2 (Dreaven, v10) | 1,283-room ocean table keyed by ASCII map, bugged-exit notes, navigation lines | OSA GAP LOW |
| illusions.lic (C) | 998 | Sorcerer guild Illusions (Kaldonis, v2.20) | illusion failure and completion lines | G5 LOW |
| stolenvalor.lic (C) | 1466 | OSANav hard-coded Cauldron routes | ship nav lines | OSA LOW |
| azbandit.lic (C) | 454 | group bandit sweep (Azanoth, v0.1) | bandit noun set (10 nouns), ambush openers, group lines | B17; bandit type HAVE (`gameobj-data.tsv:11`) |
| bandit_hunter_follower_support.lic (C) | 246 | follower side of bandit hunter (Alastir) | `a minute`, `completed this portion`, sign-up lines | B1, B9, B10 |
| minibounty.lic (D) | 322 | minimal bounty HUD (Demandred, v1.2) | full task regex set with named groups incl. `initials`, and `in order to help X take care of a bandit` | B5, B7 |
| gs3autoroller.lic (C) | 759 | character-creation stat roller (LostRanger, 2020) | best/worst stat arrays, roll ranges | N/A (creation screen) |
| nesmapper.lic (C) | 583 | room-data collector (Nesmeor, v1.1.0) | ranger/rogue `sense`: 12 climates (`arid`, `cold, damp`, ... `temperate`, `find no new insight` = indoors) and terrains (`barren scrub`, `coniferous`, `hard, flat` ...) | the map carries `climate`/`terrain` (`cena-map/src/room.rs:85,88`); a `sense` reader none. GAP LOW |
| uberbounty.lic (C) | 328 | bounty HUD with buttons (Hazado, v6.93) | task regexes; `openDialog` markup | HAVE (task); B6 sample |
| warmonger.lic (C) | 355 | Sunfist warcamp search over 25 spots (Mekimin) | `sigil of location` -> `You sense some faint` | GAP LOW (society) |
| gangplank.lic (C) | 371 | map the OSA gangplank to the pier room | port arrival lines `drifts steadily toward the (diverse\|bustling\|lively\|ash-covered...) port`, `The dock handler points to the` | OSA GAP LOW |
| eforage.lic (C) | 259 | forage helper (Ondreian, alpha) | `You peer (dir) and see`, herb colour lists, `The sense of peace and security passes away` | forage outcome lines are ebounty baseline; LOW |
| wander.lic (C) | 286 | least-recently-visited wander (Tillmen, v0.6) | targeting lines | N/A (plan/21 `WanderUntil` covers the idea) |
| houselocker.lic (C) | 213 | house locker walker (Oweodry, 2014) | 32 data: CHE house names, locker rooms by town (Solhaven only) | GAP LOW |
| birdwatching.lic (C) | 762 | bird field guide (ChatGPT-assisted) | spyglass lines, per-uid sightings | E13 LOW |
| bandit_hunter_support.lic (C) | 139 | bandit hunter support (Alastir) | `stance verbose` answers; the metal-jaws trap aimed at a named leader | B16 |
| locker2.lic (C) | 109 | go to locker (Hazado) | Twilight Hall / Crystal Lounge locker rooms, `You'll have to wait` | LOW (lockers are another pile's) |
| rogue-gambits.lic (C) | 277 | Rogue Gambits trainer (Daedeus) | `gld` lines: ranks, `The Training Administrator told you`, `repetition`; trick names | G1 |
| wiz_to_lich.lic (C) | 199 | Wizard/SF script converter (Tillmen) | tooling | N/A |
| dm_runner.lic (C) | 254 | Delirium Manor searcher (Alastir, 2018) | pass entries line, sundries finds | E4 |
| fly.lic (C) | 311 | premium teleport + Chronomage orbs (Ryjex) | `premium start` parse, departure timer, capsule chamber | T5, T6 |
| stingpotions.lic (C) | 227 | death recovery potions (zegres, fix of Xanlin) | bank refusal `that much in the account\|debt collect`, potion drink lines | LOW (healing is another pile's) |
| sailors_grief_swim_fix.lic (C) | 273 | mapdb hotpatch, Sailor's Grief swims (Tysong, v1.0.0) | `somewhere to the X of`, `is just a short swim away` | T11 |
| dm2.lic (C) | 195 | Delirium Manor variant | as dm_runner | E4 |
| nextbounty.lic (C) | 204 | swapbounty variant (Genstealer, v1.0) | `It is your duty to oppose the`, bank `^The teller carefully` | B21 |
| speed2.lic (C) | 193 | day-pass travel (SpiffyJr, 2019) | pass text parse, `A roundtrip day pass\|A one-way ticket to`, `quickly hands you`, raise answers | T1; `quickly hands you` HAVE (`day_pass.rs:448`) |
| star-wealth.lic (C) | 407 | wealth summary | `unspent bounty points`, `Total Pooled Points remaining`, event entries, `resource` totals | B15, E2 |
| temp_jfloo.lic (C) | 200 | jfloo v2.9 | `ASK me again`, `read my day pass` | T1 variant |
| tour.lic (C) | 404 | virtual room tour (LostRanger) | tooling | N/A |
| metamap.lic (C) | 695 | map metadata editor (LostRanger) | `meta:mapname:` tags | N/A (map tooling) |
| plat_updater.lic (C) | 235 | Platinum map updater | tooling | N/A |
| sos.lic (C) | 225 | periapt portal from room 25210 to the privy, Altar of the Undying, tile room | `touch tile` -> `a swirling viridian portal`; periapt charge `devoid of any power\|greenish light` (empty) / `steady viridescent light\|vivid green iridescence\|tiny viridian star` (charged), `rub periapt` | GAP LOW (a portal item's states) |
| undead_bounty.lic (D) | 703 | reroll until undead (Fizzleworth + Claude, v1.6) | typed bounty request, boosts, guard asleep, turn-in lines, refusals | B4, B13, B14 |
| autovote.lic (C) | 145 | topmudsites voter | web | N/A |
| mapnav.lic (C) | 380 | manual mapper (LostRanger) | tooling | N/A |
| hunt_begin.lic (C) | 161 | hunt starter (Alastir) | `The herbalist's assistant in Ta'Illistim`, statue teleport | HAVE (herb task wording, `bounty.rs:396`) |
| IFW_teleporter.lic (C) | 130 | Mist Harbor trinket (Gibreficul) | trinket states | T7 |
| mapdiff.lic (C) | 352 | compare two mapdb files (Xanlin) | tooling | N/A |
| spriteWL.lic (C) | 97 | Landing sprite quest | quest dialogue | E14 LOW |
| fixdb.lic (C) | 72 | mapdb editor | tooling | N/A |
| event_support.lic (C) | 108 | Duskruin prize sorting | noun lists by disposition, room 25577, exchange lines | E3 |
| bird_fieldguide.lic (C) | 556 | bird field guide | as birdwatching | E13 |
| sanctum.lic (C) | 193 | Sanctum of Scales helper | `faint thudding and notice a disturbance against one of the` | LOW |
| ship.lic (C) | 123 | Landing <-> Teras by the Glaesen Star (Ryjex) | ticket name, `mooring lines are drawn taut`, `Upper Deck` | mapdb carries the ship crossing (gangplank 136 occurrences); LOW |
| GoOutOfMelgorehn.lic (C) | 69 | fixed walk out of Melgorehn | moves only | N/A |
| chrono.lic (D) | 264 | Chronomage travel (Alastir, Ryjex) | stations, pass texts, `location` answer, raise answers | T1-T4, data row |
| duskruin_loot.lic (C) | 166 | open arena winnings | winnings package lines | E3 |
| gunnersmate.lic (C) | 512 | OSA cannons | `Thirty second warning`, enemy manoeuvre lines | OSA LOW |
| go2bandits.lic (C) | 151 | go near your bandits (Xanlin, v6) | bad-location rooms; **the group-assignment pattern** | B5, data row |
| iron_tycoon.lic (C) | 290 | make and sell iron (Landing) | charcoal/ore/pit lines | LOW (crafting) |
| rrbountyspool.lic (C) | 256 | River's Rest bounty spooler (Kaldonis, v0.40) | boosts, `You want to be removed`, `kind of busy`, `All done with that assignment`, RR NPCs `Lomara`, `Kelph` | B1, B2, B4, B13, B14 |
| Bountywhitelist.lic (C) | 318 | ebounty whitelist (Renian, v0.1.1) | settings only | N/A |
| instrumentrest2.lic (C) | 231 | bard instrument swap while resting | container lines | N/A |
| parsebounty.lic (D) | 74 | format the `bounty` report | report header, points, vouchers, **eligibility timer**, per-kind summaries | B15, B20 |
| MapEnemyShip.lic (C) | 303 | enemy-ship map entries | 22 room records | OSA LOW |
| decryption.lic (C) | 63 | quest cipher lexicon | ~90 words | data row LOW |
| xraffle.lic (C) | 777 | raffle tracker (LostRanger) | raffle lines | E7 |
| bandit_hunter_follower.lic (C) | 145 | follower (Alastir) | guard hand-off `Killing N of them should do nicely` | B22 |
| bestbeast.lic (C) | 166 | spirit beast finder (Ziled) | specimen grades, `recent brush with the spirit world` | E14 LOW |
| rescue-watch.lic (D) | 75 | start `rescue` on a death (Akono, 2016) | town death announcements | B19 |
| checkin.lic (D) | 57 | profession guild dues (Xanlin) | dues table | G4 |
| hw_hunter_follower_support.lic (C) | 111 | Hinterwilds follower (Alastir) | Cold River guard `Rawknuckle`, `points at the adventurer Halfwhistle` | B10 |
| iron.lic (C) | 293 | make iron slabs (Hazado) | burner lines | LOW |
| teleport.lic (C) | 378 | n-setting teleporter map patches (Ondreian, v1.2.0) | per-town jump points | T16 |
| flowergirl.lic (C) | 172 | weapon bless via the flower girl (Kaldonis, v0.5) | bank `flips through the books`, priest dialogue | LOW |
| joustsmart.lic (C) | 191 | jousting tactics (Tovklar, v0.4) | attendant/herald lines, 4 knight patterns | E9 |
| osatask.lic (C) | 264 | OSA crow's-nest task bird | Sea Hag's Roost task lines, `The X flies in` | OSA LOW |
| thrak.lic (C) | 45 | Raging Thrak quiz answers | fixed answers | E14 LOW |
| findwarcamp.lic (C) | 182 | find a warcamp by wandering (LostRanger) | settings | LOW |
| quest_aicrystal.lic (C) | 92 | crystal quest | beam colour lines `A red beam shoots up from your` ... indigo | E14 LOW |
| 116locate.lic (C) | 87 | Locate Person to a room number | 116 answers + `roomName` | T17 |
| bandit_hunter_grouping_support.lic (C) | 179 | KF bandit group (Alastir) | the four Kraken's Fall bandit task texts | HAVE (Python test: Bandit) |
| menuid.lic (C) | 108 | report a menu item's id (LostRanger) | `<c>_menu #N` | N/A (Cena has `menu.rs`) |
| lazymove.lic (C) | 93 | go/climb on non-cardinal paths (nishima, v0.9) | `You can't climb that.`, `You're going to have to climb that.` | HAVE (`movement.rs:131,134`, `NeedsClimb`/`NeedsGo`) |
| locksmehrquest.lic (C) | 102 | Locksmehr Larceny quest (Drafix) | search lines | E14 LOW |
| makemoons.lic (C) | 92 | Duskruin sewer moons | 11 piece names | E5 LOW |
| node.lic (C) | 205 | super-node list by town (Senato) | room names and ids | data row PARTIAL LOW |
| osa-map-plugin.lic (D) | 9299 | OSA ocean map + current-room tracker (Jymamon, 2025.03.01) | 1,252 ocean rooms; nav lines | OSA GAP LOW |
| raffle.lic (C) | 159 | raffle ticket timer (v1.99) | drawing lines | E7 |
| tlogin.lic (C) | 333 | login to the test instance (LostRanger) | instance names, account types `NORMAL\|PREMIUM\|TRIAL\|INTERNAL\|FREE` | N/A (login is Cena's own, `plan/10`) |
| feeder.lic (C) | 128 | feeder announcements to Discord (elanthia-online) | `QUEST ACCEPT`, `received a quest`, `Your passcode is to:` | LOW |
| fox2020.lic (C) | 118 | fox hunt (Obelin) | sign and trot lines | E1 |
| peek.lic (C) | 96 | room preview (Xanlin) | tooling | N/A |
| warcampfind.lic (C) | 159 | warcamp search | `shimmering patch of air` | LOW |
| fox.lic (C) | 104 | fox hunt (Ryjex) | as fox2020 | E1 |
| overloaded.lic (C) | 337 | encumbrance guard for bigshot (Vailan) | reads Cena-covered encumbrance | N/A (HAVE `character.rs`) |
| piratehunter.lic (C) | 108 | OSA pirate engage | enemy approach lines | OSA LOW |
| turnin.lic (C) | 50 | herb bounty turn-in | concoction regex | HAVE (task) |
| answers.lic (C) | 70 | jail answers (Tillmen) | `the official walks over to you and sets you free` | GAP LOW (justice) |
| group2.lic (C) | 126 | group helper (Oweodry, 2012) | `group status is currently`, pull prone members | PARTIAL: membership HAVE (`state/group.rs:168-250`); `Your group status is currently (open\|closed).` absent there (`grep -ni "status" group.rs` = one doc hit) though Lich core has it (`reference/lich-5/lib/gemstone/group.rb:525`). Also azbandit.lic:152 `X is following you` / `the leader of your group` / `also a member of your group`. LOW-MEDIUM (group bounties) |
| lichid_to_uid.lic (C) | 57 | Lich id -> uid (Xanlin) | tooling | N/A (Cena keeps both ids, `plan/21` §3d) |
| rnum3.lic (C) | 108 | room number in title (Drafix) | `roomName`, `roomDesc`, familiar stream | N/A |
| banditcounter.lic (C) | 64 | group kill counts (Ryjex) | `suppress\|succeeded in your task`; ignores `Warcamp\|Duskruin Arena\|Raging Thrak` | HAVE (task) |
| foxy.lic (C) | 100 | fox hunt (Selema) | as fox | E1 |
| iamannpc.lic (C) | 146 | random emotes | none | N/A |
| lockdown.lic (C) | 95 | locker manifest to CSV (m444w) | `Thinking back, you recall the contents`, `Obvious items:`, `There are no items in this locker` | GAP LOW (lockers) |
| smart.lic (C) | 46 | `2.leaf` -> `second leaf` | ordinals | N/A (`resolve.rs`) |
| writemail.lic (C) | 121 | mail composer | none | N/A |
| bandit.lic (C) | 63 | walk bandit territory (Hazado) | bandit task regex | HAVE |
| bandit_grouping_support.lic (C) | 145 | KF bandit group | KF task texts | HAVE |
| graveyard_fix.lic (C) | 51 | always PUSH the bronze gate | gate lines | HAVE (`routines/bronze_gate.rs`) |
| tentwatch.lic (C) | 509 | tent protection (Fulmen, v2.0) | `You quickly unfold the ... tent`, `You methodically unhook ... tent` | LOW |
| turnkey.lic (C) | 96 | keyring cycling | personal item names | N/A |
| 213.lic (C) | 80 | keep Minor Sanctuary up (Tsalinx) | `A sense of peace and calm`, `already peaceful and calm`, `spirit of aggression`, `chaotic nature` | LOW (spells are another pile's) |
| broochboost.lic (C) | 90 | brooch / long-term boost | `You have deducted`, `You do not have any Long-Term Experience Boosts to redeem` | LOW (ledger has the award line, `ledger/hunt.rs:50`) |
| duskruin_archaeologist.lic (C) | 66 | Duskruin archaeologist exchange (Taleph) | keep-list, `Just give me it again` | E5 |
| grofl-auto.lic (C) | 102 | Rings of Lumnis brooch (Azanoth) | card lines | E14 |
| tablewatch.lic (C) | 353 | table protection (Fulmen) | table transform lines | LOW |
| tagsearch.lic (C) | 82 | map tag search | tooling | N/A |
| generate-gemstone.lic (D) | 433 | random GemStone jewel (Dreaven, v11) | 70 property names by rarity | E12 MEDIUM |
| gondola.lic (C) | 31 | Caligos gondola runes | colour -> direction | T15 |
| paths.lic (C) | 66 | add hidden paths to room text (Essio) | tooling | N/A |
| quest_sell.lic (C) | 51 | sell quest items | pawnbroker lines | HAVE mostly (`town/reply.rs`, other pile) |
| step2bandits.lic (C) | 102 | step toward bandits (Xanlin) | moves | N/A |
| totbundle.lic (C) | 53 | Ebon Gate candy bundling | treat count | E6 |
| kfbandit.lic (C) | 52 | KF bandit profiles (v2.2) | KF task texts, `The taskmaster told you` | HAVE |
| roomexits.lic (C) | 48 | show exits on room display (Tysong) | `roomName`, `Obvious (paths\|exits)` | N/A (Cena room model) |
| utils.lic (C) | 201 | Nesmeor helpers | `A good positive attitude never hurts.` (smile check) | N/A |
| Temple_Drop.lic (C) | 63 | temple drop items (Taleph) | `your personal growth`, `you don't see anything that would be suitable` | LOW |
| boatswain.lic (C) | 254 | OSA ship repair (Licel) | RT lines | OSA LOW |
| boh.lic (C) | 80 | Bag of Holding fetch (Ryjex) | `whisper my sack find X` -> `You successfully rummage through\|You did not find any pockets containing an item` | LOW |
| bounty-bitch.lic (C) | 108 | group bounty board (nishima, v0.4) | `^You have been tasked to (rescue\|suppress\|recover\|hunt)` | HAVE |
| osaunderway.lic (C) | 180 | OSA underway | sail/anchor lines | OSA LOW |
| petalbless.lic (C) | 60 | priest bless | priest dialogue | LOW |
| roomorder.lic (C) | 36 | position in a queue | `You gaze around the (area\|room) and note that X is directly in front of you` | LOW |
| trigger.lic (C) | 203 | text -> command trigger (Upy) | user patterns | N/A (M8 triggers are customization) |
| bacon.lic (C) | 38 | am I in my bounty's area? (LostRanger) | `location` answer, `(.*) (near\|between)` | T4 |
| fatty.lic (C) | 81 | Hinterwilds gigas fragments / encumbrance (Machtig, v1.1) | `You are carrying N gigas artifact fragments.`; `not encumbered enough to notice` | HAVE (`currency.rs` gigas fragments) |
| gswiki.lic (C) | 199 | browse gswiki | web | N/A |
| krakii.lic (C) | 216 | browse Krakiipedia | web | N/A |
| osaanchor.lic (C) | 91 | raise anchor | capstan lines | OSA LOW |
| osasails.lic (C) | 195 | raise sails | sail lines | OSA LOW |
| biggus.lic (C) | 81 | bigshot config backup | files | N/A |
| chub.lic (C) | 73 | copy of fatty.lic | same | variant |
| citrove.lic (C) | 25 | raffle token drawer | token lines | E7 |
| draffle.lic (C) | 102 | raffle timer (JustDan) | drawing lines | E7 |
| rogue-lmas-measure.lic (C) | 21 | LMAS MEASURE rep | `Measuring carefully,`, `You have N repetitions remaining.` | G3 |
| star-watch.lic (C) | 72 | annotate alts in `Also here:` | room players | N/A |
| use_urchins.lic (C) | 82 | toggle go2 urchin guides | settings | N/A (`plan/21` §4.1 urchins) |

## Tail

190 scripts with capture + data < 5, scanned by name; the ones whose name suggested game data were
opened (first lines) and are marked with what they hold.

- **Bounty, small:** `bounty.lic` (uses Lich `Bounty`; `ask <npc> for add <person>`, a group add),
  `gbount`, `prettybounty` (B15), `bounty_instant`, `bountypopup` and `bountytimer` (both read
  `Spell[9003]` Next Bounty, which Cena has as a cooldown row, `spells.tsv:428`), `yakbounty`,
  `ggmybounty`, `nesbounty-bigshot`, `sbounty-shunt`, `bountyhunter-explorer`, `kffinish`, `xready`
  (blessed weapon by bounty creature), `convert-guild-boost` (`boost guild bounty`, B14),
  `SaveBountyGems` (a keep-list of gem names), `reimbounty` (`reim camp confirm` -> room 24842),
  `hwbounty` / `kftask` (Kraken's Fall bounty rooms 29866 Halfwhistle, 29877, 28927 Lucrecious,
  28978), `shat_bounty_parser` (B23), `gbtest` / `bbtest` (group bounty sharing, Ta'Illistim only).
- **Escort and rescue:** `officialescort`, `rescue-sunfist` (Sunfist official to an outpost),
  `rescue-me`, `childtower`, `bigshot-child`, `child-bigshot` (all key on `You have made contact with
  the child` and a `child` NPC; HAVE for the text, `bounty.rs:361`).
- **Map patches and movement:** `hinterwildriverfix` (30115 -> 29861 `south` until moved) and
  `reset_vaalor_timeto` (5907->3516 cost 0.2): **both already in the mapdb** (Python over the JSON:
  30115's wayto 29861 is that proc; 5907's timeto 3516 is 0.2), `sanctum-mapdb-patch`,
  `shattered_portals` (`$go2_use_portals` for Shattered), `nelemarfix`, `fastswim`, `enlake`
  (Illistim <-> Vaalor over the frozen lake; the mapdb's ice-mode edges, `plan/21` §1), `avalanche`,
  `crevice`, `fuck_you_water_tunnels`, `whirlpoolwatch` (stand, `You can't swim in that direction`,
  HAVE in `movement.rs:103`), `atoll`, `vaalorwater2`, `rifted` (Rift uid ranges: scatter
  4571001-4571030, planes 4566001-4570013, heart 4570014), `monastery` (a familiar puzzle), `loca`,
  `errand`, `warp`, `express`, `comp2`, `drag`, `jhorse`; go2 wrappers `famgo2`, `go2fam`,
  `go2table`, `tago2`, `pego2`, `g2n`, `go2name`, `go2name-2`, `go2locate`, `step2`, `path`,
  `gotoandback`, `map-search`, `mapcopy`, `mapresize`, `unfubarmap`, `cartographer`, `trashtag`,
  `uid`, `slocation`, `whereami`, `lost`, `saveloc`, `touchme`, `rnum`, `debugroomid`, `room-info`,
  `roomwindow_mod_nodesc`; follow and group movers `follow`, `follow2`, `auto_follow`, `stalk`,
  `deadstop`, `queen`; rest spots `kfrest`, `zulrest`, `terasrest`, `imtrest`.
- **OSA satellites:** `osaorders`, `osamap` (`<nav rm=>` for ship rooms), `osapicker`, `osanav`,
  `osacannon_watch`, `osasail_watch`, `loadcannons`, `cannons`, `readyship`,
  `nautical-charts-update` (downloads `jmbreitfeld/OSAMaps` et al.).
- **Events and quests:** `duskruin_adventurer`, `duskruin_find_rat`, `drleaderboard` (Duskruin
  endless leaderboard: `Looking at the <prof> leaderboard, you see X currently holds the top position
  at round N`), `conundrum` (the older Zephyra Wilds solver), `spitfirequest2015`, `spit2015plat`,
  `ezquest`, `brayquest`, `quest_locker`, `dreamfire-pocket-guide` (an item's verb list), `xday`,
  `hat_queue`, `birdwatchingwindow`, `star-cod`, `webgoals`, `wishlist`, `attendance`, `news`
  (TownCrier RSS).
- **Guild and society:** `illisorcguild` (G5, HAVE via `three_pillars.rs`), `warservice` (warrior
  critical padding service), `sunfist_world_tour`, `campy`, `glampy`, `tend`, `alch_buy`,
  `seawater`, `buyarrows2`.
- **Roleplay, chat, utilities (N/A):** `come`, `then`, `waitanddo`, `load`, `update`, `ifttt`,
  `twitternfl`, `www`, `show`, `swho`, `gaze`, `note`, `getnote`, `deposit`, `withdraw`, `putsafe`,
  `kisskiss`, `noodling`, `zesty`, `RYSKue`, `antivore`, `dreavquit`, `wtfo`, `iza`, `gracemon`,
  `hinterwilds`, `ring`, `swaysout`, `Intel_Ashborne`, `buns`, `grit`, `invokeme`, `spa_analytics`,
  `tickets`, `unsaturated_do`, `hps`, `camptown`, `freelinmur`, `wardento`, `teraspuncture`,
  `lich-set-login`, `converttime`, `realtime`, `star-alt`, `tcalt`, `needhalp`, `needhelp`, `lk`,
  `sancup` (sanctuary wane/settle lines), `backroom`, `dontbreakyourhead`, `obvioushiding`,
  `stand`, `cluster-rc`, `banditdispel`, `rangercompanion`, `findsal`, `autoaccept`, `adventurer`
  (copy of `duskruin_adventurer`), `gemstone`, `supergemstone` (joke shouts), `music`, `sing`,
  `tambourineman`, `rpverbs`, `dlocate`.

## Method

All scratch is in `survey/tools/8-bounty-events-travel/`.

- **Capture lines:** `extract.py` wrote each pile script's capture lines to `caps/<script>.caps`
  (pattern `=~ /`, `waitforre`, `matchtimeout`, `matchwait`, `dothistimeout`, `DownstreamHook`,
  `when /`, `Regexp.new`, `%r{`, `.match(`, `.scan(/`). `digest.py <from> <to>` printed header,
  author/version and the first nine distinct capture literals for each substantive script; that is
  the "C" depth.
- **Bounty text:** `bountyfrag.py` pulled every regex literal with a bounty keyword from the 70 pile
  scripts that mention bounties (543 literals, `bounty_frags.tsv`), counted and sorted by frequency.
  Sample game lines: `grep -Hn -E "^\s*#.*(You have been tasked|is working on a concoction|has received
  orders|You have located|You succeeded in your task|I've got a special mission|You have made
  contact|Hmm, I've got a task)|teststring = " *.lic` over the whole mirror -> 80 lines
  (`samples_raw.txt`). `cena_bounty.py` is a line-for-line transcription of `bounty.rs:208-433`'s
  patterns into Python `re` (same syntax: named groups, leftmost-first alternation) and classifies
  each sample -> `samples_result.tsv` (Bandit 9, Cull 7, Dangerous 2, Escort 1, Gem 6, Guard 3,
  Heirloom 24, HeirloomFound 1, Herb 6, Rescue 4, RescueAssignment 1, RescueSpawned 1, Skin 2, none 13).
- **Families:** `sim.py` = share of a script's distinct lines (>12 chars) found in a baseline
  (`go2`, `sbounty`, `ebounty`, `escortgo2` in `reference/scripts/scripts/`, `go2` in
  `reference/mapdb/`). The sbounty forks' `bounty_patterns` hashes were extracted with `sed` and
  `diff`ed against baseline sbounty (whitespace-only, bar two NPC names and the 2017 heirloom
  rewrite). `uniq_caps.py A B...` lists A's capture literals absent from B (used for the go2 forks,
  the sbounty forks and the travel scripts). ego2 vs escortgo2 edge overrides: `grep -oE
  "^\s*'[0-9]+,[0-9]+'\s+=> proc"` on each, `comm` of the sorted lists (96 vs 87; ego2-only: the
  Locksmehr ferry 1189-1193 and 1139->1140, 1143->1142; the ninth, 2521->2520, is a key ego2 declares twice).
- **Map checks:** Python `json.load` of `reference/mapdb/map-1789942730.json` for room titles,
  `wayto`, `timeto`, `tags` and `uid` (fox-maze uids, chronomage and supernode tags, rooms 1276,
  13779, 30115, 5907, 30266-30272, the ferry and cart edges); `grep -o "<text>" map-*.json | wc -l`
  for proc text (the file has 1,564,870 lines, so `grep -c` alone under-counts).
- **Absence claims:** each GAP names its search. The general form was
  `grep -rn "<fragment>" E:/Cena/crates --include=*.rs --include=*.tsv | wc -l`, run over:
  `Boost`, `Bounty Boost`, `heirloom that you`, `completed this portion`, `Yes, I do have`,
  `This looks perfect`, `kind of busy`, `suitable for you`, `head to another`, `in order to help`,
  `lifetime bounty`, `unspent bounty`, `It's an ambush`, `concealed`, `voice shout`,
  `Time drags on`, `already am`, `no condition to talk`, `successfully completed your task`,
  `Good job`, `duty to oppose`, `magistrate`, `escort`, `ferryman`, `mining cart`,
  `whirlwind of color`, `not valid for departures`, `carefully survey`, `distant departure`,
  `active chamber`, `short swim away`, `calm enough`, `no justice`, `Caligos`, `Training
  Administrator`, `repetition`, `LMASTER`, `Measuring carefully`, `Arcane Intensity`,
  `winnings package`, `signs of a fox`, `Eyaeno`, `treats inside`, `drawing will be`, `Raffle #`,
  `Entries:`, `rummage through`, `hard shake`, `bit the dust`, `put on ice`, `specimen`. Where a
  fragment hit, the hit is cited (e.g. `Boost` -> 20 unrelated boosts; `magistrate` ->
  `gameobj-data.tsv:19,46,47`; `concealed` -> three creature descriptions).
- **Counts:** OSA ocean rooms `grep -c ":location=>" osa-map-plugin.lic` (1,252) and
  `grep -cE "^[0-9]+ => \{" ocean-go2.lic` (1,283); per-sea counts via `grep -o ":location=>'[^']*'" |
  sort | uniq -c`; jewel properties via a Python regex over the four arrays in
  `generate-gemstone.lic`; substantive = capture + data >= 5 in the pile TSV (174; 190 tail).
- Not searched: the log archive (brief). No network. Nothing under `E:\Cena` was edited.
