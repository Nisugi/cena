# 01 — Lich-5 Script Runtime and Script-Facing API (reference inventory for Cena)

Source root: `E:\Cena\reference\lich-5\lib` (read-only). Line numbers cited as `file:line` are from that tree at the time of inventory (2026-09-17).

Files covered: `global_defs.rb`, `deprecated.rb`, `common/script.rb`, `common/script_execution_guard.rb`, `common/script_death.rb`, `common/sharedbuffer.rb`, `common/buffer.rb`, `common/limitedarray.rb`, `common/hook_registry.rb`, `common/upstreamhook.rb`, `common/downstreamhook.rb`, `common/watchfor.rb`, `common/events.rb`, `common/watchable.rb`, `common/throttle.rb`, `common/client_commands.rb`, `common/client_commands/builtins.rb`, `common/client_input_dispatcher.rb`, `common/settings.rb`, `common/settings/*`, `common/db_store.rb`, `common/vars.rb`, `common/uservars.rb`, `sessionvars.rb`, `stash.rb`, `common/move.rb`, `messaging.rb`, `util/*`, `common/class_exts/*`, `attributes/*`, plus the seams in `games.rb` (`Game.puts`/`Game._puts`, downstream feed) and `main/main.rb` (buffer sizes, `$cmd_prefix`). Docs: `docs/script-execution-guard.md`, `docs/events.md`, `docs/runtime-io.md`.

Conventions used in tables:

* **reads** = which runtime state the function consumes. `XMLData.x` = parser state field; `GameObj.*` = object registry; `script.*` = per-script state on the calling `Script` instance; `$_SERVERBUFFER_` = global raw-line history.
* **blocking?** = `no` (returns immediately), `yes` (blocks the calling script thread until a condition), `sleep` (bounded sleep), `yes/∞` (may block forever if the condition never occurs).
* Ruby truthiness: only `nil` and `false` are falsy. Many functions return a `MatchData`/`Integer` (from `=~`) or `nil` as their "boolean"; these are marked "truthy-int".
* "pause checkpoint" = calls `Script.current`, which blocks while the calling script is paused (see §2.6).

---

## 0. Runtime topology (what a script sees)

```
 game socket ──> Game reader thread ──> SizedQueue ──> parser thread
                                                       │
     $_SERVERBUFFER_ (LimitedArray, 400) <─ raw line ───┤
     XMLData (Ox SAX) <──────────────────────────────────┤ process_xml_data
     Script.new_downstream_xml(raw)  ─> per-script downstream_buffer (if want_downstream_xml)
     strip_xml(raw) split "\r\n" ─> Script.new_downstream(line) ─> per-script downstream_buffer (if want_downstream)
                                                       │              + Watchfor triggers
     DownstreamHook.run(raw) ─> frontend                └─ (games.rb:1089-1095, 1150)

 client socket ──> ClientInputDispatcher (mutex) ──> do_client(string)
        UpstreamHook.run(string)  (nil = swallow)
        ";cmd"  -> ClientCommands.dispatch -> else Script.start(name, args)
        else    -> Game._puts(string); $_CLIENTBUFFER_.push
        Script.new_upstream(string) -> per-script upstream_buffer (if want_upstream)

 script thread: Game.puts(str) ─> "[name]>str" echoed via respond unless script.silent
                                  $_CLIENTBUFFER_.push; Game._puts("<c>"+str)   (games.rb:683-706)
                respond(str)   ─> Script.new_script_output(line) (scripts with want_script_output)
                                  Buffer.update(line, SCRIPT_OUTPUT); $_CLIENT_.puts_main_stream
```

Key globals (main/main.rb:56-60, 206-209): `$SEND_CHARACTER = '>'`, `$cmd_prefix = '<c>'`, `$clean_lich_char = ';'` (`,` for Genie), `$lich_char_regex = /,|;/`, `$_SERVERBUFFER_` (LimitedArray max 400), `$_CLIENTBUFFER_` (LimitedArray max 100), `$_IDLETIMESTAMP_` (set on every client line), `$_LASTUPSTREAM_`.

---

## 1. `global_defs.rb` — every top-level function

### 1.1 Script lifecycle and control

| name | signature | semantics | reads | blocking? | notes / edge cases |
|---|---|---|---|---|---|
| `start_script` | `(script_name, cli_vars = [], flags = {})` | `Script.start(name, cli_vars.join(' '), flags)`; `flags == true` → `{quiet: true}` | Script registry | no (pause checkpoint) | returns `Script` or `nil`. cli_vars array is joined with space then re-split by Script#initialize (quotes honoured). |
| `start_scripts` | `(*script_names)` | flatten, `start_script` each, `sleep 0.02` between | – | sleep | returns array |
| `force_start_script` | `(script_name, cli_vars = [], flags = {})` | sets `flags[:force] = true` then `start_script` | – | no | non-Hash flags replaced by `{}` |
| `start_scripts_if_available` | `(script_names)` | for each name: skip if `Script.running?` or `!Script.exists?`; `start_script`, `pause 0.05`, then poll up to 0.25s while it is running (waits for it to *finish initializing*; loop exits when it stops OR 0.25s elapse) | Script registry, SCRIPT_DIR | sleep ≤0.3s/script | returns nil |
| `before_dying` | `(&code)` | `Script.at_exit(&code)` — push proc on calling script's at_exit list | script | no | returns true / false (no block → warning). Order of execution: see §2.9 |
| `undo_before_dying` | `()` | `Script.clear_exit_procs` | script | no | |
| `abort!` | `()` | `Script.exit!` — clear at_exit procs then kill | script | no (kill is async) | Kernel#abort is `undef`'d at line 2316 |
| `stop_script` | `(*target_names)` | for each: find first `Script.list` entry whose name matches `/^#{name}/i` (prefix, case-insens.); if none → respond "…isn't running!"; if it is the *current* script → `exit` (Kernel exit → SystemExit → script ends); else `condemned.kill` and respond "'x' has been stopped by y" | Script registry | no | returns `false` if 0 killed else count. Regex built unescaped from name. |
| `running?` | `(*snames, exact_match: false)` | all names must match a running or hidden script; non-exact: `/^name$/i` or `/^name/i` prefix | Script registry | no | returns `true`/`false`; note *all* must match |
| `start_exec_script` | `(cmd_data, options = {})` | `ExecScript.start` | – | no | |
| `hide_me` | `()` | toggle `Script.current.hidden` | script | no | hidden scripts are omitted from `;list` and `Script.running`, not from kill-all |
| `no_kill_all` | `()` | toggle `script.no_kill_all` | script | no | |
| `no_pause_all` | `()` | toggle `script.no_pause_all` | script | no | |
| `hide_script` | `(*args)` | toggle `hidden` on each *exactly-named* running script | registry | no | |
| `pause_script` | `(*names)` | no names → pause self (returns Script.current); else pause each prefix match unless already paused | registry | self-pause blocks at next checkpoint | `fnd.nil?` check is after `fnd.paused` (would NoMethodError on nil… but NilClass#method_missing returns nil, see §8) |
| `unpause_script` | `(*names)` | unpause each prefix-matched paused script | registry | no | |
| `goto` | `(label)` | `Script.current.jump_label = label.to_s; raise JUMP` | script | no (non-local exit) | only meaningful in label (untrusted-binding) scripts; ExecScript prints "goto labels are not available in exec scripts." |
| `variable` | `()` | `Script.current.vars` | script | no | `vars[0]` is whole arg string, `vars[1..]` are tokens |
| `quiet_exit` | `()` | toggle `script.quiet` (suppresses "has exited" line) | script | no | |
| `setpriority` | `(val = nil)` | nil → `Thread.current.priority`; `val > 3` → echo error, return current; else set every thread in the current thread group to val | thread group | no | alias `priority?` |
| `die_with_me` | `(*vals)` | push names onto `script.die_with`; echo list | script | no | names are killed (via `Script.kill(name)`) during this script's cleanup |
| `lich_shutdown` | `()` | orderly Lich shutdown via `OrderlyShutdown.request_user_exit`, sends `<c>exit` to game after drain; calling script excluded from drain | – | yes | returns Result |
| `get_settings` | `(character_suffixes = [])` | YAML profile settings via `Lich::Common::SetupFiles` (lazy `$setupfiles`) | disk | no | returns OpenStruct (DR-style profiles) |
| `get_data` | `(type)` | `base-{type}.yaml` via SetupFiles | disk | no | |
| `parse_args` | `(defn, flex_args = false)` | `Lich::Common::ArgParser#parse_args` | script.vars | no | may `exit` with help |
| `display_args` | `(defn)` | ArgParser help | – | no | |

### 1.2 Per-script stream toggles (upstream / unique / xml / echo / silence)

Per-script flags (Script attr_accessors): `want_downstream` (default true), `want_downstream_xml` (false), `want_script_output` (false), `want_upstream` (false), `silent` (false), `no_echo` (false), `quiet`, `hidden`, `paused`, `no_pause_all`, `no_kill_all`, `ignore_pause`.

| name | signature | semantics | reads | blocking? | notes |
|---|---|---|---|---|---|
| `toggle_upstream` | `()` | toggle `want_upstream` (receive client→game lines into `upstream_buffer`) | script | no | returns new value; nil if no script |
| `toggle_unique` | `()` | toggle `want_downstream` (when false, script only receives `unique_buffer` lines sent via `send_to_script`/`;send to`) | script | no | Same effect as `i_stand_alone` but returns new value |
| `i_stand_alone` | `()` | toggle `want_downstream`; returns `!want_downstream` (i.e. true when now unique) | script | no | |
| `status_tags` | `(onoff = "none")` | `"on"` → want_downstream=false, want_downstream_xml=true (echo notice); `"off"` → reverse; else toggle | script | no | alias `toggle_status`. XML mode delivers *raw* server lines (one chunk per read) instead of stripped text lines. |
| `silence_me` | `()` | toggle `script.silent` (suppresses `[name]>cmd` echo from `Game.puts`); `safe?` scripts refuse (echo warning, sleep 1, return true) | script | sleep 1 if safe | returns new value |
| `toggle_echo` | `()` | toggle `no_echo` (suppresses `echo`) | script | no | |
| `echo_on` / `echo_off` | `()` | set `no_echo` false / true | script | no | |
| `upstream_get` | `()` | if `want_upstream` false → echo hang warning, sleep 0.3, return false; else `script.upstream_gets` (blocks) | script.upstream_buffer | yes/∞ | returns line string |
| `upstream_get?` | `()` | non-blocking; false if not want_upstream; nil if empty | script | no | |
| `upstream_waitfor` | `(*strings)` | false if not want_upstream; loop `upstream_gets` until `line =~ /a|b|c/i`; returns line | script | yes/∞ | patterns joined unescaped → regex |
| `unique_get` | `()` | `script.unique_gets` (blocks until unique_buffer non-empty) | script | yes/∞ | |
| `unique_get?` | `()` | non-blocking nil-or-line | script | no | |
| `unique_waitfor` | `(*strings)` | loop `unique_gets` until `=~ /join|/` (case-SENSITIVE, unlike waitfor) | script | yes/∞ | |
| `send_to_script` | `(*values)` | `values[0]` = target name prefix (`/^x/i` across `Script.list`); rest pushed to target's `downstream_buffer` if it `want_downstream`, else `unique_buffer`; echo confirmation | registry | no | returns true/false |
| `unique_send_to_script` | `(*values)` | same but always to `unique_buffer` | registry | no | |
| `send_scripts` | `(*messages)` | `Script.new_downstream(msg)` for each — broadcast as if from game (also triggers watchfor) | – | no | alias `send_script`; returns true |

### 1.3 Output to the frontend

| name | signature | semantics | reads | blocking? | notes |
|---|---|---|---|---|---|
| `respond` | `(first = "", *messages)` | Arrays flattened; each item `.to_s.chomp` + `\r\n`; every line → `Script.new_script_output(line)` and `Buffer.update(line, SCRIPT_OUTPUT)`; then if `Frontend.supports_mono?` wraps in `<output class="mono"/>…<output class=""/>` with XML-entity encoding; if profanity → encode only; `$_CLIENT_.puts_main_stream(str)` if alive; also `detachable_clients_respond` | Frontend caps | no (async socket writer) | never raises (rescued, logged) |
| `_respond` | same | same but **no** mono wrapping / encoding (raw XML passthrough) | – | no | use for markup (`Lich::Messaging` uses it) |
| `echo` | `(*messages)` | empty → `respond` (blank line); else each → `respond("[custom/name: msg]")` unless `script.no_echo`; no script → `[(unknown script): msg]` | script | no | returns nil |
| `_echo` | same via `_respond` | | | | |
| `debug` | `(*args, &blk)` | only if `$LICH_DEBUG`: yield or `echo` | – | no | |
| `fb_to_sf`, `sf_to_wiz(line, bypass_multiline:)`, `strip_xml(line, type:)`, `monsterbold_start`, `monsterbold_end` | | thin wrappers over `Lich::Common::Markup` (frontend markup translation) | – | no | `strip_xml` may return nil; NilClass#split is patched to `[]`. `monsterbold_*` return `.dup` (mutable). |

### 1.4 Reading the downstream buffer / matching

All of these read the *calling script's* `downstream_buffer` via `Script#gets` / `gets?` (§2.10). `gets` returns `false` (after echo + 2s sleep) if the script has no downstream stream enabled (`want_downstream`, `want_downstream_xml`, `want_script_output` all false).

| name | signature | semantics | reads | blocking? | notes / return |
|---|---|---|---|---|---|
| `get` | `()` | `Script.current.gets` — pop next line, wait if empty | script buffer | yes/∞ | String |
| `get?` | `()` | `Script.current.gets?` — pop or nil | script buffer | no | String / nil / false |
| `clear` | `(_opt = 0)` | `script.clear` → `downstream_buffer.clear_snapshot` (returns and removes all buffered lines) | script buffer | no | returns Array of lines; false if no script |
| `wait` | `()` | `clear` then `gets` — wait for next *new* line | script buffer | yes/∞ | |
| `waitfor` | `(*strings)` | flatten; WizardScript special: single `'>'` → return `gets` (prompt); empty → echo, return false; loop `gets` until `line =~ /#{strings.join('|')}/i` | script buffer | yes/∞ | **strings are interpolated unescaped into a case-insensitive regex**; returns matching line |
| `waitforre` | `(regexp)` | must be Regexp (else echo, sleep 1, nil); loop until `regexp.match(gets)`; returns **MatchData** | script buffer | yes/∞ | callers use `.string` / `[1]` |
| `matchtimeout` | `(secs, *strings)` | non-numeric secs → echo, false; empty → echo, sleep 1, false; loop: `get?`; nil → sleep 0.1; `line =~ /join|/i` → return line; past end_time → return **false** | script buffer | ≤secs | timeout returns false (not nil) |
| `match` | `(label, string)` | two-arg form (label-script style): pushes onto script `match_stack_labels/strings` for later `matchwait`; if flattened count ≠ 2 → loops `gets`, returns `$~.to_s` (the matched substring) on first `=~ /string/` | script | no (2-arg) / yes (other) | 2-arg form returns Array (push result) |
| `matchwait` | `(*strings)` | with strings: regex from sources joined (Regexp args use `.source`), **case-sensitive**, returns matching line. With no strings: uses match-stack; on match finds the label by `strings.index(mdata.to_s)` or first string the line matches (`/str/i`), clears stack, `goto label` | script | yes/∞ | goto raises JUMP |
| `matchbefore` | `(*strings)` | loop gets until `=~ /join|/` (case-sens.), return `$\`` (pre-match text) | script | yes/∞ | |
| `matchafter` | `(*strings)` | same, return `$'` (post-match) | script | yes/∞ | |
| `matchboth` | `(*strings)` | same, return `[$\`, $']` | script | yes/∞ | |
| `matchfind` | `(*strings)` | regex = strings joined with `|`, each `?` replaced by `(.+)`, `/i`; loop `regex.match(gets)`; `captures.compact`; returns single capture string if <2 else Array | script | yes/∞ | unknown script → respond + `Thread.current.kill` |
| `matchfindword` | `(*strings)` | as matchfind with `?` → `([\w\d]+)` | script | yes/∞ | |
| `matchfindexact` | `(*strings)` | `?` → `(\b.+\b)`; finds first line with `line.slice(/join/)`; then for each pattern that matches, pushes `$1..$n` (via `eval("$#{n}")`) for the number of `?` in that pattern; returns single or compacted array | script | yes/∞ | empty → echo, sleep 1, false |
| `reget` | `(*lines, core: false)` | history = `$_SERVERBUFFER_.dup.join("\n")` (regetall caller adds `.history`, which is always `[]` for LimitedArray); unless script `want_downstream_xml` or core: strips `<pushStream id="spellfront|inv|bounty|society">…<popStream>`, `<stream id="Spells">…`, `<compDef|inv|component|right|left|spell|prompt>…</same>`, all tags, `&gt;`/`&lt;`; split lines, drop blanks; leading Numeric arg = last N lines; remaining args = `/join|/i` filter; returns Array or nil if empty | `$_SERVERBUFFER_` | no | no script and not core → false |
| `regetall` | `(*lines)` | `reget(*lines)` (history is empty so identical) | | no | |
| `selectput` | `(string, success, failure, timeout = nil)` | validates (raise ArgumentError); loops: `fput(string)`, `waitforre(/success|failure/i)`; if success matched → return `response.string` (the line); else yield line and repeat; optional timeout thread raises StandardError into caller → rescued → **nil** | script | yes | timeout → nil |

### 1.5 Sending commands

| name | signature | semantics | reads | blocking? | notes |
|---|---|---|---|---|---|
| `put` | `(*messages)` | `Game.puts(msg)` each: pause checkpoint, guard check, `$_CLIENTBUFFER_.push "[name]>cmd"`, `respond "[name]>cmd"` unless `script.silent`, `_puts "<c>cmd"` | script.silent | no (pause checkpoint) | returns last `$_LASTUPSTREAM_` string |
| `multifput` | `(*cmds)` | flatten.compact each `fput` | | yes | |
| `fput` | `(message, *waitingfor)` + trailing options Hash | see algorithm below | script buffer, indicators | yes | alias `forceput` |
| `dothis` | `(action, success_line)` | loop: `clear`, `put action`, read `get` lines: matches `success_line` (Regexp) → return line; handles wait/typeahead/stunned/unconscious/held/webbed/lullabye by waiting then re-sending | script buffer | yes/∞ | no timeout |
| `dothistimeout` | `(action, timeout, success_line, interrupt: nil)` | as dothis but `get?`+0.1s polls, returns **nil** at `end_time` (reset after each recovery) or when `interrupt.call` is truthy; `action` nil → don't send | script buffer | ≤timeout | |

**`fput` algorithm (global_defs.rb:1479-1639)** — options: `timeout` (default 60; 0 disables), `max_resends` (nil = unbounded), `interrupt` (callable), `resend_transient` (false), `failures` (`:false` | `:symbol`).

1. `clear`; `put(message)`; `timer = now`.
2. Loop: `string = get?`.
   * nil → if interrupted → fail `:interrupted`; if timeout>0 and elapsed>timeout → echo "fput: No game response for Ns to 'msg'", fail `:no_response`; `pause 0.1`; next.
   * reset timer on any line.
   * `/(?:\.\.\.wait |Wait )(?<wait_time>[0-9]+)/` → over cap? fail `:too_many_resends`; sleep wait_time (interruptible); `clear; put(message)`; next.
   * `/^You.+struggle.+stand/` → over cap? / interrupted?; `standing = true`; `clear; put('stand')`; next.
   * `/stunned|can't do that while|cannot seem|^(?!You rummage).*can't seem|don't seem|Sorry, you may only type ahead/` →
     * `dead?` → echo "You're dead...! You can't do that!", sleep 1, unshift line back, fail `:dead`.
     * `checkstunned` → wait while stunned (0.25s polls, interruptible); `checkwebbed` → same;
     * `/Sorry, you may only type ahead/` → wait 1s;
     * else if `resend_transient` → wait 0.25; else sleep 0.1, unshift line, fail `:refused`.
     * then over cap? → unshift, fail; else `clear; put(message)`; next.
   * `standing` (reply to 'stand' arrived) → `clear; put(message)`; next.
   * else: if no `waitingfor` → unshift line back onto buffer, **return the line**. Else if a `waitingfor` pattern matches (`/val/i`) → unshift, **return the pattern** (the arg, not the line); else over cap?/wait 1s; `clear; put(message)`; next.
3. Failure return: `false` unless `failures: :symbol` → `:no_response | :too_many_resends | :interrupted | :dead | :refused`.

Note the "unshift" — fput puts the consumed line back at the *front* of the script's downstream buffer so a following `waitfor` can still see it.

### 1.6 Timing / waiting

| name | signature | semantics | reads | blocking? | notes |
|---|---|---|---|---|---|
| `pause` | `(num = 1)` | `"5m"`/`"2h"`/`"1d"` suffix → minutes/hours/days; else `num.to_f` seconds; via `Script.execution_sleep` (guard-aware) | – | sleep | `pause` with no arg = 1s |
| `checkrt` | `()` | `max(0, XMLData.roundtime_end - now + XMLData.server_time_offset)` seconds | XMLData.roundtime_end, server_time_offset | no | Float |
| `checkcastrt` | `()` | same with `cast_roundtime_end` | | no | |
| `waitrt` | `()` | `wait_until { rt remaining > 0 }` (**waits for RT to START**, 0.25s polls) then sleep `checkrt` | XMLData | yes/∞ | dangerous: blocks until roundtime appears |
| `waitcastrt` | `()` | same for cast RT | | yes/∞ | |
| `waitrt?` | `(interrupt: nil, cap: nil)` | legacy (no opts): sleep `checkrt`, return `checkrt > 0` (whether RT *remains*). With opts: `had_rt = checkrt>0`; loop while checkrt>0: return had_rt on interrupt/cap; sleep min(checkrt, 0.1); return had_rt | XMLData | sleep | |
| `waitcastrt?` | `(interrupt:, cap:)` | legacy: if castrt>0 sleep it and return true else false; bounded contract like waitrt? | | sleep | |
| `wait_until` | `(announce = nil) { cond }` | set thread priority 0; if announce and cond false → respond(announce); loop: `script&.wait_while_paused!`; if cond → `wait_while_paused!` again, break; `sleep 0.25` | – | yes/∞ | ensure restores priority; pause-aware |
| `wait_while` | `(announce = nil) { cond }` | inverse | | yes/∞ | |
| `watchhealth` | `(value, theproc = nil, &block)` | spawns Thread: `wait_while { health(value) }` then call block | XMLData.health | no (thread) | uses deprecated `health` alias; thread is *not* registered as a script worker |
| `idle?` | `(time = 60)` | `Time.now - $_IDLETIMESTAMP_ >= time` | global | no | |
| `fix_injury_mode` | `()` | if `XMLData.injury_mode != 2` → `Game._puts '_injury 2'`, poll ≤7.5s | XMLData.injury_mode | sleep | |
| `timetest` | `(*contestants)` | run each callable 5000× and time | – | yes | |

### 1.7 Status indicators (all read `XMLData.indicator[...] == 'y'`, return true/false, non-blocking)

| name | indicator | aliases |
|---|---|---|
| `checkpoison` | `IconPOISONED` | `poisoned?` |
| `checkdisease` | `IconDISEASED` | `diseased?` |
| `checksitting` | `IconSITTING` | `sitting?` |
| `checkkneeling` | `IconKNEELING` | `kneeling?` |
| `checkstunned` | `IconSTUNNED` | `stunned?` |
| `checkbleeding` | `IconBLEEDING` | `bleeding?` |
| `checkgrouped` | `IconJOINED` | `joined?`, `checkjoined`, `group?` |
| `checkdead` | `IconDEAD` | `dead?` |
| `checkhidden` | `IconHIDDEN` | `hiding?`, `hidden?`, `hidden`, `checkhiding` |
| `checkinvisible` | `IconINVISIBLE` | `invisible?` |
| `checkwebbed` | `IconWEBBED` | `webbed?` |
| `checkprone` | `IconPRONE` | – |
| `checkstanding` | `IconSTANDING == 'y'` | `standing?` |
| `checknotstanding` | `IconSTANDING == 'n'` | – |
| `checkreallybleeding` | `checkbleeding && !(Spell[9909].active? || Spell[9905].active?)` | (`reallybleeding?` is aliased to `alias_deprecated`) |
| `muckled?` | GS: `Status.muckled?`; DR: `checkdead || checkstunned || checkwebbed` | – |
| `checkname(*strings)` | none → `XMLData.name`; else `XMLData.name =~ /^(?:a|b)/i` (truthy-int) | `myname?` |
| `checkbounty` | `XMLData.bounty_task` or nil | `bounty?` |

GS-only `Status` wrappers (each `fail`s with "toplevel X command not enabled in DR" outside GS): `checksleeping`/`sleeping?`, `checkbound`/`bound?`, `checksilenced`/`silenced?`, `checkcalmed`/`calmed?`, `checkcutthroat`/`cutthroat?` → `Status.sleeping?` etc.

### 1.8 Vitals, mind, stance, encumbrance

All read XMLData; `num` form returns `value >= num.to_i` (true/false). Many emit `Lich.deprecated(...)` pointing to `Char.*` (logged once per message).

| name | signature | returns | reads | deprecated→ |
|---|---|---|---|---|
| `checkmana` / `mana` / `mana?` | `(num=nil)` | mana or `mana >= num` | `XMLData.mana` | `Char.mana` |
| `maxmana` / `max_mana` | | `XMLData.max_mana` | | `Char.max_mana` |
| `percentmana` | `(num=nil)` | `(mana/max*100).to_i` (100 if max==0) or `>= num` | | `Char.percent_mana` |
| `checkhealth` / `health` / `health?` | `(num=nil)` | | `XMLData.health` | `Char.health` |
| `maxhealth` | | | `XMLData.max_health` | |
| `percenthealth` | `(num=nil)` | `(health/max*100).to_i` (no zero guard → NaN.to_i raises FloatDomainError if max_health 0) | | |
| `checkspirit` / `spirit` / `spirit?`, `maxspirit`, `percentspirit` | | as above (no zero guard) | `XMLData.spirit/max_spirit` | |
| `checkstamina` / `stamina` / `stamina?`, `maxstamina`, `percentstamina` | | (zero guard → 100) | `XMLData.stamina/max_stamina` | |
| `maxconcentration`, `percentconcentration(num=nil)` | | (zero guard → 100) | `XMLData.concentration/max_concentration` | not deprecated |
| `check_mind` | `(string=nil)` | nil → `XMLData.mind_text`; non-numeric string → `string =~ /mind_text/i` bool; 0..100 → `string.to_i <= XMLData.mind_value`; else echo error, sleep 1, false | XMLData.mind_text/mind_value | |
| `checkmind` / `mind?` | `(string=nil)` | nil → mind_text; string → bool match; 1..8 → index of mind_text in `['clear as a bell','fresh and clear','clear','muddled','becoming numbed','numbed','must rest','saturated']`+1, `string.to_i <= mind`; unknown text → echo, nil; else echo, sleep 1, false | | |
| `percentmind` | `(num=nil)` | `XMLData.mind_value` or `>= num` | | |
| `checkfried` / `fried?` | | `mind_text =~ /must rest|saturated/` → true/false | | |
| `checksaturated` / `saturated?` | | `/saturated/` | | |
| `checkstance` / `stance` / `stance?` | `(num=nil)` | nil → `XMLData.stance_text`; strings: `/off/i`→value==0, `/adv/i`→1..20, `/for/i`→21..40, `/neu/i`→41..60, `/gua/i`→61..80, `/def/i`→==100, else echo+nil; Integer or digit-string → `stance_value == num` | XMLData.stance_text/stance_value | `Char.stance` |
| `percentstance` | `(num=nil)` | `stance_value` or `>= num` | | |
| `checkencumbrance` / `encumbrance?` | `(string=nil)` | nil → `encumbrance_text`; Integer/digits → `string <= encumbrance_value`; else `string =~ /encumbrance_text/i` bool | | `Char.encumbrance` |
| `percentencumbrance` | `(num=nil)` | `encumbrance_value` or `num <= value` (note direction) | | |

### 1.9 Room, exits, movement

| name | signature | semantics | reads | blocking? | notes |
|---|---|---|---|---|---|
| `n ne e se s sw w nw u up down d o out` | `()` | return the long direction word (`'north'`…`'out'`) | – | no | note `u`/`up` → 'up', `d`/`down` → 'down' |
| `move` | `(dir='none', giveup_seconds=10, giveup_lines=30)` | `Lich::Common::Move.move` (§6.2) | see §6 | yes | tri-state true/false/nil |
| `multimove` | `(*dirs)` | `move(dir)` for each (flattened); ignores results | | yes | |
| `checkpaths` | `(dir="none")` | none: `false` if no exits else `XMLData.room_exits.map { SHORTDIR[x] }` (short names, e.g. 'n'); with dir: `room_exits.include?(dir) || include?(SHORTDIR[dir])` | XMLData.room_exits | no | SHORTDIR maps long→short |
| `reverse_direction` | `(dir)` | maps n↔s, ne↔sw, e↔w, se↔nw, up↔down, out→out, also long forms; unknown → echo, false | – | no | bug: `'u'`→'down' and `'d'`→'up' (returns long forms) |
| `walk` | `(*boundaries, &block)` | with block: loop `walk(*boundaries)` until block truthy, return its value. Else: if `$last_dir` set and room description matches a boundary → move back (`$last_dir`), set `$last_dir = reverse`, return `checknpcs`. Else pick random exit (excluding reverse of last unless only one), `$last_dir = reverse_direction(choice)`, move, return `checknpcs` | XMLData.room_exits/room_description, `$last_dir` global | yes | random wander |
| `run` | `()` | `loop { break unless walk }` — wander until a room has NPCs | | yes/∞ | |
| `checkarea` | `(*strings)` | room title first comma-segment without `[`; or `=~ /join|/i` | XMLData.room_title | no | |
| `checkroom` | `(*strings)` | `room_title.chomp` or match | | no | |
| `checkroomdescrip` / `roomdescription?` | `(*val)` | `room_description` or match | | no | |
| `outside?` / `checkoutside` | `()` | `room_exits_string =~ /Obvious paths:/` → true/false | XMLData.room_exits_string | no | |
| `checkfamarea`, `checkfampaths(dir)`, `checkfamroom`, `checkfamroomdescrip`, `checkfamnpcs(*strings)`, `checkfampcs(*strings)` | | familiar-window equivalents reading `XMLData.familiar_room_title / familiar_room_exits / familiar_room_description / familiar_npcs / familiar_pcs` | | no | `checkfamnpcs` returns last word of each npc; `checkfampcs` strips titles (Lord/Lady/Great/High/…) and extracts capitalised names; both return false when empty/no match |

### 1.10 Game objects: NPCs, PCs, hands, loot

| name | signature | semantics | reads | notes |
|---|---|---|---|---|
| `checkpcs` | `(*strings)` | pcs = `GameObj.pcs.map(&:noun)`; empty → nil (no args) / false (args); no args → Array of nouns; args → first pc noun found in `strings.join(' ') =~ /\bpc/i` (note: pc searched *inside the args string*) | GameObj.pcs | |
| `checknpcs` | `(*strings)` | same over `GameObj.npcs` | GameObj.npcs | |
| `count_npcs` | `()` | `checknpcs.length` (nil.length → nil via NilClass patch) | | |
| `checkright` / `righthand` / `righthand?` | `(*hand)` | nil if `GameObj.right_hand` nil or name 'Empty'/empty; no args → noun; else first arg matching `name =~ /arg/i` (returns the arg) | GameObj.right_hand | |
| `checkleft` / `lefthand` / `lefthand?` | `(*hand)` | same for left | | |
| `checkloot` | `()` | `GameObj.loot.map(&:noun)` | GameObj.loot | |
| `parse_list` | `(string)` | `String#split_as_list`: strips "You also see"/"In the … you see", splits on commas/" and " into item phrases | – | |

### 1.11 Spells

| name | signature | semantics | reads |
|---|---|---|---|
| `checkspell` / `active?` / `checkactive` | `(*spells)` | false if `Spell.active.empty?`; all `Spell[x].active?` must be true | Spell registry |
| `checkprep` / `prepped?` / `checkprepared` | `(spell=nil)` | nil → `XMLData.prepared_spell`; non-String → echo error, false; else `prepared_spell =~ /^spell/i` | XMLData.prepared_spell |
| `cast` | `(spell, target=nil, results_of_interest=nil)` | Spell object / number / name → `Spell#cast(target, results)`; else echo "cast: invalid spell", false | Spell |
| `noded_pulse` / `unnoded_pulse` | `()` | GS mana pulse estimate: `max_mana*25/100` (noded) or `*15/100` + `max(stat)/10 + min(stat)/20` where stats depend on `Stats.prof` (warrior/rogue/sorcerer → [smc, emc]; empath/bard → [smc, mmc]; wizard → [emc,0]; paladin/cleric/ranger → [smc,0]); DR → 0 | XMLData.max_mana, Stats, Skills |

### 1.12 Hands / stash wrappers (all call `waitrt?` first, then `Lich::Stash`)

| name | semantics |
|---|---|
| `empty_hands` | `Stash.stash_hands(both: true)` |
| `empty_hand` | if right hand holds something and (right arm/hand wounds+scars max <3, or left arm/hand ==3) → stash right, else stash left; skips entirely when a hand is already free with an unbroken limb (`Wounds`/`Scars` GS state) |
| `empty_right_hand` / `empty_left_hand` | `stash_hands(right: true)` / `(left: true)` |
| `fill_hands` | `Stash.equip_hands(both: true)` |
| `fill_hand` | `equip_hands()` (pops right stack first, then left) |
| `fill_right_hand` / `fill_left_hand` | `equip_hands(right: true)` / `(left: true)` |

### 1.13 Misc utilities

| name | signature | semantics |
|---|---|---|
| `dec2bin(n)` | | `"0" + 32-bit binary without leading zeros` |
| `bin2dec(n)` | | inverse |
| `sync_detachable_client_globals`, `detachable_clients_snapshot`, `detachable_client_count`, `detachable_client_primary?(c)`, `detachable_listener_connected(b)`, `detachable_client_register(c)`, `detachable_client_unregister(c)`, `detachable_clients_respond(str)`, `detachable_clients_close`, `detachable_client_send_init(c)`, `detachable_client_send_player_id(c)`, `handle_detachable_client(c)` | | internal: multi-frontend (`--detachable-client`) registry; `handle_detachable_client` is the per-client read loop (handles `SET_FRONTEND_PID`, user-exit dispatch, prefixes `$cmd_prefix`, calls `dispatch_client_input`) |
| `dispatch_client_input(str)` | | `ClientInputDispatcher.dispatch` (mutex) → `$_IDLETIMESTAMP_ = now`; `do_client(str)` |
| `do_client(client_string)` | | strip; `UpstreamHook.run` (nil → return); if `/^(?:<c>)?[;,](.+)$/` → `ClientCommands.dispatch(cmd)` else `Script.start(name[, args])`; else if `$offline_mode` respond; else (`\egbbk` → `bbs`) `Game._puts`; `$_CLIENTBUFFER_.push`; finally `Script.new_upstream(client_string)` (always, incl. `;` commands) |
| `report_errors(&block)` | | run block; rescue every error class → respond "--- Lich: error: …" + log; SystemExit → nil |
| `alias_deprecated` | | echo "The alias command you're attempting to use is deprecated." |

### 1.14 Alias table (global_defs.rb:2316-2370)

`undef :abort`. Aliases (alias → target): `mana`,`mana?`→`checkmana`; `max_mana`→`maxmana`; `health`,`health?`→`checkhealth`; `spirit`,`spirit?`→`checkspirit`; `stamina`,`stamina?`→`checkstamina`; `stunned?`→`checkstunned`; `bleeding?`→`checkbleeding`; `reallybleeding?`→`alias_deprecated`; `poisoned?`→`checkpoison`; `diseased?`→`checkdisease`; `dead?`→`checkdead`; `hiding?`,`hidden?`,`hidden`,`checkhiding`→`checkhidden`; `invisible?`→`checkinvisible`; `standing?`→`checkstanding`; `kneeling?`→`checkkneeling`; `sitting?`→`checksitting`; `stance?`,`stance`→`checkstance`; `joined?`,`checkjoined`,`group?`→`checkgrouped`; `myname?`→`checkname`; `active?`,`checkactive`→`checkspell`; `righthand?`,`righthand`→`checkright`; `lefthand?`,`lefthand`→`checkleft`; `mind?`→`checkmind`; `forceput`→`fput`; `send_script`→`send_scripts`; `stop_scripts`,`kill_scripts`,`kill_script`→`stop_script`; `fried?`→`checkfried`; `saturated?`→`checksaturated`; `webbed?`→`checkwebbed`; `pause_scripts`→`pause_script`; `roomdescription?`→`checkroomdescrip`; `prepped?`,`checkprepared`→`checkprep`; `unpause_scripts`→`unpause_script`; `priority?`→`setpriority`; `checkoutside`→`outside?`; `toggle_status`→`status_tags`; `encumbrance?`→`checkencumbrance`; `bounty?`→`checkbounty`.

### 1.15 `deprecated.rb`

Globals: `$version = LICH_VERSION`, `$room_count = 0`, `$psinet = false`, `$stormfront = true`.

| name | semantics |
|---|---|
| `survivepoison?` / `survivedisease?` | echo "no XML for poison rate", return true |
| `fetchloot(userbagchoice = UserVars.lootsack)` | false if `GameObj.loot` empty; exclusion regex from `UserVars.excludeloot` (", "-split); if both hands full stow right-hand noun into `UserVars.lootsack`; for each loot not excluded: `fput "get noun"`, then `fput "put my noun in my bag"` if a hand holds something; restore stowed item |
| `take(*items)` | if both hands full stow right; `fput "take x"` / `"put my x in my lootsack"`; restore |
| `String#to_a` → `[self]`; `String#silent` → false; `String#split_as_list` | class extensions (see §8) |

---
