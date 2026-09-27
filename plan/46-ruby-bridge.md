# 46 — The Ruby bridge: how a Lich script runs against Hydra

**Status: PROPOSED 2026-09-27, author asked for it; the eleven questions ANSWERED the same day
(§10).** **Step 1 BUILT 2026-09-27** (§11), the author moving M7b ahead of M6's live run (§10,
question 11); the rest is not built. It
takes [`plan/38-scripting-bridge.md`](38-scripting-bridge.md)'s shape (scripts in their own
process, talking to Hydra over [`plan/35-m7-agent.md`](35-m7-agent.md)'s connection) down to how
it works, what Hydra has to answer, and which scripts it runs. The evidence is
[`inventory/13-ruby-bridge-evidence.md`](../inventory/13-ruby-bridge-evidence.md): Lich's runtime
read line by line, every Lich call mapped to Hydra, and both script collections measured.

> **RENUMBERED 2026-09-27** from `plan/45`, which M8's trigger plan (`plan/45-m8-triggers.md`,
> 2026-09-26) already held on its own branch; the two met when the branches merged. Sixty-eight
> files cite the triggers plan and three cited this one, so this one moved.

Marks as elsewhere: **AUTHOR** is quoted and dated; **MEASURED** cites its source; the rest is
**PROPOSED**, open until the author says otherwise.

---

## 0. The author's position

**AUTHOR, 2026-09-26:** *"We intend to allow scripting through the mcp connection for the agent.
We intend to accomplish this by writing a translator or whatever you want to call it. First one
will be ruby. The goal of it is to take a ruby script translate it into hydra speak and talk to
hydra over the mcp server. This is accomplished by having behaviors or whatever that they can
call and get answers, ect."*

That answers `plan/38` §11's first question (users write scripts, out of process), names Ruby
first, and names MCP as the transport.

## 1. What is translated: the calls, not the source

A translator could work on either of two things.

- **The source**: read a `.lic` and write something Hydra runs. Set aside. `inventory/07` §4
  measured blocks in 79.6% of scripts and regexes in 68.0%, with threads, `rescue`/`retry`,
  `eval` and `require` behind them. A translator of Ruby is an implementation of Ruby.
- **The calls** (PROPOSED): the script runs in real Ruby, unchanged where it can be. A library
  defines Lich's names (`fput`, `waitfor`, `GameObj`, `Room`, `Spell`...), and each call becomes
  "Hydra speak": a read of a local copy of the character, or a request on the connection (§4).
  That is the translator.

One piece does read source, as a tool, not as the path: **a checker** that reads a `.lic` and
lists the lines that will not work under Hydra and why (§6's list), from the census's own lexer
(`inventory/13-ruby-bridge-evidence/tools/rblex.py`). Run over both collections, it is also the
published answer to "does my script run?".

## 2. The pieces

```text
 game ─► Hydra, per character: parser ─► model ─► command queue
            │   the connection (plan/35: MCP, loopback, token)
            ▼
 runner (a Ruby process, one per character)
   ├─ the bridge: Lich's names, answered from a local copy and from requests
   ├─ Lich's script engine: threads, buffers, pause, kill, before_dying, `;` commands
   └─ scripts: .lic files, each a thread, as under Lich
```

**One runner per character** (PROPOSED; `plan/38` §3 had one for all). Ruby's globals belong to
the process, and scripts pass `$globals` to each other (MEASURED: 32 files in A, 260 in B,
`inventory/13` §3.1). Lich runs one character per process, so every such script assumes `$x` is
this character's. A shared runner would hand one character's `$bigshot_...` to another's scripts.
A script that drives several characters names them in its calls, as every request can.

**The cost is not measured.** A runner is a Ruby process plus the bridge and its scripts, with no
map and no parser; `plan/38` §2a measured Lich at 466 MB committed, 100 MB of it the map and
about 30 MB everything else. Measure a runner before deciding (§9).

## 3. One call's journey, and why it has no race

`fput "stand"`, under the bridge:

1. The bridge asks Hydra to send `stand`. Hydra queues it as `Origin::Script`
   (`crates/cena-session/src/command/verdict.rs:85`) and answers with the **cursor** of its
   `Sent` event.
2. Hydra pushes the lines that follow. Every event carries a cursor that only increases, across
   reconnects too (VERIFIED, `crates/cena-session/src/observation.rs:20`), and `Sent` is an event
   in that same stream (VERIFIED, `crates/cena-session/src/actor/event.rs:50`). So "the lines
   after my command" is exact.
3. The bridge runs fput's own rules over those lines, in Ruby: resend on `...wait N`, stand when
   struggling, fail on a stun (`inventory/13` §1.2).

**Matching stays in Ruby.** `plan/38` §4.2 proposed send-and-wait inside Hydra, to avoid racing
the game. Cursors remove the race, and Ruby matching keeps scripts' patterns exact: Rust's `regex`
has no lookaround and no backreferences, and scripts use both (`inventory/07` §4.2). The contract
gets smaller, not larger.

**Reads are local.** Scripts poll: `wait_until { checkrt == 0 }`, loops over `GameObj.npcs`. A
round trip per read would flood the connection. So the runner keeps a copy of its character
(§4.2), updated from the same stream in cursor order, and a read never leaves Ruby. Lich updates
its state before a script sees the line (VERIFIED, `inventory/13` §1.1); delivering the state's
change ahead of the line in the stream keeps that.

## 4. Hydra speak: the contract

Versioned from its first release, as `crates/cena-ui/WIRE.md` is (`plan/38` §8). Every request
names a character; a runner's default is its own.

### 4.1 The stream (pushed, one per character, in cursor order)

| Event | Carries | Serves |
|---|---|---|
| `line` | cursor, stream, text, and the line's **links** (exist id, noun, text), **bold spans** and **preset**, as data | waitfor, get, match*, fput's replies, and most raw-XML uses (§6.2) |
| `sent` | cursor, the line, its origin | fput's starting point; nobody's else |
| `state` | cursor, what changed | the local copy (§4.2) |
| `prompt` | cursor, the game's time | `checkrt`'s server offset |
| `notice`, `lifecycle` | Hydra's own messages; reconnects with the new generation | `Script` flows across a reconnect |
| `lagged` | how far behind | never a silent loss (`SessionObserver` already reports `Lagged`) |

**Which lines** are Lich's script view (VERIFIED, `inventory/13` §1.1): main text, thoughts,
combat, familiar, death and logon lines; speech once, from its main copy; room text from the
main copy; **never the prompt**; not the inventory, bounty, society, spellfront, reserve or
speech-window copies. Hydra's line assembler already builds per-stream lines for the web page
(`crates/cena-ui/src/lines.rs`); the stream uses it with Lich's selection.

> **BUILT OTHERWISE** (step 1): the stream carries the model's own line
> (`crates/cena-model/src/line.rs`) on every stream, as the game sent it, before the sorter and
> the triggers, and the runner makes Lich's selection (`bridges/ruby/hydra/listener.rb`): which
> lines a script sees is its language's business, and a second bridge may want them all.

### 4.2 The local copy (reads that never leave Ruby)

The script projection: room (id, uid, title, exits, description with its links, the objects,
creatures and players with their ids and statuses), hands, worn items, containers, vitals,
stance, encumbrance, mind, every status as `StatusInfo` keeps it (unknown kept unknown), effects
with their time left, the prepared spell, roundtime and cast-roundtime ends, targeting, group,
known spells, skills, stats, society, bounty, experience and currency. `plan/35` §5's agent
projection is its core.

MEASURED against the model (`inventory/13` §2.1): of the 1,851 file-uses of Lich's model calls,
**79.9% already exist in Hydra, 20.0% are partial, 0.1% missing** (the familiar's room, one
script). The partials are shape: Lich's names for skills and body parts, minutes against seconds,
`XMLData.game`'s codes, the room title's form. The bridge translates those, from one table
(`lich_short` and `squash` already do some of it, privately, in
`crates/cena-model/src/state/spell_time.rs:177-187`).

### 4.3 Requests (round trips)

| Request | Serves |
|---|---|
| `send(line)` → the `Sent` cursor, or a refusal | put, fput, multifput, move, dothis, dothistimeout |
| `quiet(line)` → the report's lines | `Lich::Util.issue_command`, `quiet_command` (`send_quietly` exists, `round_trip.rs:97`) |
| `hydra(command)` → an operation id; `operation(id)`, `control(id)` | a built-in started by name (§7), `do_client`; `Script.run` waits for the operation to end (`plan/35` §6's ids) |
| `say(text, style, window?, link?, mono?)` | echo, respond, `Lich::Messaging` (§5) |
| `map.room(id)`, `map.uid(uid)`, `map.path(from, to)`, `map.nearest(tag or title)`, `map.rooms(page)` | Room, Map, `find_nearest_by_tag` (`plan/38` §6d) |
| `spell(n)` | `Spell[n]`'s table row, its durations and costs evaluated for the character |
| `history(n, patterns?)` | `reget` (the model keeps 2,000 lines, `crates/cena-model/src/state/streams.rs:60`) |
| `store.get/set(scope, key)` | Settings, CharSettings, GameSettings, UserVars, Vars (§5) |
| `scripts.register(names)` | the `;name`s this runner answers (§5) |

### 4.4 What Hydra has to build for it

From `inventory/13` §2, the rows that are not EXISTS:

- **The line stream.** None exists: `SessionObserver` carries frames only.
- **The script projection and its changes.** `cena-agent` (`plan/35` §7) builds the core.
- **Script output with a window and a link.** `Notice` has four kinds and mono, and names both
  of these as not built (`crates/cena-session/src/notice.rs`).
- **`Room.current` held per session.** Today it is worked out inside travel
  (`crates/cena-behavior/src/travel/drive.rs:280`).
- **Lich answers that live in behaviors**, moved down or exposed: `affordable?`
  (`crates/cena-behavior/src/cast.rs:203`), nearest-by-tag (`travel/itinerary.rs:99`), the hands'
  store rules (`travel/hands.rs:73`).
- **The stores**, beside `crates/cena-session/src/settings_store.rs`.
- **`;name` forwarding** to the runner, and `;kill`, `;pause`, `;unpause`, `;list` over its
  scripts.

## 5. The runner: Lich's engine, in Ruby

**Reuse Lich's own Ruby for the engine** (PROPOSED). Lich-5 is **BSD 3-Clause** (VERIFIED,
`reference/lich-5/LICENSE`), so its script engine can be copied with the notice kept: `fput`'s
ladder, `waitfor` and the `match` family, `move`'s twenty remedies, `Script` (threads, pause
checkpoints, the kill and its cleanup order, `before_dying`, argument parsing), the Settings
proxies, `Lich::Messaging`. Only the data source changes. Where Lich's behavior is subtle, the
bridge is then faithful by construction rather than by re-derivation. `plan/35` §1 could not take
LAB's code for its GPL; this is the opposite case.

**Kept, because scripts depend on it** (`inventory/13` §1):

- the 400-line buffer that drops the oldest line; lines already buffered count for `waitfor`;
  string patterns go into the regex unescaped;
- pause takes effect at the next API call, kill at once, then `die_with`, `before_dying`, hooks,
  in that order; `hide_me`, `no_kill_all`, `no_pause_all`, `silence_me`, `daemon_me`;
- the `;` commands (`;k ;ka ;kd ;p ;pa ;u ;ua ;l ;la ;force ;send ;e ;eq ;en`), and a bare
  `;name args` starting a script;
- **the `NilClass` patch**: under Lich `nil.anything` is nil and `nil.split` is `[]`
  (`reference/lich-5/lib/common/class_exts/nilclass.rb:9`). Scripts lean on it without knowing, and would crash
  without it;
- `waitforre` returning nil (`inventory/13` §1, the correction to `inventory/01`): faithful, not
  fixed.

**Dropped**: `.cmd`/`.wiz` scripts, trust, Lich's maintenance commands, and frontends other than
the one `$frontend` names (§10, question 7). Labels and `goto` (1 file in A, 48 in B) come along
if the engine is copied, at no cost.

**Stores stay Lich's** (§10, question 6; this replaces `plan/38` §11's "in Hydra"). The engine
is Lich's (question 4), so its Settings, CharSettings, UserVars and Vars write `lich.db3` as Lich
does: Marshal blobs keyed by the script's name and a scope (`inventory/13` §1.8), write-through.
The file lives in Hydra's data folder, copied once from the player's Lich data folder so scripts
keep their settings on day one. One runner per character is no obstacle: several Lich processes
already share one `lich.db3`. A second language's bridge would not read Ruby's Marshal; that is
its problem when it comes.

**Lich's environment** (G15, 115 files in A): `$lich_char` is the character's symbol,
`XMLData.game` is Lich's code for the instance (`GSIV`, `GSPlat`), `LICH_VERSION` a version the
scripts' `required: Lich >=` headers accept.

## 6. The two things only Hydra decides

MEASURED, for a runner that is ordinary Ruby (`inventory/13` §3.3): Gtk, the operating system, the
network and `eval` all run as under Lich. What Hydra alone must answer is hooks and raw XML.

| | A: elanthia-online, 236 | B: the old repository, 2,129 |
|---|---:|---:|
| runs with read-only lines and no raw XML | **147 (62%)** | **1,659 (78%)** |
| ...and with display and input hooks through the runner (§6.1) | **179 (76%)** | **1,859 (87%)** |
| needs raw XML or Lich's internals (§6.2) | 57 (24%) | 270 (13%) |

The scripts that do not run are the big ones: by lines, 49% of A and 35% of B are in scripts that
need raw XML. bigshot, eloot and loottracker are among them, and Hydra has those built in (§7).

### 6.1 Hooks

MEASURED (`inventory/13` §3.2): of A's 83 display hooks, 64 rewrite (41 hide lines, 23 change
them); of its 29 input hooks, 26 rewrite. B: 411 of 546 and 94 of 113. `plan/38` §3 said lines
are read-only and hiding is M8's squelch. That leaves 52 of A's scripts (22%) and 346 of B's (16%)
needing a rewrite. Three ways:

- **(a) Read-only.** As `plan/38` has it; those scripts move their hiding to M8.
- **(b) Hooks through the runner** (PROPOSED). When a script on a character adds a display hook,
  each display line for that character passes through the runner, whose hooks answer keep,
  replace or hide within a deadline; past it, the line shows as it came. Typed lines likewise,
  for input hooks. Lich's rule is kept where it matters: hooks change only what is shown, after
  the model has the line (VERIFIED, `inventory/13` §1.6), so nothing a behavior or the model sees
  depends on a script. The cost is a line shown a little late on a hooked character, never lost.
- **(c) Declarative rules handed to Hydra**: no round trip, but Ruby's regexes become Rust's, and
  most hooks decide in code, not in a pattern.

Under any of them, **hooks see text, not XML**: a hook whose regex matches markup changes
(read-only hooks alone: A 9 of 18, B 37 of 135, a lower bound).

### 6.2 Raw XML

Settled, and not reopened here: nothing above the parser sees raw bytes (`CLAUDE.md`, *Parse
first*). 57 scripts in A and 270 in B read XML: `want_downstream_xml` (19/69), `status_tags`
(16/113), `$_SERVER*` (11/66), `strip_xml` and kin (2/32), Lich's internals (11/20).

**What they look for is mostly structure the parser already keeps.** MEASURED over the 46 scripts
in A that use `want_downstream_xml`, `status_tags` or `quiet_command_xml` (a grep of their
patterns for tag names): `<a exist=` 221 times, `<pushBold/>` about 175, `<preset` 54, then
`<prompt` 33, `<container` 34, `<progressBar` and `<dialogData` 26, `<d cmd=` 13. Links, bold and
presets are in the parser's runs; the rest are model facts. So the `line` event carries them as
data (§4.1), and such a script changes a regex over markup into a read of a field. The checker
(§1) points at each line.

**The fallback, if labelled data is not enough (AUTHOR, 2026-09-27):** *"Before faking tags I
would rather dupe the byte source and send it."* So a script that truly needs the markup gets a
**copy of the game's own bytes**, not markup re-rendered from the runs. Hydra still parses once,
and nothing inside Hydra sees raw bytes; the copy is handed out, like the session log. Faked tags
are not built.

## 7. The behaviors scripts call

MEASURED (`inventory/13` §3.4): scripts start other scripts by name, and the most-started are
Hydra's built-ins. When a script starts one, the runner starts the built-in through `hydra(...)`,
Lich's arguments translated, and `Script.run` waits for its operation to end.

| Started by name | Callers (A + B) | Hydra | What the mapping needs |
|---|---:|---|---|
| go2 | 512 | travel | little: room numbers are the same (Hydra's `RoomId` is the mapdb's, `crates/cena-map/src/room.rs:11-17`); go2's flags |
| bigshot | 117 | hunt | bigshot alone runs the character's default profile: Hydra needs a default-profile setting; `quick`, `bounty`, `head`, `tail`, `solo` map to `;hunt` forms |
| useherbs, eherbs | 54 | heal | the flags |
| waggle, ewaggle | 49 | waggle | |
| eloot | 24 | loot | **a standalone loot**: Hydra's loot runs inside the hunt's rest today; scripts call eloot to loot the room now, or `eloot sell` |
| foreach, sorter, multi | 22, 9, 4 | the same | |
| infomon | 11 | the model | `;infomon sync` becomes Hydra's sync; nothing else to run |
| spellactive | 2 | keep | |
| ebounty | 9 | — | Bounty is still to build (`plan/35` §9) |

**The runner's own**, not Hydra's: repository (100 callers: installing scripts), autostart (10:
scripts started at login), updater (6).

**Scripts that run as scripts**: lnet (49; its TLS connection is ordinary Ruby, and its `;chat`
is an input hook, §6.1), sloot, step2, tpick, poolparty, stand and the rest of the list.

## 8. Safety

- **A script is the player's own code on the player's machine**, as under Lich: the runner has
  Ruby's file and network access. Hydra does not sandbox it, and says so.
- **Its level is the player's to set** (`plan/35` §3), and a script never sets its own. The
  agent's denylist (drop, give, sell, destroy) cannot be a script's: eloot sells. PROPOSED, a
  level for scripts, per character, apart from the agent's.
- **Stopping a character stops its scripts.** The page lists them, beside the running behavior.

## 9. Measurements owed

None of these is measured; each is a guess until it is.

- A runner's memory per character: Ruby, the bridge, and N scripts. **MEASURED 2026-09-27, with
  no script running: 27.0 MiB working set, 58.8 MiB private** (`Get-Process` on the runner three
  seconds after it started), against Lich's 466 MB committed (`plan/38` §2a). With scripts:
  unmeasured.
- **A runner's start time.** The author, 2026-09-27: *"I'm not sure waiting until you try to run
  a script and taking 30-90 seconds for ruby to boot up is a good idea, but let's at least try
  it."* Most of Lich's start is loading its 100 MB map (`plan/38` §2a), which a runner never
  does. **MEASURED 2026-09-27: 338 ms** median from starting Ruby to the runner's first `listen`
  reaching a socket, 313-370 ms over six runs (Ruby 4.0.3, Windows 11, the author's machine;
  Lich's engine loaded, no script). How: `bridges/ruby/hydra/runner.rb` started with the
  contract's environment, `HYDRA_URL` naming a socket that accepts, timed to the first
  connection. So the runner starts on the first script, as the author asked.
- The local copy's update size and rate in combat.
- A send to its first reply line, against Lich's in-process path.
- A hooked line's display delay (§6.1), and the deadline to set.
- MCP notification delivery under load (`plan/38` §5 keeps a plain-JSON layer in reserve). **Not
  built that way**: a runner long-polls `listen` (§11, step 1), so what is owed is a line's time
  from the session to a script's buffer, in combat.

## 10. Questions for the author: ANSWERED 2026-09-27

1. **One runner per character** (§2), for `$globals`. AUTHOR: *"sure let's try it your way."*
2. **Hooks through the runner, with a deadline** (§6.1). AUTHOR: *"sure ask the script, with a
   time limit."*
3. **Raw XML: labelled data first** (§6.2). AUTHOR: *"I agree we try labelled data first. Before
   faking tags I would rather dupe the byte source and send it."* The fallback is a copy of the
   bytes; faked tags are not built.
4. **Reuse Lich's engine** (§5). AUTHOR: *"sure reuse it, we can always make a slimmer version
   later."*
5. **The built-in wins over a script of the same name** (§7), unless the player says otherwise.
   AUTHOR: *"yep."*
6. **Stores.** AUTHOR: *"lich script settings? would be saved in lich no? so data folder?"*
   Answered in §5: yes, `lich.db3` as Lich writes it, in Hydra's data folder, copied once from
   Lich's. (Confirm.)
7. **`$frontend` is `'stormfront'`, `LICH_VERSION` the engine's (5.21.0), `XMLData.game`
   Lich's codes** (§5). AUTHOR: *"yep."*
8. **A script level apart from the agent's**, allowing what a Lich script may do (§8). AUTHOR:
   *"yep."*
9. **Hydra starts the runner on a character's first script**, with the player's Ruby found as
   Saga finds it. AUTHOR: try it, measure it (§9); at login if it is slow.
10. **Ship the checker** (§1). AUTHOR: *"yes."*
11. **M7b, after the agent's connection** and after M6's live run. AUTHOR: *"got it."*
    **MOVED 2026-09-27**: the author asked *"So M7 is done ... that means we can work on the
    scripting bridge in a worktree no?"*, was told this answer put it after M6's live run and
    that M7's is not run either, and answered *"go"*. Branch `m7b-ruby`, worktree
    `G:\dev\Cena-m7b`, from `m7-agent`.

## 11. Steps, when scheduled

0. The decisions above.
1. The contract document; the stream's `line`, `sent`, `prompt` and `lagged`; `send` and `say`; a
   runner with Lich's engine; **the first script end to end: `trollspeak`** (`plan/38` §10).
   **BUILT 2026-09-27** (the author: *"go"*), in three commits:
   - **Hydra's half** (`crates/cena-session/src/script.rs`, `crates/cena-agent/src/scripts.rs`).
     The contract is `crates/cena-agent/SCRIPTS.md`, `hydra-script/1`. A runner has a listener of
     its own on loopback, a port the system chooses, opened with the first runner, and a token
     per runner kept only in memory and handed to it in its environment; the agent's listener
     is untouched. MCP without a session: each call a `tools/call` on its own. **The stream is
     long-polled, not pushed**: `listen` from a position, which lets go of what is before it, so
     a lost reply is answered again; simpler than notifications, and it batches under load. Its
     positions are its own, beside the session's cursor, because a line the player types for the
     runner (`typed`) has no cursor and must still arrive in order; a typed line is never let go
     for room. **What a script reads is the game's line** (`Event::Heard`, published only while a
     runner listens, before `;sorter` and the triggers): Lich's scripts read before its hooks, so
     a squelch hides a line from the player and never from a script waiting for it. `send` is a
     line as typed, as a trigger's is: Hydra's command when it starts with the symbol, else to
     the game at once as `Origin::Script`, ungated and unqueued, as Lich's `put`; it answers the
     cursor its `sent` carries (`Sent::Ok` now carries it). `say` is a notice, mono for
     `respond`. **No script level yet** (question 8): a runner acts on its own character only,
     which its player started it on; the level comes with the first request that names another.
   - **The runner** (`bridges/ruby`, README there): Lich's engine, sixteen files **byte for byte**
     as upstream's `236a9a2`, loaded by an edge of Hydra's that answers `Game.puts`, `respond`,
     `_respond` (its markup shown as text), a script's `$stdout`, `Lich.log` and the little of
     `XMLData` the engine reads; the player's typed commands go to Lich's own `ClientCommands`,
     then `Script.start`, as `do_client` does. Hydra carries the files in the binary and writes
     them out where they differ; the player's Ruby is found on the `PATH`, then under Lich's
     Windows install.
   - **The binary** (`crates/cena/src/scripts.rs`): `;name args` runs a script from `scripts` in
     Hydra's data folder (and its `custom` folders), found as Lich finds one; Lich's own words
     (`;k`, `;l`, `;p`, `;u`, `;force`, `;e`...) go to the runner, and those that act only on
     running scripts are answered while none runs. **Hydra's words come first**, after the check
     that tells a family still starting to wait, so `;go2` during the login is never `go2.lic`
     (question 5). A character leaving the table stops its runner. The runner's standard error
     goes to the terminal, and its last words to the player when it stops by itself.
   - **Tests**: the session's door (`crates/cena-session/tests/script_door.rs`), the listener
     over MCP (`crates/cena-agent/tests/scripts.rs`), a Lich script of Hydra's own end to end
     spawning Ruby (`crates/cena-agent/tests/runner.rs`), and the command line
     (`crates/cena/src/scripts.rs`). **Tillmen's `trollspeak.lic` runs unchanged** from the old
     repository's mirror: `say`, `echo` and its usage (the same file's Tier 2 test,
     `CENA_LICH_SCRIPTS`). Mutations: the runner reading viewer lines, a typed line dropped for
     room, a wrong sent cursor, main-window lines dropped by the runner, scripts heard before
     Hydra's starting words, and a runner left running at `close`: each turns a test red.
   - **Not yet**: the line's links, bold and preset as data (§4.1; with the raw-XML scripts);
     `fput` matching its replies by cursor (the cursor is answered, the engine does not use it
     yet); `XMLData` beyond the game and the name (step 2); stores (step 3); hooks (step 4); a
     runner stopped gently, its scripts killed as Lich kills them so their `before_dying` runs
     (today leaving the table ends the process). CI installs Ruby 4.0 for the runner's tests.
     Measured: §9.
2. The local copy and the reads (GameObj, Char, XMLData, `Room.current`, Spell, the check*
   family); **`wander`** (`plan/38` §10).
3. Stores and the import; `Script.run` of a built-in, **go2 first** (512 callers).
4. Hooks, as decided.
5. The checker over both collections, and the list of what runs published.
6. §9's measurements, written here.
