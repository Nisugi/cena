# 13 — The Ruby bridge's evidence: Lich's runtime, Lich against Hydra, and the scripts

What `plan/46` stands on, measured 2026-09-26/27. Three measurements, each made by a research
agent reading the sources and spot-checked afterwards (three claims re-read at their lines:
`waitforre`'s loop at `global_defs.rb:1402`, the 400-line buffer at `common/script.rb:1827`,
the `NilClass` patch at `common/class_exts/nilclass.rb:9`; all held).

Sources: `reference/lich-5` at `236a9a2` (LICH_VERSION 5.21.0, **BSD 3-Clause**, so the bridge
may reuse its Ruby with the notice kept); `reference/scripts` at `7fd0b97` (236 `.lic`);
`reference/lich_repo_mirror` at `d326ee9` (2,130 `.lic`); Cena's `crates/` at `54f9107`.

Paths in §1 are under `reference/lich-5/lib/`: `gd` is `global_defs.rb`, `sc` is
`common/script.rb`. VERIFIED means read at the line; VERIFIED-run means Lich's own code was run
under Ruby 4.0.3; INFERRED is marked. Script counts in §1 are `grep -lE` over the 236 scripts.

---

## 1. How Lich runs a script

**Two corrections to `inventory/01`** (which also stops after its §1): `waitforre` returns
**nil**, not a `MatchData` (`regobj = … until regobj` is a loop expression, whose value is nil;
gd:1399-1402, VERIFIED-run), so `selectput` (gd:493-499) never sees its success line. And hidden
scripts **are** exempt from `;ka`: `kill_all(force: false)` works over `Script.running`, which
excludes them (sc:1505-1514, 1635-1641); only `;kd` reaches them.

### 1.1 Game lines

- **State first, then scripts.** Per socket line (`games.rb:929`): `$_SERVERBUFFER_.push(raw)`
  (:964, max 400, `main/main.rb:207`) → XMLData and Infomon parse (:1068, :1085) →
  `Script.new_downstream_xml(raw)` (:1089) → `strip_xml(raw)`, split on `\r\n`, each non-empty
  line to `Script.new_downstream` (:1081-1095) → only then `DownstreamHook`, then the frontend
  (:983, :1149). **A script reading state after a line sees the state that line produced.**
- **What `strip_xml` removes** (`common/markup.rb:105-157`): whole `pushStream` blocks for
  spellfront, inv, bounty, society, reserve, speech and talk (:32); `<stream id="Spells">`;
  whole `compDef|inv|component|right|left|spell|prompt` elements (:35). Every other tag is
  dropped with its text kept; five entities decoded; blank lines dropped; a chunk with more
  pushStreams than popStreams is held until they balance (:148-157).
- **So a script sees** main text, thoughts, combat, familiar, death and logon lines as plain
  text; speech once, from its main copy (INFERRED, `plan/27f-model-api-streams.md:49-55`); room
  text from the main copy, not the room window's component (INFERRED). **Never the prompt.**
- **The buffer**: a `LimitedArray` capped at 400 (sc:1826-1827). Full, a push drops the
  **oldest** (`limitedarray.rb:46-55`); an `unshift` over the cap drops the newest (:57-63).
  Upstream and unique buffers cap at 200.
- **Flags**: `want_downstream` (on) gets stripped lines; `want_downstream_xml` (off) the raw
  chunk, raw first when both are on; `want_script_output` also gets every `respond` (sc:1585).
- **Reading**: `get` (gd:1429) blocks, no timeout; `get?` (:1433) nil when empty; `clear`
  (:1298) returns and empties; `wait` is `clear` then `get` (:1423). With every flag off, both
  warn, sleep 2 s and return false (sc:3074-3123).
- **`reget(n?, *pats)`** (gd:1437-1469) reads `$_SERVERBUFFER_`, not the script's buffer, and
  does not consume; an older, lighter strip; a number first means the last N lines; patterns
  joined `/…|…/i`; nil when nothing matches. `regetall` is the same, since
  `LimitedArray#history` always returns `[]` (`limitedarray.rb:95`).

### 1.2 Sending

- **`put`** (gd:1641) → `Game.puts` (`games.rb:683-706`): a pause checkpoint; echoes
  `[name]>cmd` unless silenced; writes `<c>cmd` under a mutex. **No queue, throttle or
  typeahead limit.**
- **`fput(msg, *waitingfor, opts)`** (gd:1479-1639): clear, put, then poll `get?` every 0.1 s;
  60 s with no line returns false. Each line:

  | line | action |
  |---|---|
  | `(?:\.\.\.wait \|Wait )(\d+)` | sleep N, clear, resend |
  | `^You.+struggle.+stand` | `stand`, then resend after its reply |
  | `stunned\|can't do that while\|cannot seem\|^(?!You rummage).*can't seem\|don't seem\|Sorry, you may only type ahead` | dead: false. Stun/web indicator: poll 0.25 s, resend. Typeahead: 1 s, resend. Otherwise **false**, the line unshifted, unless `resend_transient` |
  | anything, no `waitingfor` | unshift, return the **line** |
  | matches a `waitingfor` (/i) | unshift, return the **pattern** |
  | anything else, with `waitingfor` | 1 s, clear, **resend** |

  Options: `timeout`, `max_resends` (nil unbounded), `interrupt`, `resend_transient`,
  `failures: :symbol`. Never waits for a prompt or checks roundtime first. `/stunned/` also
  matches "The kobold is stunned!", so fput can fail on someone else's stun.
- **`multifput`** is fput per command (gd:1475). **`dothis(action, re)`** (:1890): clear, put,
  get until it matches, no timeout; retries on `...wait N` (sleeps N−0.5), typeahead, stun,
  web; waits up to 10 s when unconscious, lullabied or held. **`dothistimeout`** (:1949): the
  same, polling, the deadline reset on each retry; nil on timeout.
- **`move(dir, 10, 30)`** (`common/move.rb:167-446`): waitrt?, out-waits stun, clears (keeping
  the lines), puts. Success is `XMLData.room_count` rising, which counts
  `<popStream id='room'>` (`xmlparser.rb:577-580`). About 20 remedies (stand, unhide, retreat,
  open, climb/go, drag, empty hands, falls) through `fput(cmd, timeout: 3, max_resends: 1)`.
  Gives up after 10 s of silence or 30 unknown lines; restores the lines it consumed; returns
  true (moved), false (bad exit) or nil (blocked).

### 1.3 Waiting

All read the script's **own buffer**: lines that arrived before the call and were not cleared
still count. Only `wait`, fput, the dothis pair and move clear first. **Strings go into the
regex unescaped.**

| fn | gd: | match | returns |
|---|---|---|---|
| waitfor | 1405 | /i | the line; forever |
| waitforre | 1399 | as given | nil; forever |
| matchwait | 1376 | case-sensitive; bare: the match stack and goto | the line |
| matchtimeout(t, …) | 1324 | /i, polls 0.1 s | the line, or false |
| matchfind / matchfindword | 1695 / 1713 | /i; `?` → `(.+)` / `([\w\d]+)` | the capture(s) |
| matchbefore / matchafter | 1351 / 1359 | case-sensitive | the text before / after |
| checkrt | 292 | | max(0, rt_end − now + server offset); the offset set at each `<prompt time>` (`xmlparser.rb:653-655`) |
| waitrt? | 312 | | sleeps checkrt once, then whether RT remains; `interrupt:`/`cap:` poll 0.1 s; does not wait for RT to start |
| waitrt / waitcastrt | 282 / 287 | | wait for RT to **start**, then sleep it: can hang |
| pause(n) | 1273 | | a sleep; takes "5m", "2h", "1d"; not a pause checkpoint |
| wait_until / wait_while | 647 / 680 | | polls 0.25 s; honours pause; no timeout |

### 1.4 The engine

- **Starting**: `Script.start(name, args, {quiet:, force:})` (sc:96-260, 1134) returns at
  once; `Script.run` is start plus join (:1147; 88 scripts). Names resolve in `custom/`, then
  `custom/*/`, then `scripts/`, exact before prefix (:414-436). A second copy is refused
  "already running (use ;force)" (:155-160).
- **Variables** (sc:1808-1822): `vars[0]` the raw string, `vars[1..]` the tokens; double quotes
  group, even mid-token; `\"` is a literal quote; single quotes do not group. VERIFIED-run:
  `one "two three" fo"ur five" x\"y` → `[raw, "one", "two three", "four five", "x\"y"]`. A local
  `script` is bound for the body (:198).
- **The body** is `eval`ed in a new thread at priority 1 (:196-215). A column-0 `label:` switches
  it to label-by-label execution with `goto` (:169, 205-210). "Trusted" now means only "has no
  labels"; `;trust` says it is unavailable (`client_commands/builtins.rb:159-174`); `$SAFE` is
  nil. A first line of `quiet` or `hush` suppresses the active/exited messages (:1871).
- **`Script.current`** is the script whose thread group holds `Thread.current` (:1075-1105);
  threads a script spawns inherit it (INFERRED, Ruby's semantics).
- **Pause is cooperative** (:2866): the script blocks in `wait_while_paused!` (0.2 s, :2895) at
  its next `Script.current` call (get, put, echo, waitfor…) or inside wait_until/wait_while. A
  plain `sleep` runs on.
- **Kill is preemptive** (:1908). A cleanup thread: `Thread#kill` every worker (:2180-2182), kill
  the `die_with` scripts (:2194-2198), run `before_dying` in order (:2200-2204), clean up hooks,
  clear buffers, print "was killed" or "has exited" (:2698-2711). `Script.current` is still the
  dying script, so fput works inside `before_dying` (INFERRED). A normal finish is killed this
  way too (:239).
- **Flags** (gd:150-190, 505-516): `hide_me` drops a script from `Script.running` (so from
  `;list`, bare `;k`, `;ka`, `;pa`); `no_kill_all`; `no_pause_all` (from `;pa`/`;ua`);
  `silence_me` (no `[name]>` echo); `toggle_unique` (no game lines); `daemon_me` is hidden,
  no_kill_all and no_pause_all (sc:1060).
- **`Script.running?`** is exact (`/^n$/i`, :1434); the global `running?` matches a prefix and
  counts hidden scripts (gd:136); `Script.exists?` checks the file (:1526).
- **Exec scripts**: `;e`, `;eq`, `;en name` make `exec1`, `exec2`… (sc:3343-3456), no labels,
  no `Script.db`.

### 1.5 The player's `;` commands (`client_commands/builtins.rb`; `;` or `,`, `main.rb:60`)

| command | line | effect |
|---|---|---|
| `;k` | 23 | kill the newest visible script |
| `;k`/`;p`/`;u name` | 70 | exact, then prefix; running before hidden |
| `;ka` / `;kd` | 47 / 51 | all but hidden and no_kill_all / everything |
| `;p` `;u` / `;pa` `;ua` | 31-66 | the newest / all but no_pause_all |
| `;l` / `;la` | 91 | names, "(paused)"; `;la` with hidden |
| `;force name args` | 105 | a second copy |
| `;send msg` / `;send to name msg` | 115 | inject into every script as if from the game / into one |
| `;e` `;eq` `;en` | 147-151 | exec scripts |
| `;set x on\|off` | 207 | a `lich_settings` row |
| anything else | gd:2249-2256 | `Script.start(first word, rest)` |

Maintenance (`;trust ;hmr ;infomon ;sk ;display ;magic ;banks ;l5u ;help`) is Lich's own.

### 1.6 Hooks

- **`DownstreamHook.add(name, proc, persist:, priority: 0)`** (`common/hook_registry.rb:42`):
  every **raw XML chunk** (a copy) passes through, highest priority first, then insertion order
  (:128-134; `downstreamhook.rb:39-52`). nil hides the chunk from the frontend; a changed
  string replaces it. Hooks run **after** scripts and XMLData have the line, so they change
  only what is shown. A hook that raises is removed. On its script's death, `persist: false`
  hooks go, `true` stay, unset stay with a warning (:81-99).
- **`UpstreamHook`** (`upstreamhook.rb:39`): each player line before `;` dispatch (usually
  carrying `<c>`, `main.rb:855-857`); nil swallows it: not sent, dispatched or buffered
  (gd:2243-2245).
- **`toggle_upstream`** (gd:164): each player line that survives the hooks, `;` commands
  included, goes into a 200-line buffer (gd:2268); scripts' own sends never do. `upstream_get`
  polls 0.05 s, no timeout (sc:3126).
- **`Lich::Util.quiet_command(_xml)`, `issue_command`** (`util/util.rb:156-231`; 20 and 24
  scripts) hide a command's output with a DownstreamHook.

### 1.7 Output (to the main window; held while the game has a pushStream open,
`class_exts/synchronizedsocket.rb:107-127, 262-279`; copied to `want_script_output` scripts)

| fn | line | sends |
|---|---|---|
| `respond` | gd:1758 | `\r\n` lines, `& < >` encoded; `<output class="mono"/>…<output class=""/>` on wrayth, genie, saga (`frontend/*.rb:5`) |
| `_respond` | 1781 | the same, raw: markup passes through |
| `echo` | 212 | `respond("[name: msg]")` unless `no_echo` |
| `monsterbold_start/_end` | 2049 | `<pushBold/>` / `<popBold/>` |
| `Messaging.msg(type, msg)` | `messaging.rb:127` | bold for error/monster; thought preset for warn; whisper preset for info; speech preset for green/debug |
| `stream_window(msg, win)` | :21 | `<pushStream id="win" ifClosedStyle="watching"/>…`; GS: familiar, speech, thoughts, loot, voln |
| `make_cmd_link(text, cmd)` | :132 | `<d cmd='cmd'>text</d>` |
| `mono(msg)` | :138 | the mono pair |

### 1.8 Stores (all in `DATA_DIR/lich.db3`, Marshal blobs; tables `lich.rb:204-207`)

| API | key | saved |
|---|---|---|
| `Settings[k]` | script name + scope `":"`, `script_auto_settings` | every assignment and nested mutation (`settings_proxy.rb:253, 287`); `Settings.save` is a no-op |
| `CharSettings` / `GameSettings` | scope `"GAME:Name"` / `"GAME"` | as Settings |
| `Vars` / `UserVars` | a hash per `"GAME:Name"`, table `uservars` | every 300 s if changed (MD5), on `Vars.save`, at shutdown (`vars.rb:102, 172`); nil deletes |
| `DB_Store.read/save` | either table | at once (`db_store.rb:13, 22`) |
| `Script.db` / `open_file(ext)` | `DATA_DIR/<name>.db3` / `<name>.ext` | sc:305-340 |
| `Lich.db` | the raw handle; `Lich.foo` falls through to Vars | `lich.rb:179-198` |

### 1.9 What scripts silently depend on

- **Top-level names**: `include Lich::Common` (`lich.rbw:148`), `include Lich::Gemstone`
  (`main.rb:232`); `XMLData` top-level (`lich.rbw:150`). `XMLData.game` in 55 scripts,
  `Room.current` 88, `GameObj` 113.
- **The `NilClass` patch** (`class_exts/nilclass.rb`): `nil.anything` is nil, `nil.split` is
  `[]`, `nil + x` is x.
- `$frontend` compared to `'stormfront'` (18 scripts); `$fake_stormfront` never set;
  `LICH_VERSION` (22); `$clean_lich_char` (56); `DATA_DIR`, `SCRIPT_DIR`
  (`constants.rb:1-8`); `required: Lich >= x` headers (sc:562-631); `Script.current.name` in
  keys and paths (101); the `terminal-table` gem (20).

---

## 2. Each Lich call against Hydra

Crate prefixes: `m/` cena-model/src, `s/` cena-session/src, `map/` cena-map/src, `b/`
cena-behavior/src, `p/` cena-protocol/src. File counts are `inventory/07`'s (GS column for
globals; §3.2's combined counts for methods, the DR share about 0); † re-measured 2026-09-26
over the 236 scripts, quotes and comments stripped (±). Every mapping VERIFIED (the Rust item
opened) unless marked.

**Status**: EXISTS (typed, fed from the game, readable today) · PARTIAL · MISSING-MODEL ·
CONNECTION (an operation the connection must provide) · RUNNER (the Ruby runner's own) ·
NOT-PROVIDED.

### 2.1 Totals (224 entries, alias pairs merged; 4,153 file-uses, an upper bound)

| Status | calls | % | file-uses | % |
|---|---:|---:|---:|---:|
| EXISTS | 115 | 51.3 | 1,479 | 35.6 |
| PARTIAL | 20 | 8.9 | 371 | 8.9 |
| MISSING-MODEL | 1 | 0.4 | 1 | 0.0 |
| CONNECTION | 28 | 12.5 | 927 | 22.3 |
| RUNNER | 48 | 21.4 | 1,121 | 27.0 |
| NOT-PROVIDED | 12 | 5.4 | 254 | 6.1 |

**The model half is effectively complete**: of its 1,851 file-uses, 79.9% EXISTS, 20.0%
PARTIAL, 0.1% MISSING (the familiar's room, one file). The PARTIALs are shape: names, units,
`Room.current` not held per session, room-description links not pulled out as items,
`affordable?` living in the behavior crate. **The work is the connection and the runner**,
about half of all file-uses. No M7 code exists (`grep -ri "mcp\|jsonrpc" crates/` finds
nothing).

### 2.2 Output and sending

| Lich | files | Hydra | Status | Note |
|---|---|---|---|---|
| `respond` / `echo` | 152 / 135 | `SessionHandle::say(Notice)` s/command/handle.rs:331; `Notice` s/notice.rs:77 | CONNECTION | typed notice, no markup |
| `_respond` | 74 | — | NOT-PROVIDED | raw XML; Hydra refuses forged game markup (notice.rs:14-22); the bridge downgrades to `say` |
| `Messaging.msg` / `.mono` | 26† / 14† | `NoticeKind` notice.rs:52, `Body::Mono` :67 | CONNECTION | ~20 colour names fold into 4 kinds |
| `Messaging.make_cmd_link` / `stream_window` | 3† / 2† | — | CONNECTION | notice.rs names both as not built |
| `Messaging.msg_format` / `xml_encode`; `monsterbold` | 6† each | — | RUNNER | string helpers |
| `fput` | 114 | `send_gated(.., Origin::Script, .., Gate::Act)` s/command/round_trip.rs:67; `Origin::Script` s/command/verdict.rs:85 | CONNECTION | refusals are values; fput's resend ladder is not built |
| `put` / `multifput` | 53 / 26 | `send_now` handle.rs:466 | CONNECTION | Script jumps the queue like Manual, never preempts |
| `dothistimeout` / `dothis` | 83 / 4 | `send_and_await` round_trip.rs:51 | CONNECTION | `Matcher = fn(&Frame) -> bool` (s/queue.rs:86): a caller cannot pass patterns |
| `waitfor` `match` `matchwait` `matchtimeout` `wait` `get?` | 10/41/5/10/17/8 | `SessionObserver::subscribe` s/observation.rs:77 (frames only) | CONNECTION | no line stream |
| `reget` | 19 | `GameState::stream("")` m/state/streams.rs:119 (2,000 lines, :60) | CONNECTION | the history exists; it needs a query |
| `Lich::Util.issue_command` / `quiet_command_xml` | 22† / 14† | `send_quietly` round_trip.rs:97 | CONNECTION | returns lines, never XML |
| `do_client` | 8 | `SessionHandle::typed` handle.rs:316 | CONNECTION | Hydra's `;` commands |
| `move` / `walk` | 39 / 3 | `MoveFeedback` m/movement.rs:30; b/travel/mover.rs | CONNECTION | only inside travel |
| `cast`, `Spell[x].cast`/`force_*`/`putup` | 25 / 26† | `Casting::lines` b/cast.rs:69, `classify` :129 | CONNECTION | only inside behaviors |
| `empty_hands` / `fill_hands` | 32 / 29 | `store_commands` b/travel/hands.rs:73, `take_back` :100 | CONNECTION | store rules ported |
| `Group.check`, `StowList.check` | 4† / 5† | the model reads the replies | CONNECTION | the send is the operation |

### 2.3 The runner's own

`pause` `wait_while` `wait_until` (39/67/52); `before_dying` `undo_before_dying` (103/11);
`start_script` `stop_script` `running?` `pause_script` `unpause_script` `send_to_script`
`start_exec_script` `no_kill_all` `no_pause_all` `silence_me` `report_errors`
(33/4/42/23/5/5/6/7/6/22/3); `hide_me` (14; hides the **script**, gd:150); `variable`,
`Script.current` (.vars 97, .name 62) (43/112); `Script.running?` `run` `exists?` `start`
`self` `kill` `pause` `list` `unpause` `paused?` `running` `hidden`
(84/64/29/19/16/16/12/13/12/10/9/6); `goto`, `get_settings`, `parse_args` (3; 4/5);
`UserVars` `CharSettings` `Settings` `Vars` (53/43/17/10; s/settings_store.rs is where Hydra
keeps its own); `Log.out`, `Lich.log`, `$frontend` (17/4†/36†); `GameObj.new`,
`Spell.lock_cast`/`unlock_cast` (11; 5/5; the lock is cross-script, so per runner);
`Char.total_wound_severity` (3; defined by scripts that reopen `Char`, `puritan.lic:33`,
`spa.lic:93`).

### 2.4 Not provided

| Lich | files | Note |
|---|---|---|
| `DownstreamHook.add/remove` | 47 | plan/38 §3: read-only lines only; hiding is M8's squelch |
| `UpstreamHook.add/remove` | 29 | `Event::Sent` (s/actor/event.rs:50) is read-only; the Desk catches typed `;` lines |
| `status_tags`, `want_downstream_xml`, `$_SERVERBUFFER_` | 19 / 12† / 4† | no raw XML (plan/38 §6b) |
| `Gtk`; `Lich.db`; `XMLData.reset` | 51; 3†; 1† | |
| `GameObj.load_data`, `Spell.load` | 7 / 3 | the tables are compiled in (m/state/gameobj.rs:59, data/spells.tsv) |
| `Map.reload` | 4 | no runtime map changes (plan/38 §6d) |

### 2.5 The model: GameObj, XMLData, Char, Spells, Room, statuses

| Lich | files | Hydra | Status | Note |
|---|---|---|---|---|
| `GameObj.right_hand`/`left_hand`; `checkright`/`checkleft` | 64/56; 30/30 | `Hand` m/state/hands.rs:37; `Hand::noun` :103 | EXISTS | Unknown / Empty / Holding{id, noun, name} |
| `GameObj.npcs` / `loot` / `pcs` | 40 / 38 / 28 | `room.creatures` m/state/room.rs:208, `creatures().in_room()` m/state/creatures.rs:161; `room.objects` :210; `room.players` :212, `PlayerStatus` :132 | EXISTS | creature ids are i64 |
| `GameObj.inv` / `containers` | 31 / 23 | `worn.items()` m/state/worn.rs:128; `inventory.containers()` m/state/inventory.rs:82 | EXISTS | None until listed; containers cleared on reconnect |
| `GameObj.room_desc` | 18 | `room.description: Runs` room.rs:181 | PARTIAL | links survive, never pulled out as items |
| `GameObj.targets` | 14 | `Creatures::targets` creatures.rs:168; `Targeting::ids` m/state/targeting.rs:101 | EXISTS | requires the hostile flag |
| `GameObj.type_data`/`sellable_data`, `#type`/`#sellable` | 6/3, 46†/6† | `gameobj::classify` gameobj.rs:243, `categories` :262 | EXISTS | |
| `#id #noun #name #contents #status` | — | `RoomItem` room.rs:60-114; `Inventory::container` :77; `CreatureInstance::statuses` m/state/creatures/instance.rs:307 | EXISTS/PARTIAL | a creature's status is typed flags, not Lich's string |
| `XMLData.game` | 51 | `character.instance` m/state/character.rs:271 | PARTIAL | raw `<app game=>`; Lich remaps to `GSIV`/`GSPlat` |
| `.name`; `.room_count` | 13; 8 | `character.name` :261; `GameState.arrivals` state.rs:141 | EXISTS | |
| `.room_title` / `.room_id` / `.room_exits` | 22 / 5 / 6 | `room.title` room.rs:175; `room.id` :163; `room.exits` :206 | PARTIAL | no brackets, may carry the number; None where Lich hashes; short exit tokens |
| `.room_exits_string`, `.room_description` | 3† / 5 | `room.component("room exits")` :255; `Runs::plain` | EXISTS | |
| `.injuries`; `.active_spells` | 10; 4 | `character.injuries` character.rs:221; `effects.in_category("Active Spells")` m/effects.rs:398 | EXISTS | |
| `.server_time`; `.current_target_id(s)`; `.prepared_spell`; `.next_level_*` | 3; 4; 3; 3 | `game_time_now` m/state/clock.rs:55; `Targeting::current` :107; `GameState.prepared` state.rs:185; character.rs:102-104 | EXISTS | |
| `.level`; `.bounty_task` | 3†; 1 | `experience.level` character.rs:85; `bounty.task()` m/state/bounty_status.rs:107 | PARTIAL | a verbatim string; the parsed task only |
| `.health/.mana/…/.max_*`, `.mind/stance/encumbrance_text` | 3†; 1† | m/state/vitals.rs:95-113; character.rs:98, 226, 230 | EXISTS | |
| `.familiar_room_*` | 1† | — | MISSING-MODEL | a room reader over the familiar stream |
| `Char.name`; vitals; percents; `prof`; `Stats.*` | 57; 4-9; 3-6; 15; 3-20 | `character.name`; `Vital`; `Identity` m/state/character/stats.rs:442; `Stat` :116 | EXISTS | points as Option; percent is the game's rounded `value=` |
| `Char.level`, `Stats.level` | 11 / 11 | `experience.level` | PARTIAL | a string |
| `Skills.<skill>` (46); `Spells.<circle>`; `Skills.to_bonus` | 24; 7; 6† | `SkillSet::ranks` skills.rs:549, `::circle` :555; `to_bonus` spellsong.rs:311 | EXISTS | Lich short names vs `SkillKind::key`; the private `lich_short`/`squash` in spell_time.rs:177-187 already map them |
| `Wounds.<part>` / `Scars.<part>` | 11 / 10 | `Body::rank` body.rs:163 | EXISTS | Lich's aliases (`rarm`, `nerves`) need a table |
| `Spell[...]`; `.known?`; `.active?` | 64; 33†; 32† | `spells::spell` m/spells.rs:576; `KnownSpells::knows` m/state/known_spells.rs:143; `Effects::active` :309 | EXISTS | known? from the spell list, not circle ranks |
| `.affordable?` | 32† | `cast::ready` b/cast.rs:203 | PARTIAL | in the behavior crate; lacks the 9699 / Overexerted / sigil rules |
| `.timeleft`; `Effects::Spells.time_left`; `.name/.num/.time_per` | 11†; 5†; 8†/3† | `Effects::remaining` :385; `Spell` spells.rs:223; `spell_minutes` spell_time.rs:41 | EXISTS | seconds vs Lich's minutes |
| `Spell.active`; `Effects::*.active?`, `.to_h` | 10; 3-9† | `in_category`; `active_named` :367 | PARTIAL/EXISTS | returns effects, not Spell objects |
| `Room.current` / `Map.current` | 133 / 8 | `room_of` b/travel/drive.rs:280 → `Map::locate` map/locate.rs:124 | PARTIAL | not held per session; refuses to guess |
| `Room.current.id/location/tags/uid/title` | 70†/19†/16†/7†/3† | `cena_map::Room` map/room.rs:47-106 | EXISTS | |
| `.path_to`, `Map.dijkstra`; `.find_nearest(_by_tag)` | 10†/13; 13†/6† | `Map::routes` map/route.rs:185, `path_to` :146; `Target::Nearest` route.rs:40, b/travel/itinerary.rs:99 | EXISTS; PARTIAL | the tag filter lives in the behavior |
| `.wayto/timeto`; `Room[id]`; `Map.ids_from_uid`; `Room.list`/`Map.list` | 5†/4†; 28; 15; 16/24 | `Exit` map/exit.rs:293; `Map::room` map.rs:125; `ids_for_uid` :144; `rooms` :150 | PARTIAL; EXISTS | crossings as data, not StringProc |
| `Society.*`; `Bounty.*`; `checkbounty` | 3-6†; 3; 7 | m/state/character/standing.rs:51, :58; bounty_status.rs:107-113 | EXISTS; PARTIAL | Lich returns the task's text |
| `CMan/Feat.available?/known?`; `Group.members/leader`; `StowList.*`; `Claim.mine?` | 3-5†; 4†; 3-5†; 3† | `psm_availability` psm/cost.rs:239; m/state/group.rs:579/586; containers.rs:539/561; m/state/claim.rs:260 | EXISTS | |
| `Lich::Util.silver_count` | 13† | `currency.silver` m/state/character/currency.rs:40 | PARTIAL | the last `wealth` only; Lich sends a quiet command per call |
| `waitrt?`/`checkrt`; `waitcastrt?`/`checkcastrt` | 78/11; 38/10 | clock.rs:156, :197 | EXISTS | the sleep is the runner's |
| `checkmana` … `percenthealth`, the vitals checks | 3-17 | vitals.rs:95-113 | EXISTS | |
| `checkmind`/`percentmind`/`fried?`/`saturated?`; `checkstance`/`checkencumbrance` | 2-11 | character.rs:98-100, 226-232 | EXISTS | |
| `checkprep` `checkpcs` `checknpcs` `checkpaths` `checkname` `outside?`; `checkroom`/`checkarea` | 2-16; 10/2 | `prepared`, room fields; `room.title` | EXISTS; PARTIAL | |
| `dead?` `stunned?` `webbed?` `hidden?` `invisible?` `kneeling?` `standing?` `sitting?` `checkpoison` `checkdisease` `bleeding?` `joined?` `cutthroat?` `sleeping?` `bound?` | 0-31 | `StatusInfo` m/status.rs:192-233; `known()` :164 | EXISTS | the bool accessor reads unknown as false; Lich also wants a Debuff for the text-derived three |
| `muckled?` | 10 | compose from status | PARTIAL | only the creature one exists (instance.rs:598) |

### 2.6 What the bridge smooths over

- **Unknown versus false**: `Hand::Unknown`; exits `None` vs `Some([])`; `saw_players()`;
  `active_in` is `Some(false)` only when the category was stated; `knows` None when the list
  was never sent; `hostile()`; `Vital.current`; `SkillSet::ranks` None before a table (Lich
  reads 0); `PsmAvailability`'s Options.
- **Ids**: Lich's are strings; Hydra's item, hand, group and room-item ids are strings
  (players negative), but creatures, targeting and `Gate::Act` use i64. A map room is
  `RoomId(u32)`; the game's number is `Uid(i64)` in the map, `Option<String>` in the model.
  Lich's MD5 room-id fallback is `None`.
- **Units**: effect and spell time left are integer seconds where Lich's `timeleft` is float
  minutes (`spell_minutes`, `spellsong_timeleft` are already minutes); roundtime is absolute
  server seconds on an extrapolated clock; `Vital.percent` is the game's rounded value where
  Lich truncates current/max (and answers 100 at max 0).
- **Names**: skill short names; printed circle names; the wire's body parts; short exits; the
  title's form; `game` codes; the level string.
- **Same answer, another source**: `known?`; text statuses need no confirming Debuff and clear
  on reconnect; `targets` needs hostile; `Room.current` is nil more often, since Hydra never
  guesses.
- **Taught by a command** (skills, stats, bounty, stow list, silver): only as fresh as the last
  command. Container contents clear on reconnect.
- **Sending**: `Origin::Script` has no authority token (a sequence needs a claimed one); a
  refusal is a value and nothing resends; send-and-wait cannot take patterns; output is 4
  kinds plus mono, with no link, window or markup.
- **One process, many characters**: every call names a character; Lich is one per process.
- **Stale doc found on the way**: `status.rs:152-157` says a reconnect clears the indicators;
  `state/reconnect.rs:351` keeps them.

---

## 3. The scripts: what each needs, and which start which

MEASURED by the tools beside this file (`13-ruby-bridge-evidence/tools/`):
`python census.py && python report.py` for the tables, `python retier.py` for §3.3. Collection
A is `reference/scripts` (236 files, 182,977 lines), B is `reference/lich_repo_mirror/lib`
(2,130 files, 850,548 lines).

**Method.** `rblex.py` removes comments, `=begin/=end`, `__END__`, strings, regexes, heredocs
and `%`-literals, keeping `#{}` code (about 80 of 66,025 regex literals read by hand; none was
a division, INFERRED). Names match on word boundaries, never after `.`, `::`, `:`, `@` or `$`,
and not when the file defines the name itself; `Lich::Common::`-style prefixes are stripped.
Not a Ruby parser: two B files do not lex (`jars.lic` is not Ruby, `wbread.lic` has a syntax
error). **DragonRealms**: A has none, B one (`expbrief.lic`); the mirror keeps only `gs`/`any`
scripts in `lib/` (`lichless_repo/lichless_repo.rb:470-482`), so the tables are GemStone: A 236,
B 2,129. Classification choices, INFERRED with their sources: `puts`/`print` are output (Lich
sets `$stdout = $_CLIENT_`, `lib/main/main.rb:741`); an undefined `Lich.<name>` is a store
(`Lich.method_missing` forwards to `Vars`, `lib/lich.rb:179-185`); file **reads** are not G12.

### 3.1 Files per capability group

| Group | A | % | B | % |
|---|---:|---:|---:|---:|
| G1 send | 131 | 55.5 | 1,456 | 68.4 |
| G2 read lines | 122 | 51.7 | 1,077 | 50.6 |
| G3 timing | 173 | 73.3 | 1,464 | 68.8 |
| G4 output | 211 | 89.4 | 1,530 | 71.9 |
| G5 room and objects | 125 | 53.0 | 872 | 41.0 |
| G6 the character | 146 | 61.9 | 1,014 | 47.6 |
| G7 map and travel | 103 | 43.6 | 645 | 30.3 |
| G8 the engine | 205 | 86.9 | 1,403 | 65.9 |
| G9 stores | 111 | 47.0 | 691 | 32.5 |
| G15 Lich's environment (`$frontend`, `$lich_char`, `LICH_VERSION`, `XMLData.game`) | 115 | 48.7 | 472 | 22.2 |
| G10 hooks | 64 | 27.1 | 429 | 20.2 |
| G11 Gtk | 51 | 21.6 | 169 | 7.9 |
| G12 files written, network, subprocess, sqlite | 60 | 25.4 | 206 | 9.7 |
| G13 raw XML, Lich internals | 57 | 24.2 | 270 | 12.7 |
| G14 eval, metaprogramming | 42 | 17.8 | 105 | 4.9 |

Within groups (A/B files): go2 started as a script 62/428; `Script.*` 159/586;
`before_dying` 104/439; start/kill/pause 52/562; `running?` 39/421; `$globals` shared between
scripts 32/260; `goto` 1/48; UserVars/Vars 65/432; Settings family 66/319; file writes 44/166;
network 18/45; subprocess 6/26; direct sqlite 8/17; `status_tags` 16/113;
`want_downstream_xml` 19/69; `$_SERVER*`/`$_CLIENT*` 11/66; `strip_xml` and kin 2/32; string
`eval` 6/37; `send` 37/71 (mostly ordinary dispatch, e.g. `Wounds.send(method)`,
`scripts/puritan.lic:35`). `Thread.new` 37/146; `Lich::Util.quiet_command_xml` 14/49.

### 3.2 Hooks: read, or rewrite

Every `Hook.add` traced to its proc (inline, a variable, `method(:x)`, a builder). A **rewrite**
returns nil anywhere, mutates or reassigns the line, returns another value, or ends in anything
but the line; **read-only** is none of those, the line last.

| Hooks | A down | A up | B down | B up |
|---|---:|---:|---:|---:|
| total | 83 | 29 | 546 | 113 |
| read-only | 18 | 3 | 135 | 19 |
| rewrite: squelch only (nil paths) | 41 | 21 | 264 | 74 |
| rewrite: changes the text | 23 | 2 | 146 | 19 |
| unclear (counted as rewrite) | 1 | 3 | 1 | 1 |

Files: A 11 read-only only, 53 with a rewrite; B 82 and 347. **Read-only hooks whose regexes
match XML markup**: A 9/18, B 37/135, a lower bound (regex literals only). Hand check: 21 of 22
sampled read-only verdicts and 10 of 10 rewrite verdicts agreed with the code (the miss:
`newfavortracker.lic:36`).

### 3.3 Which scripts run, for a runner that is ordinary Ruby

The census's own tiers (in `report.py`) count Gtk, the operating system, network and `eval` as
blockers, which is right for a Lua runtime (`inventory/07` §6) and wrong for `plan/38`'s runner:
in ordinary Ruby they run as they do under Lich (Gtk given the gem). `retier.py` counts only what
Hydra itself must answer:

| For the runner | A files | A lines | B files | B lines |
|---|---:|---:|---:|---:|
| runs with read-only lines and no raw XML | **147 (62.3%)** | 68,522 (37.4%) | **1,659 (77.9%)** | 465,309 (54.7%) |
| ... and with display and input hooks through the runner | **179 (75.8%)** | 93,486 (51.1%) | **1,859 (87.3%)** | 556,244 (65.4%) |
| needs raw XML or Lich's internals (G13) | 57 (24.2%) | 89,491 (48.9%) | 270 (12.7%) | 294,218 (34.6%) |
| rewrites display or input (G10) | 52 (22.0%) | 64,880 (35.5%) | 346 (16.3%) | 299,424 (35.2%) |
| needs the `gtk` gem (G11) | 51 (21.6%) | 95,382 (52.1%) | 169 (7.9%) | 354,819 (41.7%) |

G13's reasons, in files (A / B): `status_tags` 16/113; `want_downstream_xml` 19/69;
`$_SERVER*`/`$_CLIENT*` 11/66; `strip_xml`, `sf_to_wiz`, `fb_to_sf` 2/32; Lich internals 11/20;
Lich's internal globals 4/12; other `XMLData` fields 3/14; `Game.*`/frontend internals 3/1.

**The big scripts are the blocked ones**: by lines, bigshot, BlackArts, eloot and loottracker in
A, ocean-go2 (37,482) and Ashborne (32,527) in B; the largest that run are ebestiary (2,534,
A), wannabe (1,932, A), huntpro (19,160, B) and osa-map-plugin (9,299, B, read-only hooks).

### 3.4 Which scripts start which

A string-literal script name in `Script.run/start`, `start_script(s)`, `Script.exists?`,
`running?`, kill/pause/unpause/`send_to_script`, or `fput`/`put`/`multifput`/`do_client` of
`;name`; self-references excluded; `;repo` merged into repository (Lich resolves a unique
prefix, `lib/common/script.rb:432-434`), `;chat` into lnet (its UpstreamHook,
`scripts/lnet.lic:1263`). **Callers**: A 100 scripts (84 start one) over 56 targets; B 754 (626)
over 325. **Dynamic names**, not in the graph: A 259 call sites in 56 files, B 958 in 210. 75 of
A's 100 callers and 562 of B's 754 touch something Hydra has built in. B has no go2, bigshot or
eloot of its own: the mirror drops what Elanthia Online maintains (`sync_eol.sh`).

| # | started | A | B | Hydra has |
|---|---|---:|---:|---|
| 1 | go2 | 66 | 446 | travel |
| 2 | bigshot | 10 | 107 | hunt |
| 3 | repository | 11 | 89 | — |
| 4 | lnet | 4 | 45 | — |
| 5 | useherbs | 5 | 34 | heal |
| 6 | waggle (not in either collection) | 2 | 37 | waggle |
| 7 | sloot | 1 | 27 | — |
| 8 | eloot | 3 | 21 | loot |
| 9 | foreach | 4 | 18 | foreach |
| 10 | eherbs | 3 | 12 | heal |
| 11 | step2 | 0 | 14 | — |
| 12 | infomon | 3 | 8 | — (the model does its work) |
| 13 | ewaggle | 3 | 7 | waggle |
| 14 | autostart | 3 | 7 | — |
| 15 | ebounty | 1 | 8 | — |
| 16 | sorter | 1 | 8 | sorter |
| 17 | stand | 0 | 9 | — |
| 18 | tpick | 0 | 9 | — |
| 19 | ego2 | 1 | 7 | — |
| 20 | poolparty | 0 | 8 | — |
| 21-30 | child2, huntpro, wander, cure (in neither), updater (in neither), signore, holdhands, oleani-lib, shunt, tsquelch | 0-3 | 2-7 | — |
| 31-40 | bountyhunter, ecleanse, explorer, slootbeta, herbmaster, alias, osacrew, ecure, osacombat, closecontainers (in neither) | 0 | 5 | — |

Hydra's other built-ins are started rarely: multi 0/4, spellactive 0/2, loottracker 0/0,
spellcaster 0/0.

**Limits.** Counts are upper bounds (a name in dead code counts); hook verdicts are heuristic
(above); dynamic names are counted apart; A has grown from `inventory/07`'s 234 files to 236.
