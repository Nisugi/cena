# Cena crate review — 2026-10-01

**Scope.** All 13 crates, the Ruby bridge's own code (`bridges/ruby/hydra/`), the web assets, CI and the
architecture tests, read in 31 areas of 2,000–10,000 lines each. The working tree at `75095f2a` plus the
uncommitted hunt/travel fix (findings in those files are tagged IN-FLIGHT). No live game, no binary run.

**Where the evidence is.** Each area's full report, with `path:line` citations, the quoted lines, the
scenario and what was checked to refute it, is in `.workflows/findings/crate-review-2026-10-01/<AREA>.md`
(gitignored). This document is the index and the synthesis; it does not repeat the evidence. Findings are
labelled VERIFIED or INFERRED in the area files. The lead re-read the code for BE-A-4, BE-E-3, MO-B-4,
MO-C-3 and BI-D-2 and confirmed each.

**Totals: 13 P1, 90 P2, 105 P3** as the readers rated them; **16 P1, 89 P2, 105 P3** after the author's
answers at the end of this document.

## Baseline

| Check | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace --no-fail-fast` | 4,137 passed, 15 ignored, **1 failed**: `split_parents_stay_facades` (`cena-gui/src/keys.rs` 459 lines, cap 450; `cena/src/play.rs` a split parent with no cap). **Both are committed**, not in-flight (AR-1) |
| `cargo doc --workspace --no-deps` | **fails**: `cena-behavior/src/hunt/drive.rs:15` links `Gate::Act`, whose import the in-flight change removed |
| JavaScript tests | 72 passed |
| CI | **No commit since 2026-09-28 has been through CI**: `main` is 257 commits ahead of `origin/main`. The arch suite has been red locally for 66 commits (AR-1). `docs.yml` has failed on both pushes since `rodio` landed: it does not install `libasound2-dev` (AR-2) |

## The three patterns behind most of the findings

### 1. The round trip ends at the next prompt, and the game prompts after everything

`plan/12` §4.4 defines a command's answer as everything until the next `Frame::Prompt`. The game sends a
prompt after every burst of output, asked for or not, and §4.4's table has no row for unasked output inside
an open window. The session implements it exactly (`actor/io.rs:601-606`, `:685-686`). §4.4 says it ports
Lich's `fput`, but `fput` (`global_defs.rb:1479`) and `dothistimeout` (`:1949`) read line by line and match
the expected answer; neither waits for a prompt. The author found it live on 2026-09-30 (`fire` ×3 in 170 ms).

The in-flight `hunt/answer.rs` fixes the hunt's own sends by content. Everything else still takes the first
prompt, and once one window closes early the shift chains: each later back-to-back command reads the previous
one's reply until a move or pause lets the stream catch up (BE-D-8, SE-A). What it does, by area:

- **Spends or discards things (P1):** a second day pass (BE-D-1), a valid pass dropped (BE-D-2), a herb batch
  bought twice (BE-E-3, pinned as intended by `tests/heal_stock.rs:268`), kept ALTER items, transmogs and
  scrolls sold (BE-E-7), a working wand dropped (BE-A-6).
- **Misleads (P2):** a key left in hand unrecorded (BE-C-2), a language changed and never restored (BE-C-3),
  "too poor" after a successful withdrawal (BE-C-7, BE-D-6), an amulet or ticket bought again (BE-D-3, -5),
  a boon creature stored traitless for good (BE-A-9, MO-E-4), waggle multicasting on a full target (BE-F-3),
  `;multi` silently dropping a refused line (BE-F-2, BI-D), a follower hunting solo (BE-B-8), an agent's
  `command` answered with the wrong text and every later one shifted (SE-B, AG-A).
- **Hides game text:** inside a quiet window (the login sync, `;foreach`'s looks), every unasked main-stream
  line is dropped from the story for good, and the real report then spills into it (SE-A-4, GU-A-3, WE-1,
  SE-B-2 with Lich). Display hooks show every quiet report (SE-A-3).
- **The in-flight fix has its own holes:** any line containing "Roundtime" (speech included) answers an
  attack or cast (BE-A-2), and a stale `...wait N` from the previous line makes this one resend and run twice
  (BE-A-3). `wave`, `assess`, `appraise`, `sell`, `order`, `buy`, `spell active` and the group commands have
  no answer row.
- **Why no test caught it:** `cena-platform/src/answering.rs`, which every session and behavior test talks
  to, cannot send unasked output after login. Its doc (`:28-33`) calls an unasked prompt mis-attribution
  (PL-4, AR). `plan/06` §5 forbids exactly that kind of double ("a mock encodes our belief about the
  protocol"). The `fire` ×3 bug has no replay fixture, which `plan/06` §1.4 requires.

**Recommendation.** Move `answer.rs`'s idea into the session as the round trip's contract: every send says
what answers it, Lich's `dothistimeout` shape, with the prompt as a hint and not the terminator. The rule of
three is long met (hunt, travel, batch, sync, agent, heal, town). Give the double a `say(bytes)` that
delivers unasked output, and write the replay fixture first.

### 2. Other players' speech is read as the game

The model marks spoken lines (`ChunkLine::is_spoken`), but most classifiers never check it. Lich's patterns
usually require a link or an anchor that speech cannot carry; Hydra's match anywhere in the line.

- Disarm: `"Your sword tears free from your hands and floats"` makes the hunt `get <speaker>` for ten turns,
  neither fighting nor fleeing, then end (MO-B-4, P1; `incident.rs:253`, Lich `messages.rb:94`).
- Selling: `"I'll give you 5 silver"` during `appraise` sells a kept item (MO-B-6, P1).
- Travel: `"You may not pass."` bans a good exit; a "must be standing" line sends `stand` and the move again
  (BE-C-1, MO-A-4).
- Also: the bounty task rewritten (MO-B-1), hive-trap prose walking the hunt out (MO-B-5), the Empyrean
  captain's death faked (MO-E-1), "but it has no effect" ending the hunt (BE-A-8), seeking's vision (BE-D-9),
  `;foreach` skipping on "That is closed." (BE-F-5).

**Recommendation.** One rule for every classifier that drives automation: a spoken line is never the game's.
Enforce it where the chunk is read, not per pattern.

### 3. Hydra's command line does not know who sent a line

`SessionHandle::typed` (`command/handle.rs:342`) takes no origin, so anything that reaches it is the player.
A trigger's send fills captures with no escaping (`trigger/respond.rs:228`) and hands the line to `typed`
(`cena/src/triggers/act.rs:51`). A trigger with `send = "$1"`, or `.multi 1,say $1`, therefore lets another
player run `.agent level takeover`, `.agent approve`, `.trigger approve` or `.lich on` (BI-D-2; MO-F-3). A
Ruby script's `send` and every line the player's Lich writes reach the same place (BI-B-2), and through
`.all` one such line reaches every character. The agent itself cannot get there (SE-B). The only guard,
`reaches_others`, blocks `.to`/`.all` and splits only on commas (BI-D-1). **Fix:** pass the origin into the
command line and refuse the `agent`, `trigger`, `lich` and relay families to anything but `Manual`.

## Crate by crate

Counts are P1/P2/P3. The area file has every finding; the lines below are the ones that matter most.

### cena-platform (PL: 0/0/4)
Login security held on every point: a pin mismatch is fatal before anything is sent, and the fallback never
resends the password elsewhere. P3s: the log sink's timestamps, roll boundaries, a same-second overwrite, and
PL-4 (the test double above).

### cena-protocol (PR: 0/1/4)
One `Frame::Prompt` per prompt, exactly; nothing else parses as one. **PR-1:** R9 is fixed for `roundTime`,
`castTime` and `timer` but still open for progress bars: a broken `value=` becomes 0% health or mana, which the
hunt's death switch and rest trigger read (latent; no evidence the game sends one). PR-5: a prompt sharing a
line with a malformed tag is swallowed. PR-2: the tag/arm cross-check is blind to multi-line arms (34 names).

### cena-model (MO-A..F: 3/17/19)
- **MO-C-3 (P1):** `profile <other>` writes the other character's society, rank, citizenship and age into
  this one and saves them. Lich stops at a foreign `Name:` (`parser.rb:585`); Hydra skips `Name:` by design.
- **MO-B-4, MO-B-6 (P1):** speech, above.
- MO-A-1: stow and ready lists keep their container ids across a reconnect, so loot drags to containers that
  no longer exist. MO-A-2: an unpopped `inv` stream eats the rest of the window (an archer's shots vanish
  from combat, loot and incidents).
- MO-C-4: `profile` erases a known society rank (wracking off for 30 days). MO-C-7: a herb on a rank-1 nerve
  wound leaves "wound" when it is now a scar; heal eats the wrong herb up to 20 times. MO-C-1, -2: Standing,
  currency and enhancives re-sync at every login for good.
- MO-E-2: effect end times are stamped against the previous prompt, so after a quiet stretch a fresh spell
  reads as expiring and is recast. MO-D-1: an unknown-length stun is applied as 999 rounds. MO-D-2: another
  player's attack can claim our held pre-flares (Lich does the same).
- MO-F-1: `expiring` answers unknown whenever the effects list is empty, so a step waiting for the last buff
  to expire never fires. MO-F-2 (IN-FLIGHT), with BE-F-1: `cast::power` sends 9700+ by display name, which
  is wrong for 9821, 9808, 9803, 9725 and 9920 and drops every target; the right verb is in
  `spell_extras.tsv`'s cast column.

### cena-map and cena-host (MA: 0/0/4)
Decoding, routing (bad prices are walls, unknown conditions block) and the session table hold. R11 and R6
are fixed. Under Recommendations, but worth promoting: with a room number the map has never seen and one
unnumbered room sharing the title, `locate` names that room even though its description does not match, so
travel would start from the wrong room. HO-1: `take_all` at shutdown reserves no accounts.

### cena-session (SE-A, -B, -C: 0/9/11)
- SE-A-1: since `9f3cbb68` a typed `kill <tag>` goes out unresolved (only the queue's arm calls `retarget`);
  `tests/creature_tags.rs` sends by another path. SE-A-2: an abandoned window strands its owed prompts, which
  later eat a behavior's reply. SE-A-7: a reconnect answers an instant send `Dead`, not `Interrupted`, so a
  network blip mid-walk ends the hunt and tells the group the member closed.
- SE-B-1: `toss <item>` drops an item and passes the `commands` level's denylist. I1, I2, I4 fixed.
- SE-C-1: R5 is incomplete: `save_level` (`cena/src/agent.rs:416`) and `keep_switch` (`cena/src/lich.rs:300`)
  write the settings file outside the lock, so a drop to Observe can be lost and the next login is back at
  `takeover`. SE-C-2: the player log's folder is keyed by name alone, so retention on one instance deletes a
  same-named character's history on another.

### cena-behavior (BE-A..F: 9/34/10)
- **Hunt (BE-A):** **BE-A-4 (P1),** the watchdog beats once a turn (`drive.rs:272`; no other `beat()` in
  `hunt/`), so a walk to rest, a selling round or a heal longer than 30 s is preempted as wedged.
  **BE-A-5 (P1),** "You can't target" makes the hunt resend `target #id` every round trip until the creature
  leaves. **BE-A-6 (P1),** the wand above. BE-A-7: a disconnect mid-send ends the hunt. BE-A-10: R1 is still
  partly open (a lag seen by `drain` waits a turn). BE-A-1: `rooted` is never cleared.
- **Groups (BE-B):** **BE-B-4 (P1),** group boards and the handover table are keyed by bare name across game
  instances. R6 was fixed for seats, not boards. BE-B-5, -6: a dead member carried out without the hold
  having taken; a link-dead member left behind. BE-B-1: bigshot `script <name>` entries import as game lines.
- **Travel (BE-C):** all seven are P2 (speech, a late reply, an urchin "no" never asked again, a trip ended by
  `LOST_WAIT` leaving weapons stored). BE-C-4 contradicts `travel.rs:47-50`'s promise.
- **Routines (BE-D):** BE-D-1, -2 (P1) above. BE-D-4: the cutter's ticket search reads an inventory snapshot
  nothing requests, and its test injects one. BE-D-10: patrol can send 2,000 `stand`s (upstream stops at 10).
- **Loot, town, heal (BE-E):** **BE-E-6 (P1),** the weapon stowed to free a hand goes into the selling bag and
  is sold at a later shop (needs `weapon` in the sell types and no ready weapon set). BE-E-3, -7 (P1) above.
  BE-E-1: eloot's `sell_exclude` regexes import as literal text and protect nothing. BE-E-5: a pooled box whose
  contents were never seen is trashed. The errand drivers have no test against a scripted game, and all three
  P1s sit on that seam.
- **The rest (BE-F):** BE-F-4: an imported trigger's `sound` path is opened as given (see BI-B-1).

### cena-ui (UI: 0/2/5)
UI-4: `parse_hex` slices by byte and panics on `#12345é1`, reachable from a theme file at launch and from
`;theme accent`. UI-7: the trigger editor's live test refuses every character-limited or switched-off draft.
R10 and R14 fixed. `WIRE_VERSION` stayed 1 through a required-field addition (a recommendation, given the
document's stance).

### cena-web (WE: 0/2/2)
The boundary holds: loopback, a 256-bit token in the fragment compared in constant time, exact Host and
Origin, no `innerHTML`. WE-1: quiet windows above. **WE-2:** the built-in map (`Cargo.lock` hydra-mapper
`defeb84`) does not match the explorer snapshot's hash, so on a default run the minimap says "Map versions
differ" and hunt setup refuses. I6 fixed.

### cena-agent (AG-A, AG-B: 0/6/5)
- The MCP listener holds (loopback, bearer token, Host checked). `records` is read-only, one statement, 500
  rows, 5 s. I3 is only partly fixed: AG-A-1, a restarted character restarts its cursor and generation at 0
  unannounced, so an act decided against the old session is admitted on the new one.
- **AG-B-3:** the runner, the checker and Lich inherit Hydra's environment, `CENA_PASSWORD_<ACCOUNT>`
  included; any script can read the password. AG-B-1: Lich not reading its socket jams the relay; `;lich off`
  hangs and `;lich on` can start a second Lich. AG-B-2: Lich's `Vars` are not saved when a runner ends (up to
  five minutes lost). I5 fixed. No game text reaches a Ruby `eval`.

### cena-gui (GU-A..D: 0/10/20)
- GU-A-1: a window saved on a monitor since unplugged reopens off-screen, the hub included. GU-A-2 (inferred,
  check by hand): a typed password survives Log out in the field's undo history (Ctrl+Z).
- GU-B-1: an unreadable `_presets.json` loads as empty and the next save overwrites the player's presets (the
  layout file has a guard for this; presets do not). A widget following another character never sends on the
  wrong one.
- GU-C-1 (inferred): an image over 8192 px panics the window thread and every session with it, again at every
  start because the path is saved. GU-C-3, -5: "None." and "Spell: none" for what the game never said, with
  tests asserting the wrong text.
- GU-D-6: *Duplicate* on an imported, unapproved trigger saves a copy that sends without approval (consider
  P1). GU-D-1: a macro's commands are sent by separate tasks and can reach the game out of order. GU-D-2:
  macro commands behind an `s` wait cannot be cancelled and go to a new connection after a reconnect.
- R6, R8, R10, R12, R13, R14 fixed (GU-B-6: an old story line's link still sends its pre-reconnect id).

### cena, the binary (BI-A..D: 1/7/15)
- **BI-B-1 (P1):** a trigger sound path from a shared Wrayth file can be a UNC path; Hydra checks it at every
  trigger reload and plays it, which on Windows can leak the player's NTLM hash to the file's author
  (CVE-2023-23397's mechanism).
- BI-B-2, BI-D-1, BI-D-2: the command-line origin, above.
- BI-C-1: a panic in any hunt-desk run leaves the authority held for the session; every later hunt command
  waits forever and `;go2` is refused, silently. BI-A-1: `--first <cmd>` is sent again on every hub start
  and reconnect. BI-C-2, -3: three readings of a settings switch disagree (`yes`/`on`/`true`).
- R5, R6, R11, I3 fixed in their paths; nothing tests `play.rs`'s `Table`.

### cena-arch-tests, docs, CI (AR: 0/2/6)
AR-1 and AR-2 are in the baseline above. AR-3: a raw identifier (`r#type`) makes the lexer read to the end of
the file, so every token rule fails open after it (latent). AR-4: the colour-literal scan stops at the first
`#[cfg(test)]`, leaving about 3,700 production lines unscanned. AR-5: `architecture.rs` and the glossary
still say the command symbol is `;` (it has been `.` since 2026-09-29). AR-7: citations in manifests and
crate `.md` files are never checked. The glossary and `architecture.rs:233-236` restate the first-prompt
round trip and call it Lich's `fput`.

## Earlier findings re-checked

Fixed: R2, R3, R4, R7, R8, R10, R11, R12, R13, R14, I1, I2, I4, I5, I6. Partly open: **R1** (BE-A-10),
**R5** (SE-C-1), **R6** (boards, BE-B-4; an old link, GU-B-6), **R9** (progress bars, PR-1), **I3**
(restarts, AG-A-1).

## Suggested order

1. The command line's origin (pattern 3) and the child-process environment (AG-B-3): small fixes, and they
   close the security findings. Then BI-B-1's sound paths (refuse anything but a local file), and the
   agent's denylist and quit gate (L-1, L-2, SE-B-1).
2. The round trip (pattern 1): answer by content in the session, a `say()` on the test double, and a replay
   fixture of the 2026-09-30 hunt. This retires most of the P1s and P2s in behavior and the agent.
3. The speech rule (pattern 2) at the chunk reader.
4. The rest of the P1s: the hunt's heartbeat (BE-A-4), untargetable creatures (BE-A-5), the selling bag
   (BE-E-6), `profile`'s name (MO-C-3), boards by session (BE-B-4).
5. Get CI green and running again: the two caps, `docs.yml`'s ALSA package, and a push so CI sees the 257
   commits.

## The author's answers, 2026-10-01, and what they change

> *"the game sends a prompt every time it sends something. A prompt is it's way of saying "over" if it was
> talking on a radio."*

1. **A prompt after every burst, a type-ahead refusal included** (SE-A's question). This is pattern 1's
   premise in the author's words: a prompt says "I have finished speaking", never "I have answered you".
   The owed-prompt ledger's one-prompt-per-line assumption holds for a refused line.
2. **Creature ids are stable unless the server reboots** (MO-E). Holding a target's id across a reconnect is
   sound; item ids still change at every login (`plan/59` §1b), so MO-A-1 stands. Still open: whether a
   *player's* negative `exist` id is the same on every login (PR's fixture-privacy question).
3. **`_drag` onto a player or a bin gives or discards it, and the game takes `qui` as quit** (SE-B). Two new
   findings, both VERIFIED by the lead:
   - **L-1 [P1]:** the `commands` level's denylist refuses a `_drag` only onto `drop` or the ground
     (`cena-session/src/agent/denylist.rs:131-135`). `_drag #<item> #<player>` gives the item away and
     `_drag #<item> #<bin>` destroys it, both through a level whose promise is "never drop, give, sell or
     destroy anything". Rated P1 as I4 was; SE-B-1 (`toss`) is raised to P1 with it.
   - **L-2 [P2]:** a quit is recognised only as exactly `quit` or `exit` (`actor/ending.rs:474-478`), so `qui`
     skips `may_quit` (`actor/io.rs:140`, `:234`): an agent below `takeover`, or a behavior, logs the
     character out, which the author's rule of 2026-09-29 forbids. The session then reads the close as a
     dropped connection and reconnects. `quit`'s prefixes belong in `is_exit_intent`, and in the denylist.
4. **Seeking's confirm does take the walker to the last place offered**, so BE-D-9 (a destination read one
   offer late, then confirmed) teleports the walker to the wrong place: **raised to P1**. The underwater
   signposts route stays open (nobody has been there); guarding the stale-room stroke costs little either way.
5. **A combat drop-down's target name cannot contain a comma** (GU-C): the comma split is safe, no finding.
6. **`--first` is not remembered.** It is an argument from the first live run (`cena/src/travel.rs:49`,
   author, 2026-09-21: *"log in, move south, then walk to the bank"*): `--first south` sends one line as the
   player before anything else. BI-A-1 found it is sent again on every hub start and reconnect, and goes past
   Hydra's command line. **Recommendation: delete it**, rather than fix it.

**Totals after the answers: 16 P1, 89 P2, 105 P3.**
