# 38 — Scripts in any language, through the connection

**Status: PROPOSED 2026-09-25, author asked for it** (*"I would like you to write up a plan
yes, we're not to the point of implementing any of it yet. Just trying to wrap my head around
it."*). Nothing here is built, and nothing is scheduled. This plan extends
[`plan/35-m7-agent.md`](35-m7-agent.md): M7's connection, used by programs as well as by an
agent.

Every line below is marked. **AUTHOR** is a decision or position the author gave, quoted and
dated. **MEASURED** gives the command or the source. Everything else is **PROPOSED**: mine,
and open until the author says otherwise.

---

## 0. The author's position

All 2026-09-25, in the order given:

1. *"for M7 we said MCP server for the agent... You could also use that as a scripting
   vehicle hmm?"* Then, correcting me: *"not the agent, the connection"*.
2. *"But a running instance of lich is like 200-400 mb with all the ruby bloat."* (§2
   measures it.)
3. *"Look I don't care if people have to change their scripts, I was just trying to keep it
   in a place they could write their scripts in ruby like their use to and they work with
   hydra. If that means making a pared down version of lich that gets it's data from hydra
   then that is an option."*
4. *"You know we converted lich's api to rust, so it should all be there in some form right?
   So we could even support thiings like Lich::Messaging, ect?"* (§6 measures it.)
5. *"Hydra has all the data. Hydra can capture any data we need. Faster than anyone else.
   With the MCP connection we can make a bridge for any scripting language no? Lich does what
   lich does because it was built over a decade and ruby updates. Then you have to keep
   backwards compatibility for the person who hasn't updated their scripts in 10 years."*

6. On the map (§6d): *"lich would be a controller, it would get information fed from hydra
   and act on that information, just like it gets information from lich and acts on it. So it
   really wouldn't even need to know how to do the scripted edges. The exception would be if
   they wanted to make a script for like an external map viewer or so on, but at that point
   they should probably have their own data source or maybe a way to access the tsvs."*
   Changing the map at runtime: *"Yeah that's a bandaid, we wouldn't do bandaids in hydra."*
   Letting scripts see the whole map: *"yeah no sense to limit what they can see!"*

So the goal is **familiar, not compatible**. A player writes Ruby the way they are used to,
and the script works with Hydra. Scripts may have to change. Any language can have a bridge,
and Ruby comes first because it is what the community writes.

## 1. What this changes, if the author takes it

`CLAUDE.md`, **Settled decisions**, says: *"No embedded scripting language _for now_.
Automation is curated Rust behaviors ... Users do not author scripts in any milestone
currently planned."*

This plan keeps the first half and changes the second:

- **Nothing is embedded.** Scripts run in their own process, in their own language's
  runtime, and talk to Hydra over the connection. Hydra ships no interpreter and no `#[cfg]`
  for one.
- **Users would author scripts**, out of process. That is the line that changes, and only
  the author changes a settled line. **This plan does not edit `CLAUDE.md`.**
- **Curated behaviors stay primary**: Hunt, Loot, Heal, Bounty and Travel, in Rust. Scripts
  are for the long tail. Of the author's short list (`inventory/12-lich-repo-mirror.md` §11),
  `trollspeak`, `blue-tracker`'s Discord posting and `lumnismon` are scripts' work, not
  behaviors'.
- **Lua stays deferred** (the `CORRECTED 2026-09-21` note in `CLAUDE.md`), and this may make
  it unnecessary: Lua, if it came, would be one more bridge.

## 2. Considered and set aside

### 2a. Real Lich, with Hydra standing in for the game

Lich can already be pointed at a host and port (`--game`/`-g`,
`reference/lich-5/lib/main/argv_options.rb:246`; the socket opens at
`reference/lich-5/lib/main/main.rb:591`). So Hydra could log in, listen on loopback, and hand
Lich a copy of the stream, and every Lich script would run unchanged.

**Set aside on memory.** MEASURED 2026-09-25 on the author's running Lich (`rubyw`, from
PowerShell's `Get-Process`, and the author running `;e` in it):

| | |
|---|---:|
| Physical memory in use (working set) | 295 MB |
| Committed private memory | 466 MB |
| Ruby's live heap (`ObjectSpace.memsize_of_all`) | 130 MB, 1,374,034 objects |
| Of which the map, 36,838 rooms (a walk of `Map`'s `@@list`) | **100 MB, 1,099,808 objects** |
| GTK's windowing library (`libgtk-3`) | not loaded |

Lich keeps one character per process (`XMLData`, `GameObj` and `Char` are process-wide), and
any script touching `Room.current` loads the map. So a relay costs about 470 MB per
character, about 11.7 GB committed for 25, most of it 25 copies of one map. Kept as a
possible escape hatch for one character and one unported script. Not planned.

### 2b. Re-implementing Lich's API exactly

`research/11-player-requirements.md` §C.4 concluded that script compatibility *"cannot be
written down, so it can never be met"*. And the author does not want it (§0.3). Set aside.

### 2c. An embedded runtime

Deferred by `CLAUDE.md`, and not needed if this works (§1).

## 3. The shape

```text
 game ──► Hydra: one parser, one model per character, one map, one spell table
            │
            ├── the connection (plan/35: loopback, token, versioned contract)
            │      │
            │      ├── an agent (plan/35)
            │      ├── the Ruby bridge: one runner process, many scripts, any character
            │      └── a Python bridge, a Node bridge, ... (anyone can write one)
            │
            └── Despana / the GUI: who is driving, the stop button, script output
```

- **One connection, language-neutral** (PROPOSED). A bridge is a thin client library in one
  language, plus a **runner**: one process that runs many scripts, each bound to a character
  by default and able to name another.
- **A script is a controller** (AUTHOR, §0.6). It is fed information by Hydra and acts on
  it, as a Lich script is fed by Lich. Hydra captures, models, and carries out what is asked:
  a script asks to go somewhere, and Hydra walks there.
- **Hydra keeps the data and the parsing.** Scripts subscribe to events and lines, query the
  model, and send commands. No bridge parses the game stream or loads the map.
- **`;name` typed in Hydra starts a script** in the runner configured for it, the way Lich
  does now. Stopping a character stops its scripts.
- **Script output comes back as a styled message** to a window: `Lich::Messaging`'s model
  (§6c).
- **Safety is plan/35's.** Each client is named and has a control level
  (plan/35 §3). Its commands go through as `Origin::Script`
  (`crates/cena-session/src/command/verdict.rs`), through the same gate and denylist. Lines
  are read-only: a bridge can watch a line, never rewrite or hide one. Hiding is M8's squelch
  and highlights. `bigshit2.lic` hides every game line from the player
  (`inventory/12-lich-repo-mirror.md` §6), and that is the argument.

## 4. What the connection needs beyond plan/35

plan/35 designs the connection for an LLM. A program needs six more things (PROPOSED):

1. **Push.** plan/35 §2 chose `wait(since, kinds, timeout)`, a pull, because a generic MCP
   client cannot be relied on to wake a model. A program holds a stream. Keep `wait` for
   agents, and add a subscription that delivers events and lines as they happen, and says so
   when it lags. `SessionObserver::subscribe` already reports `Lagged` rather than dropping
   silently (`crates/cena-session/src/observation.rs`).
2. **Send and wait.** Most Lich scripts are built on Lich's `dothistimeout`: send a command,
   then wait for one of these replies. Done on the script's side, it races the game. Done in
   Hydra, it is one call: the command, the patterns, the timeout, and back come the reply
   lines up to the one that matched. It is an operation (plan/35 §6) with a reply attached.
3. **Lines.** Each game line's text as the parser produced it, with its stream, beside the
   typed facts. Scripts write their own patterns, and they will keep doing so. This is still
   parse-first: a line is the parser's text, not raw bytes, and no `Frame` crosses (plan/35
   §7).
4. **Queries** over the model: room, objects and hands, the character, effects, spells, the
   map and a path, bounty. §6 lists them against what exists.
5. **Script output**: a message with a style, a target window, an optional command link and
   a mono flag (§6c).
6. **Stores**: per-script and per-character settings, Lich's `Settings`, `CharSettings` and
   `UserVars`. PROPOSED: kept in Hydra, beside `crates/cena-session/src/settings_store.rs`,
   so that one script's settings are the same whichever bridge runs it, and a
   multi-character script reads every character's.

**The contract is versioned from its first release**, as `crates/cena-ui/WIRE.md` versions the
frontend projection. The compatibility promise is the protocol, not a runtime (§8).

## 5. Transport: MCP, or plain JSON

MCP is JSON-RPC over HTTP, so any language can speak it, with an SDK or by hand. But its
framing is tool calls, made for LLM hosts, and push to a generic client is exactly what
plan/35 §2 did not trust. A script bridge may be simpler over plain JSON on a WebSocket, which
Despana's server already speaks.

plan/35 §2 already keeps the transport a thin layer over one core ("a layering rule, not a
trait"). So PROPOSED: build the core once. Build the Ruby bridge against MCP first, and measure
the friction: the round trip of a send-and-wait, and whether notifications arrive when they
should. Add a plain layer only if MCP fails a real script. Two transports on day one is what
`plan/05` §-1 refuses.

## 6. The Ruby bridge: familiar Lich, data from Hydra

### 6a. Lich's calls, and where each would come from

The author's point (§0.4) holds for the game model and not for the script engine. MEASURED
2026-09-25, by counting the `crates/` files that cite each Lich file by name:

```sh
cd reference/lich-5/lib
for f in common/*.rb gemstone/*.rb; do
  grep -rlF --include=*.rs "$(basename $f)" ../../../crates | wc -l
done
```

- **The game model is ported**: 30 of the 93 top-level files are cited, and they are the
  model's (`xmlparser`, `gameobj`, `spell`, `creature`, `infomon`, `group`, `bank`, `fog`,
  `move`, `stance`, `injured`, `claim`, `inventory`, `psms`, `society`, `wounds`, `scars`,
  `readylist`, `stowlist`, `disk`, `armaments`, `effects`...). So is `gemstone/combat/`
  (18 of 19 files) and `gemstone/critranks/` (21 of 21). The count **undercounts**: `bounty.rb`,
  `currency.rb` and `experience.rb` show zero, but Cena has `crates/cena-model/src/state/bounty.rs`,
  `crates/cena-model/src/state/character/currency.rs` and the experience report, cited by
  the files under them.
- **The script engine is not ported**, because Hydra had no scripts: `script.rb` (3,675
  lines), `downstreamhook.rb`, `upstreamhook.rb`, `vars.rb`, `uservars.rb`, `settings.rb`,
  `watchfor.rb`, `arg_parser.rb`: zero citations each. That is the runner's to write.

Against the census of what GemStone scripts call (`inventory/07-script-corpus-api-census.md`
§3.1; calls and files over the 234 GemStone scripts):

| Lich, as scripts call it | GS calls / files | From Hydra | Half |
|---|---:|---|---|
| `GameObj` (npcs, loot, pcs, hands, inv, room) | 1,621 / 108 | `crates/cena-model/src/state/gameobj.rs`, `state/creatures.rs`, `state/hands.rs`, `state/inventory.rs`, `state/room.rs` | model |
| `Spell[...]`, `Spell.active`, `Effects` | 1,276 / 64 | `crates/cena-model/src/spells.rs`, `crates/cena-model/src/effects.rs` | model |
| `Room.*`, `Room[...]`, `Map.*`, go2 | 826 + 480 + 320 | `crates/cena-map`, travel (`crates/cena-behavior/src/travel.rs`) | model |
| `XMLData.*` (about 10 fields in use) | 505 / 71 | a typed query per field | model |
| `Char`, `Stats`, `Skills`, `Society` | 586 + 213 + 494 | `state/character/stats.rs`, `state/character/skills.rs`, `state/vitals.rs`, `state/societies.rs` | model |
| `Bounty` | | `crates/cena-model/src/state/bounty.rs` | model |
| `waitrt?`, `waitcastrt?`, `checkrt` | | `GameState::in_roundtime` (`crates/cena-model/src/state/clock.rs`) | model |
| `put`, `fput`, `multifput` | | a send, as `Origin::Script` | connection |
| `dothistimeout`, `waitfor`, `matchtimeout` | | send-and-wait, the line stream (§4) | connection |
| `echo`, `respond`, `Lich::Messaging` | | script output (§6c) | connection |
| `UserVars`, `CharSettings`, `Settings`, `Vars` | 1,464 + 896 + 262 | the stores (§4.6) | engine |
| `Script.*`, `start_script`, `kill_script`, `before_dying` | 1,256 / 154 | the runner | engine |
| `DownstreamHook` | 197 / 47 | the line stream, read-only | engine |
| `Log` | 259 / 17 | the runner's log | engine |
| `Gtk` | 55 files (§6 Tier 2) | not provided; not blocked either (§6b) | — |
| `DRC*`, `Flags`, `DRStats` | | DragonRealms, deferred (`CLAUDE.md`) | — |

**One Lich hazard disappears.** Lich stores spell durations and costs as Ruby expression
strings and `eval`s them at runtime (`inventory/07` §6, "One hazard that is not in any
table"). Hydra already
evaluates those formulas for the character (`crates/cena-model/src/spells/expr.rs`, plan/37
Stage 2), so the bridge's `Spell[...]` asks Hydra and evaluates nothing.

**Real Ruby removes the census's hardest items.** Blocks (79.6% of the corpus), regex with
lookarounds and named captures (68.0%), threads, `rescue`/`retry` and `require` were the
blockers for a Lua API (`inventory/07` §6). In Ruby they need no work.

### 6b. What a script author changes

- **Hooks read, never rewrite.** `DownstreamHook` becomes a subscription to lines. A script
  that squelched or rewrote lines moves that to Hydra's highlights and squelch (M8).
- **No raw XML.** `XMLData`'s fields in use become typed queries. `$_SERVERBUFFER_` and the
  parser's internals do not exist.
- **Other characters are reachable.** A call defaults to the script's character and may name
  another. What `invdb` and Ashborne do with files (`inventory/12-lich-repo-mirror.md` §5)
  becomes a query.
- **`$globals` still work between scripts in one runner**, since it is one Ruby process (the
  census counts 111 files using them for cross-script messages). Not across languages.
- **GTK is not provided, but not blocked.** The runner is ordinary Ruby; a script that
  requires the GTK gem can still open windows, at its own memory cost.
- **Unknown is not false** (§6e).

### 6c. `Lich::Messaging`: supportable, and small

148 lines (`reference/lich-5/lib/messaging.rb`), all presentation:

- `msg(type, text)`: `error`, `bold`, `monster` in bold (monsterbold); `warn`, `thought` in the
  thought preset; `info`, `whisper` in the whisper preset; `speech`, `debug` in the speech
  preset; and the named colours.
- `stream_window(text, window)`: to `familiar`, `speech`, `thoughts`, `loot` or `voln`.
- `mono(text)`, and `make_cmd_link(text, command)`.

In the bridge each becomes one script-output message: a style, a window, an optional link, a
mono flag. Hydra shows it the way it shows the game's own presets and streams
(`crates/cena-model/src/state/stream_windows.rs` names the windows). Nothing in it is hard;
what Hydra needs is "a script said this, styled so, in this window" in what it sends
frontends.

### 6d. The map: queried, never loaded

The author, 2026-09-25: *"how do we get around ruby needing to have the map in memory? Do the
scripts query hydras mapdb I guess?"* Yes. The map is loaded once, in Hydra, from one binary
file (`crates/cena-map/src/binary.rs`), for every character. The bridge's `Room` and `Map` are
thin stand-ins that ask. MEASURED, the map calls in the elanthia-online scripts
(`grep -rn` over `reference/scripts/scripts/*.lic`, counted per call):

| Lich call | Uses | Bridge | Hydra has |
|---|---:|---|---|
| `Room.current` | 768 | **no query**: Hydra pushes the room on every move, and the bridge keeps it | `crates/cena-map/src/locate.rs` |
| `Room[id]` | | fetched once, cached | `Map::room` (`crates/cena-map/src/map.rs`) |
| `Map.ids_from_uid` | 77 | one query | `Map::ids_for_uid` |
| `Map.dijkstra`, `findpath` | 30 | one query; Hydra returns the path and its cost | `crates/cena-map/src/route.rs` |
| `Map.list`, `Room.list`, iterated to find something | 93 + 44 | **the call that changes**: a question instead of a loop, such as rooms with a tag, the nearest room with a tag, or rooms whose title matches | travel's destination rule already finds the nearest tagged room (`crates/cena-behavior/src/travel/itinerary.rs:88-153`) |
| go2 | | a travel command; the script waits for arrival | travel, and the crossings as Rust steps and routines (`crates/cena-map/src/step.rs`, `routine.rs`) |

So the bridge holds a handful of rooms, not 36,838. Iterating the map in Ruby was always
Lich's costly pattern: §2a measures it at 1.1 million objects.

Three things Lich scripts do with the map, decided by the author (§0.6):

- **Crossings stay in Hydra.** Lich's `wayto` holds Ruby (`StringProc`); Hydra ported those
  crossings to steps and routines (`plan/21`). A script never needs them, because it does not
  walk: it asks, and Hydra walks. A bridge could translate the crossings back into Ruby, and
  there is no reason to. `Room[x].wayto` returns an exit's crossing as data, for reading only.
- **No runtime map changes.** `sailors_grief_swim_fix.lic` (on the author's short list) patches
  `StringProc` crossings into the loaded map. The author: *"that's a bandaid, we wouldn't do
  bandaids in hydra."* A wrong or missing crossing is fixed in the map's data, through
  `plan/21`'s pipeline, for everyone.
- **Scripts may see all of it.** The author: *"no sense to limit what they can see."* The
  queries above answer the common questions cheaply, and the whole map stays readable for
  anything else. PROPOSED, two ways, both read-only:
  - **Over the connection**: a paged listing of rooms with every field, crossings as data.
    A script that wants the whole map pays for it in its own memory, as it chose.
  - **As files**: the converted per-room JSON (`<dir>/index.json` and
    `rooms/<thousand>/<id>.json`, `crates/cena-map/src/files.rs`) and the game-data TSVs
    (`crates/cena-model/data/`), documented as a published, versioned artifact. This is the
    author's *"their own data source or maybe a way to access the tsvs"* for an external map
    viewer, which needs the whole map and gains nothing from a round trip per room.

### 6e. Unknown is not false

Cena distinguishes *"the game said no"* from *"nobody has said"*, where Lich answers `false`
or `nil` for both: `CreatureInstance::hostile` is an `Option<bool>`
(`crates/cena-model/src/state/creatures/instance.rs`), and `Effects::active_in` separates a
stated absence from silence (`crates/cena-model/src/effects.rs`). The bridge must choose,
and choose **once**, in its contract, not call by call. PROPOSED: predicates (`active?`,
`hostile?`) answer `false` for unknown, as Lich does, so scripts read the same; and each has a
companion that returns `nil` for unknown, for the scripts that care.

## 7. Other languages

Every bridge speaks the same contract, so a bridge is a small library, and anyone can write
one. PROPOSED: Hydra's project maintains the Ruby bridge first (§0.3). A second is the
author's call when someone wants it: Python has the largest general scripting population,
and Lua, if players want it, can be a bridge like any other, out of process.

## 8. Backwards compatibility: the author's point, by design

*"Then you have to keep backwards compatibility for the person who hasn't updated their
scripts in 10 years."* Lich carries that because scripts reach into its internals
(`XMLData`, `$_SERVERBUFFER_`, patched core classes), so every internal becomes a promise,
and because the scripts ride Ruby's own version changes.

Here, by construction:

- **A script can only use the contract.** It runs in another process. There is nothing else
  to reach.
- **The contract is small and versioned** (§4), as `crates/cena-ui/WIRE.md` is.
- **The language runtime is the user's.** A Ruby upgrade is the bridge's problem, and the
  bridge is small.
- **An old contract version is served beside the new one for a stated window, then
  dropped.** A bridge declares the version it speaks, and the health route names the
  versions served (plan/35 §2). A ten-year-old script gets a clear refusal, not a silent
  misreading.

The cost, stated plainly: the contract becomes a public promise. A breaking change costs a
version and a window, so the contract should stay small.

## 9. Speed and memory

What holds **by construction**: the game is parsed once, in Rust, per character. The map, the
spell table and the bestiary are loaded once for every character. A bridge parses nothing.

What is **not measured**: a script's reaction adds a loopback round trip that Lich's
in-process hooks do not have. It should be small against a game roundtime, but it is a guess
until measured. With the first bridge, measure the event delivery time, the send-and-wait
round trip, and the runner's memory with N scripts over M characters. §2a's table is the
comparison: everything in Lich except the map is about 30 MB of live data.

## 10. Steps, when M7 comes (not scheduled)

0. The author's decisions (§11).
1. plan/35's core, with §4's additions and the contract document.
2. The Ruby bridge's connection half: events, lines, send, send-and-wait, `echo` and
   `Lich::Messaging`.
3. **The first script end to end: `trollspeak` and `trollhear`** (145 and 114 lines, on the
   author's short list). They read speech lines and send speech: the smallest real round
   trip.
4. The game-model classes (§6a's model half).
5. The runner's engine: lifecycle, `before_dying`, stores, read-only hooks, `;name`
   forwarding.
6. **The second script: `wander`** (286 lines, also on the short list): room, objects,
   movement and `CharSettings`.
7. Measure (§9), and write the numbers here.

## 11. Open questions for the author

1. **The settled line** (§1): users author scripts, out of process, in any language?
2. **Lich's names**: keep `GameObj`, `Char`, `Room`, `Spell` and the rest top-level, as Lich
   has them? PROPOSED yes, so a script's diff stays small.
3. **Stores in Hydra or in the runner** (§4.6). PROPOSED Hydra.
4. **The milestone**: inside M7, or an M7b after the agent.
5. **Shipping**: does Hydra ship the Ruby bridge (Lich's players have Ruby installed), or is
   it installed separately?
6. **The Lich relay** (§2a): keep as a deferred escape hatch, or drop it.
7. **MCP only, or a plain JSON layer for bridges** (§5): PROPOSED, measure MCP first.
8. **Which data files are published** (§6d): the per-room map JSON and the game-data TSVs, and
   are they versioned with the contract or on their own?

