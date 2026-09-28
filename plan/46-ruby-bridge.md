# 46 — The Ruby bridge: how a Lich script runs against Hydra

**Status: PROPOSED 2026-09-27, author asked for it; the eleven questions ANSWERED the same day
(§10).** **Steps 1 to 6 BUILT 2026-09-27** (§11), the author moving M7b ahead of M6's live
run (§10, question 11); the rest is not built. It
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
- **(b) Hooks through the runner** (ANSWERED, §10 question 2; **BUILT**, §11 step 4, with the
  showing held after `;sorter` and the triggers, not before them). When a script on a character
  adds a display hook, each display line for that character passes through the runner, whose
  hooks answer keep,
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

## 9. Measurements: MEASURED 2026-09-27 (§11 step 6)

**How.** `crates/cena-agent/tests/measure.rs`, three ignored tests that print rather than assert:
`cargo test -p cena-agent --test measure -- --ignored --nocapture --test-threads 1`. The combat
is a real hunt's, `crates/cena-behavior/tests/fixtures/smithy_kill.xml` (a pegasus fought and
killed: 28 chunks, 278 lines, 70 KB in 11 game seconds), each chunk the scripted game's answer to
a script's `next`, so it arrives as the game sends it: a chunk at a time, closed by its prompt. A
Ruby script (`tests/fixtures/measure.lic`) keeps when it sent and when each line reached it; the
test keeps when the session published each line, each line shown and each prompt, on the same
clock. Ruby 4.0.3, Windows 11, the author's machine. The ranges are over five runs of **a debug
build**, as every build here is (`CLAUDE.md`); one machine, not quiet, and the spread is its own
finding. **Then once in a release build**, the author asking for it (2026-09-27: *"yea do a
release build"*): `cargo test ... --release`, the same tests. Where the two differ, both are
given; the runner's Ruby is the same either way.

**What the release build settles**: the debug build's long tails were Hydra's model reading a
chunk. In release the session reads the heaviest combat chunk in **16 ms** (debug: 177-787 ms),
and a line reaches a script in combat in **15 ms median, 32 ms at the 95th percentile**, most of
it the bridge. The bridge's own cost, **13-16 ms for a command and its reply**, is the same in
both, so it is Ruby's and HTTP's, not Rust's: each call a fresh connection (`connection.rb`), a
thing to measure before changing if it ever matters.

- **A runner's start**: **0.94-0.95 s** median from starting Ruby to its first `listen`
  reaching a socket (two sets of six starts, run alone), the method of the first measurement
  below; 0.96-1.46 s in three sets run straight after the combat measurement, on a machine still
  busy with it. Step 5 had taken it to 1.6 s by loading what Lich loads before any script; those
  libraries now load when a script first names one (Ruby's autoload,
  `bridges/ruby/hydra/engine.rb`), and step 2's runner and today's, timed alike from a Ruby
  script in two rounds, start alike (step 2's 0.93 and 1.01 s, today's 0.93 and 1.06 s). So
  **the runner stays started on the first script** (§10, question 9). Of the second, Ruby itself
  is 0.25 s, then Sequel 0.21-0.23 s, Lich's `gameobj.rb` 0.18 s, `sqlite3` 0.13 s and `net/http`
  0.10 s.
  - *Before, kept*: 338 ms at step 1 (Lich's engine, no script: 313-370 ms over six runs), 885 ms
    at step 2 (`GameObj`, `ox`, `lich.rb`, the stores and `sequel` too), timed from a separate
    script the same way.
  - **With windows** (the gtk3 gem loaded, §10 question 12): **5.0-5.5 s** median (the release
    run's six, 4.3-6.3 s; the debug run's, up to 14 s on a busy machine). Gtk alone is 1.6 s
    warm in a bare Ruby and 8 s the first time after the machine starts, reading its libraries
    from disk. Release: 0.97 s without windows.
- **A runner's memory** (`Get-Process`): **39.3-39.7 MiB working set, 73.1-75.5 MiB private**
  with no script; **with windows, 159-192 MiB working set and 201-230 MiB private**, Gtk's own; 42.2-42.5 and 78.9-79.3 after the combat; 42.5-42.6 and 80.2-82.5 with a
  display-hook script running; **42.8-43.0 and 90.7-91.0 with twelve scripts running**, about 1
  MiB private a script. Against Lich's 466 MB committed a character (`plan/38` §2a): 25
  characters' runners are some 2 GB private, 25 Lichs some 11.6 GB committed (not the same
  measure, so an order, not a ratio). With windows, 25 runners are some 5-6 GB private.
  - *Before, kept*: 27.0 and 58.8 MiB at step 1; 38.8 and 72.8 at step 2; 47.6 and 79.6 when
    step 5 loaded Lich's libraries at start, which autoload undid.
- **The local copy's update size and rate in combat**: **12-13 `state` events over the 28
  chunks, 19.3 KB, about 690 bytes a chunk and 1.75 KB a game second**; the median event 0.6-1.7
  KB, the largest 3.3 KB. What changes most is the room (7 events: its creatures and objects go
  whole) and the effects (6). The lines themselves are 188 events and 21 KB. Negligible.
- **A send to its first reply line, against Lich's in-process path**: a script's `look`, answered
  with one line, **12.9-17.1 ms median under Hydra, 0.23-0.54 ms under Lich's own path**
  (`tests/fixtures/lichpath.rb`: Lich's engine in one process, a socket for the game, a reader
  thread handing each line to the scripts, without Lich's XML parse, so a little quicker than
  Lich). The difference is the bridge: an HTTP call for the send, then the line waiting for its
  prompt, the copy, `listen`'s answer and Ruby's reading of it. A script's command costs some
  15 ms more than under Lich, in a debug build.
- **A line's time from the session to a script, in combat** (what "MCP notification delivery
  under load" came to, since a runner long-polls): **median 18-202 ms, 95th percentile 180-623
  ms**, of which:
  - **the session reading the rest of the line's chunk** (a chunk's lines wait for its prompt,
    so its state goes first, §3): median 0.1-133 ms, 95th percentile 177-543 ms; the slowest
    chunk 177-787 ms from its first line to its prompt.
    This is Hydra's model reading 2.5 KB of combat in a debug build, and every viewer waits on
    it as a script does. **It is most of the time, and not the bridge's**;
  - **the bridge, from the prompt to the script: median 8.0-23.5 ms, 95th percentile 15.7-35.5
    ms** (one run 359 ms).
  So a `next` to its reply's first line is 42-120 ms median.
  - **Release**: a line **15.1 ms median, 31.7 ms at the 95th percentile**; its chunk's prompt
    1.0 ms median (16.5 ms at the 95th), the bridge 13.4 ms median (19.8 ms); a `next` to its
    reply's first line 17.1 ms median.
- **A hooked line's display delay, and the deadline to set**: with a display hook that changes
  nothing, **median 12.3-18.5 ms, 95th percentile 51-529 ms, the longest 330-564 ms**; with none,
  a line is shown 0.00 ms after the session publishes it (the longest 0.05 ms). A hooked line
  waits for its chunk's prompt as a script's does, then for the runner's answer (8-40 ms of it);
  what is long is the session reading the chunk. **The deadline stays 500 ms**: the runner's own
  answer is well inside it, and it was passed by 64 ms at most, while the session was still
  reading the chunk that held the line, since the deadline is kept by the session between frames.
  **Release**: median 14.6 ms, 95th percentile 16.3 ms, one line of 183 at 333 ms.
- **Not measured, and why**: the game's own round trip over the network, which a script pays
  under Lich too.

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
   Lich's. (Confirm.) **BUILT otherwise in one respect** (step 3): not the file but its three
   script tables, merged row by row, so Lich's login cache (`simu_game_entry`) never reaches
   Hydra's folder and a runner holding the file need not stop.
7. **`$frontend` is `'stormfront'`, `LICH_VERSION` the engine's (5.21.0), `XMLData.game`
   Lich's codes** (§5). AUTHOR: *"yep."*
8. **A script level apart from the agent's**, allowing what a Lich script may do (§8). AUTHOR:
   *"yep."*
9. **Hydra starts the runner on a character's first script**, with the player's Ruby found as
   Saga finds it. AUTHOR: try it, measure it (§9); at login if it is slow. **MEASURED**: under a
   second (§9), so on the first script.
10. **Ship the checker** (§1). AUTHOR: *"yes."* **BUILT otherwise in one respect** (step 5):
    not from the census's lexer but from Ruby's own parser, against the runner itself, since a
    player has Ruby and not Python, and a checker that reads the runner cannot drift from it.
11. **M7b, after the agent's connection** and after M6's live run. AUTHOR: *"got it."*
    **MOVED 2026-09-27**: the author asked *"So M7 is done ... that means we can work on the
    scripting bridge in a worktree no?"*, was told this answer put it after M6's live run and
    that M7's is not run either, and answered *"go"*. Branch `m7b-ruby`, worktree
    `G:\dev\Cena-m7b`, from `m7-agent`.
12. **Load Gtk in the runner?** (§6: Gtk runs in ordinary Ruby given the gem; step 5 found 16
    scripts of elanthia-online's and 55 of the old repository's stop only at a window.) AUTHOR,
    2026-09-27: *"sure for now we will load gtk, but we will probably not use gtk on release."*
    **BUILT**: Hydra says whether a runner may open windows (`HYDRA_WINDOWS`,
    `runner::Start::windows`; the binary says yes, `crates/cena/src/scripts.rs`), and a runner
    that may loads the gtk3 gem when the player has it, as Lich does, giving its main thread to
    Gtk's loop (`bridges/ruby/hydra/engine.rb`, `runner.rb`; Lich's `gtk.rb` and
    `gtk_compaction.rb` vendored). It costs a runner some 4 s more to start and 120-150 MiB more (§9), so
    a release that says no is a one-line change.

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
   **BUILT 2026-09-27** (the author: *"step 2!"*), in four commits:
   - **The local copy** (`crates/cena-agent/src/scripts/local.rs`, `watch.rs`): the agent's
     projection as its core, as this plan says, in Hydra's own names, so a second language's
     bridge is not bound to Lich's; plus the room count (what Lich's `move` watches), the
     room's description and exits line, the map's room, the target, and each player's noun.
     A copy is taken at each prompt and its changed fields told as a `state` event, the first
     whole. **The lines of a chunk are held until its prompt**, so the copy's changes go
     first: §3's *"delivering the state's change ahead of the line"*, kept. A chunk with no
     prompt goes after 250 ms.
   - **The reads** (`bridges/ruby/hydra/copy.rb`): Lich's own `GameObj`, `Char`,
     `constants.rb` and the check family, unchanged, over an `XMLData` answered from the copy by
     Lich's reader names. The Lich shapes are made in one place: the title bracketed, exits
     spelled out, statuses as `IconSTUNNED => y`, every body part present, effects as Lich's
     dialogs. What Hydra does not send yet is not answered, so a script reading it fails
     naming it.
   - **The map** (`bridges/ruby/hydra/map.rb`): the binary hands its map and travel's own
     `room_of` to the runners, so `Room.current` is the room travel would name, never a
     guess, and nil where Lich's map would pick the first that fits; `Room[id]` asks Hydra's
     map once per room (`room`). `wayto` is the command, or a callable that asks travel to walk
     a crossing Hydra ports; `timeto` is seconds for a constant or a Haste roundtime at full
     price, and a proc answering nil for a cost only travel can answer for the walker (a
     gate, a ladder, a price table), as Hydra takes a cost it cannot answer as impassable.
     `cena-agent` gains the edge to `cena-map`.
   - **The stores, from step 3** (`wander`'s first line is `CharSettings`): Lich's own
     `lich.rb`, settings, `CharSettings`, `GameSettings`, `Vars` and `UserVars`, unchanged,
     writing `lich.db3` in `HYDRA_DATA` (`lich` in Hydra's data folder). Found in building
     it: Lich makes its tables before loading its settings, or the settings' adapter makes
     `script_auto_settings` without the key its saves need, and every save fails. The import
     from the player's Lich folder stays in step 3. `Lich::Messaging` is answered in its
     colour's notice kind, beside a `Frontend` saying Hydra is Wrayth's family with no GSL.
   - **`Spell`, its reads** (`bridges/ruby/hydra/spell.rb`): the table's row asked of Hydra
     once (`spell`, by number or name), what a cast lasts and costs asked each time, evaluated
     for the character by Hydra's own evaluator (`GameState::spell_minutes`, `spell_cost`), so
     the runner carries neither Lich's effect list nor its Ruby formulas; `known?` from the
     spell list in the copy (`known_spells`), `active?`, `timeleft` and `Spell.active` from its
     effects. `affordable?` checks mana, spirit and stamina, not Lich's Monk and overexertion
     rules. **Not `cast`**, which acts: it says it is not answered yet.
   - **Tests** (`crates/cena-agent/tests/scripts_local.rs`, `runner.rs`): a stun's `state`
     before the line that stunned; a mapped room named and answered; a script reading a room
     the scripted game describes whole through Lich's own classes, Heroism known, up and priced
     among it; a script walking an exit
     of `Room.current` with Lich's `move` and keeping a setting that a **second runner
     process** finds in `lich.db3`. **Tillmen's `wander.lic` runs unchanged** over two scripted
     rooms: out of the Quiet Glade by the map, the kobold found, `target random`, stopped
     (Tier 2, `CENA_LICH_SCRIPTS`), five runs in a row. Mutations: a chunk's lines before its
     state; a runner ignoring `state`; the stores without `init_db` first (which a second run
     in the same process passed, Lich's settings cache hiding the file: hence the second
     process); every spell read as known.
   - **Not yet**: `Spell#cast`; Lich's `Effects`; `Stats`, `Skills` and Lich's `Infomon`; `GameObj`'s type data (`gameobj-data.xml`, which Hydra has only as
     its own TSV); the familiar's room; `Room#path_to` and `find_nearest`; a crossing's walk
     waits on the copy naming the room, which step 3's operation replaces. CI installs `ox`,
     `sqlite3` and `sequel`, as Lich's installer does.
3. ~~Stores~~ (built in step 2, for `wander`) and the import; `Script.run` of a built-in,
   **go2 first** (512 callers).
   **BUILT 2026-09-27** (the author: *"step 3!"*), in three commits:
   - **Hydra's half** (`crates/cena-session/src/script.rs`, `crates/cena-agent/src/scripts/`):
     the script door's `perform` starts a line through the binary's performer, exactly as an
     agent's `perform` does, but **not among the agent's operations**: an agent neither reads
     nor steers a walk a script began, and a bad end drops no agent level. The listener's
     `perform` answers a run's number, `stop` steers it, and the runner hears `ended`,
     **after a fresh copy** (the watcher takes one first), so a script that waited for a
     walk reads the room it arrived in. Its progress goes nowhere yet.
   - **The runner** (`bridges/ruby/hydra/builtins.rb`): `Script.start`, `Script.run`,
     `start_script` and a `;go2` a script has Lich run all start Hydra's travel when they
     name `go2`, whatever `go2.lic` the player has (question 5). It runs as a Lich exec
     script named `go2`, so `Script.run` waits for it, `running?` and `Script.exists?` see it,
     and `;k go2` or `stop_script` stop it and Hydra's walk with it (an `ensure` around the
     wait). Lich's go2 settings (`_disable_confirm_`, `--delay=`, `typeahead=`) are left
     behind; travel takes the destination alone. The map's crossings walk the same way.
   - **The import** (`crates/cena/src/scripts/import.rs`): `;scripts import <Lich folder>`
     merges Lich's `script_setting`, `script_auto_settings` and `uservars` rows into Hydra's
     `lich.db3` as they are (Lich's replacing a row Hydra has under the same key), copies
     the `.lic` and `.rb` scripts of `scripts` and its `custom` folders **never over one Hydra
     has**, and Lich's `gameobj-data.xml` when Hydra has none, so `GameObj#type` works.
     `;scripts` says where they are. A script already running keeps what it read.
   - **Tests**: a built-in started and ended over MCP with a stand-in performer, stopped
     mid-way, refused, and before the behaviors are ready; in the session, absent from the
     agent's operations; in Ruby, `Script.run('go2', '229 _disable_confirm_')` walks by a
     stand-in travel and reads the new room, `start_script('go2')` runs and exists, and
     `stop_script` stops the walk; the import's tables, scripts and item types, a login's
     cache left behind and Hydra's own script kept, and through the command line. **Tier 2:
     the author's own install imported** (`CENA_LICH_FOLDER=C:\Gemstone\lich-5`): 45
     settings rows, 32 scripts and the item types, into a folder the test then removes.
     Mutations: killing the script leaving travel walking; the import writing over Hydra's
     own script.
   - **Not yet**: the other built-ins of §7 (bigshot, eherbs, waggle, eloot, foreach,
     sorter, multi, infomon, spellactive); a run's progress read by a script; the performer's
     refusals are worded for an agent, which a script meets only on a line it never
     builds.
4. Hooks, as decided.
   **BUILT 2026-09-27** (the author: *"step 4"*), in three commits:
   - **Hydra's half** (`crates/cena-session/src/script.rs`, `crates/cena-session/src/actor/hooked.rs`;
     `crates/cena-agent/src/scripts/hooks.rs`). While a runner has **display hooks**, what a
     viewer is shown of each line -- after `;sorter` and the triggers, as it would have been
     published -- waits for the runner to answer the line it heard (`shown`, by the `line`
     event's cursor): kept as it came, links and colours and all; hidden; or text in its place,
     which the player's triggers answer again (painted, substituted, squelched), as a
     frontend's highlights answer what Lich's hooks let through. A line the triggers squelched
     stays squelched. **Only the showing waits**: the model, the log, the triggers' flags,
     attention and sends, and every script have the line on time, so nothing a behavior sees
     depends on a script (§6.1, Lich's rule). Lines are shown in the game's order; one not
     answered in 500 ms goes as it came, and everything held goes when the hooks go or the
     connection ends. While a runner has **input hooks**, each line the player types at a
     frontend (`SessionHandle::send_typed_at`, which the window and Despana now call) is asked
     first (`input`), before Hydra's commands or the game see it: kept, replaced or swallowed,
     and as typed past the deadline. Hydra's own lines on the manual path (`;multi`'s, a
     relayed `;to`, the sorter's) never meet them, as Lich's `put` never does: a hook turning a
     line into a `;multi` of itself would never end. A runner says which hooks it has
     (`hooks`); its dismissal takes them.
   - **The runner** (`bridges/ruby/hydra/hooks.rb`). Lich's own `DownstreamHook`,
     `UpstreamHook` and their `HookRegistry`, vendored byte for byte, so names, priorities, a
     raising hook removed, and `persist:` at a script's death are Lich's. Hydra is told as the
     first hook of a kind comes and the last goes: added, removed, or taken with a script's
     death, which Lich does before it says the script was killed. Each game line goes to the
     scripts, then through the display hooks as `text\r\n`, as a line of Lich's chunks ends; a
     batch's answers go back together before the next `listen`. The player's typing goes
     through the input hooks as `<c>line`, as a Wrayth frontend sends it. Markup a hook adds is
     not drawn (tags only, so a line's own `<3` stays).
   - **Tests**: in the session (`crates/cena-session/tests/script_hooks.rs`), a hooked line kept
     with its link, hidden, changed, and shown when answered rather than at the deadline; one
     unanswered shown as it came at the deadline, with an answered one behind it waiting; the
     hooks' going showing what they held at once; a trigger's flag set while its line waits, a
     changed line painted and squelched by the triggers, a squelched one staying squelched; the
     player's typing swallowed, replaced, or sent as typed after the deadline, and Hydra's own
     line never asked. Over MCP (`crates/cena-agent/tests/scripts.rs`), the same through
     `hooks`, `shown` and `input`, and nothing asked once dismissed. In Ruby
     (`crates/cena-agent/tests/runner.rs`), a Lich script's display hook hiding one line and
     changing another, its markup not drawn, its input hook sending `tt` as `target`, and both
     gone when it is killed. Mutations: the deadline ignored (the actor spins and the test
     hangs), lines never held, a change ignored, a squelch undone, typing not asked, the hooks'
     going or an answer not waking the actor, input hooks never installed, a dismissed runner's
     hooks kept; in Ruby, answers never sent, display hooks never told, input not asked, markup
     drawn. Each turns a test red.
   - **Not yet**: a hook's markup drawn (the `<pushBold/>` it adds, a `<pushStream>` that moves
     a line to a window); a hook matching across a chunk's lines, or on markup (`<prompt`),
     which sees text a line at a time (the checker, step 5, is to point at them); the
     deadline measured (§9); Lich's `quiet_command`, a hook of its own (`util.rb` is not
     loaded).
5. The checker over both collections, and the list of what runs published.
   **BUILT 2026-09-27** (the author: *"step 5"*), in three commits:
   - **The checker** (`bridges/ruby/hydra/check.rb`; `crates/cena-agent/src/scripts/checker.rs`
     runs it). Ruby reads the script with its own parser (Prism) as Lich runs it -- cut at each
     line that is a label alone, `script` in scope -- and every name it uses is judged against
     the runner itself: the runner split in two, `bridges/ruby/hydra/engine.rb` loading Lich's
     engine and Hydra's edges for both the runner and the checker, so what the checker finds
     defined is what a script finds. Listed by hand is only what the runner defines and does
     not answer as Lich does: the game's markup, and each method registered as not answered yet
     (`Hydra.unanswered`: `Spell#cast`). A finding's kind is `stops`, `markup`, `windows` or
     `differs` (a script's verdict is its worst), and it says whether it is Hydra's to answer
     or the script's own. What it cannot see into it does not judge: a method of an unknown
     receiver; names from a gem, a mixin, a script started or a script beside it named like
     the name; a use behind `defined?` or `respond_to?`, under a `rescue` modifier, or in a
     DSL's block.
   - **Found in building it, and changed in the runner**: scripts lean on what Lich loads
     before any script (`OpenStruct`, `YAML`, `Time.parse`, `Terminal::Table`...), which the
     runner now loads, each as it is found; and `HAVE_GTK`, now false, as a Lich started
     `--no-gtk`, so a script that asks goes its way without a window.
   - **`;scripts check <script>`** (`crates/cena/src/scripts/check.rs`): the script found as
     Lich finds one (now in Lich's order: `custom` first, a whole name before a prefix), and
     its verdict and findings said, each thing once at its first line.
   - **The list** (`inventory/14-what-runs.md`, a TSV a collection): **35% of elanthia-online's
     scripts and 59% of the old repository's run today**, against `inventory/13`'s ceilings of
     76% and 87%. The difference is the runner's to-do list, ordered there by how many scripts
     each answer lets run: the game's markup (§6.2), Lich's windows, `Spell#cast`,
     `Lich::Util`, then `Stats`, `Skills`, `Spells`, `Wounds`, `Scars`, `Effects`, `Society`
     and `XMLData`'s fields, which Hydra's model holds and the runner does not name yet. 23 and
     212 scripts stop on something of their own: a name defined nowhere, `File.exists?`
     (dropped in Ruby 3.2), a file Ruby cannot read.
   - **Tests**: the checker over a fixture with one line of each kind and lines it must not
     find; what a check says, grouped and capped; `;scripts check` through the command line;
     finding in Lich's order. Mutations, each red: no label cut, no `script` local, no guards,
     no rescue modifier, the script's own constants judged, `<c>` taken as markup, the not-yet
     registry unread, Lich's classes taken as the script's, a sibling script's names judged, a
     prefix before a whole name. Hand check: 30 random stop findings in the old repository all
     real; of the 1,254 that run, 18 mention a blocking name, each explained.
   - **Not yet**: a spell held in a variable (`sign.cast`), which the checker misses. Lich's
     windows, raised here as a question, were answered: loaded for now (§10, question 12).
6. §9's measurements, written here.
   **BUILT 2026-09-27** (the author: *"step 6"*): §9 has each, measured over a real hunt's combat
   replayed through the scripted game, by `crates/cena-agent/tests/measure.rs` (ignored; it
   prints). What it found beyond the numbers: loading Lich's libraries at start (step 5) had
   taken a runner's start from 0.9 to 1.6 s and its memory up 9 MiB, so they now load when a
   script first names one (`bridges/ruby/hydra/engine.rb`; `REXML` with its stream listener,
   `bridges/ruby/hydra/rexml.rb`); and most of a line's time to a script, in combat, is the
   session reading the rest of its chunk in a debug build, not the bridge.
   **And a bug, found by a test that failed now and then while measuring**: a script killed
   while it was starting one of Hydra's built-ins -- before the run's number came back -- left
   Hydra's walk going, and a stop sent from the killed script's own thread took seconds, and
   twice in some forty runs never came. Lich stops a script by killing its threads, so both
   calls are now made by a thread of the runner's own (`bridges/ruby/hydra/builtins.rb`,
   `Errands`), which stops a run whose script is gone as soon as its number comes. The test
   (`crates/cena-agent/tests/runner.rs`) kills a walk while it is still starting (`go2 slow`)
   and counts every walk started and stopped: red six times in six on the old code.
