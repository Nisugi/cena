# Hydra's script contract — `hydra-script/1`

What a **script runner** may rely on when it runs a character's scripts against Hydra
(`plan/46`, M7b). A runner is a program in another language -- Ruby first, carrying Lich's own
script engine -- that runs one character's scripts, as Lich runs them, and reaches the character
through the tools below. Versioned as `CONTRACT.md` is: a field that changes meaning or goes
bumps the version; a new field, tool or event kind does not, because **a runner ignores what it
does not know**.

A runner is not an agent (`CONTRACT.md`). It has no level and no denylist: a script is the
player's own program, which the player started, and does what a Lich script does.

## Connecting

Hydra starts the runner and hands it, in its environment:

| Variable | Is |
|---|---|
| `HYDRA_URL` | the listener, `http://127.0.0.1:<port>/mcp`: loopback, a port the system chose |
| `HYDRA_TOKEN` | this runner's bearer token, for the `Authorization` header, **never a tool argument**. It names the character; it is kept only in Hydra's memory and opens nothing once the runner is dismissed |
| `HYDRA_CHARACTER` | the character's name |
| `HYDRA_GAME` | the game the character logged into, by its login code: `GS3` (GemStone IV), `GSX` (Platinum), `GST` (test), `GSF` (Shattered) |
| `HYDRA_SCRIPTS` | the folder the player's scripts are in |
| `HYDRA_DATA` | the folder a runner keeps its own data in (a Lich runner's `DATA_DIR`) |
| `HYDRA_SYMBOL` | the character's command symbol, `;` unless changed |

MCP's streamable HTTP, **without a session**: each request is a `tools/call` on its own, answered
as JSON; no `initialize` is needed, and a restart of either side needs no handshake again. Send
`Accept: application/json, text/event-stream`. Every tool answers JSON in its first text block.
A request with a token Hydra does not hold is answered `401`.

## `listen` `{ since?, timeout_ms? }`

What happened after position `since` (0 or absent: everything kept), waiting up to `timeout_ms`
(default 10000, at most 30000) for the first. Answers `{events, next, lagged, closed}`; ask next
from `next`. **Asking from a position lets go of everything at or before it**: a reply lost on
the way is answered again when you ask again from where you were.

Each event has `at`, its position, and a `kind`:

| Kind | Carries |
|---|---|
| `state` | `cursor`, `fields`: the **local copy** changed (below); each field that did, with its new value. The first event a runner hears is one, with every field |
| `line` | `cursor`, `stream` (`""` is the main window; `thoughts`, `speech`...), `text`: a line of game text **as the game sent it**, before the player's triggers and `;sorter`. A line the player squelched is still here |
| `sent` | `cursor`, `line`, `origin` (`manual`, `behavior`, `script`, `trigger`, `agent`): a line went out to the game. Yours are `script`, at the cursor `send` answered |
| `prompt` | `cursor`, `time` (the game's clock, epoch seconds, or `null`), `text` (`>`, `R>`...): the end of a chunk |
| `typed` | `line`: the player typed a command for the runner, without the symbol (`trollspeak say hi`, `k trollspeak`) |
| `input` | `asked`, `line`: while the runner has input hooks, a line the player typed, **as typed**, the symbol and all, before anything else sees it: answer it with `input` (below) |
| `lifecycle` | `cursor`, `state` (`ready`, `reconnecting`...), `generation` (advances on every reconnect) |
| `ended` | `run`, `work` (`completed`, `failed`, `interrupted`, `no_opportunity`, `unknown`), `reason` (the behavior's own word: `arrived`, `stopped`...), `left` (what it left undone, in words): a built-in `perform` started ended |
| `lagged` | `missed`: that many of the session's events were missed on the way here; a line a script waits for may be among them |

`cursor` is the session's own, which only increases, across reconnects too: **a line whose
cursor is above the one `send` answered came after your line**. Which lines: every finished line
of every stream, the prompt aside. Lich's scripts see fewer (not the inventory, bounty, society
or spell-window copies, `inventory/13` §1.1); choosing is the runner's.

**A chunk's `state` comes before its lines.** The game's text arrives in chunks, each closed by
a prompt; a copy of the character is taken at the prompt, and its changes are told **before**
the chunk's lines and its `prompt`, so a script woken by a line reads the state that line
produced, as under Lich. So a `state`'s `cursor` is later than the lines after it: it is the
snapshot's. Lines with no prompt after them go without a `state` after a quarter of a second.

**`lagged: true`** in the answer means events after `since` were let go for room (the last 4096
are kept; never a `typed` or an `input`). **`closed: true`**: the character's session ended, or the runner was dismissed;
nothing more will come.

## The local copy

What a script reads without asking. The `state` events keep it: apply each one's `fields` over
the last. Its fields are `CONTRACT.md`'s `state`, the agent's read -- `character`, `game`,
`lifecycle`, `generation`, `room` (the game's room number as `id`, `title`, `exits`,
`creatures`, `objects`, `players`), `hands`, `vitals`, `statuses`, `roundtime`,
`cast_roundtime`, `stance`, `encumbrance`, `mind` and their `_percent`s, `prepared_spell`,
`injuries`, `effects` -- with their rules (**unknown is never false**; a list the game has not
stated is `null`), less what only counts the clock (`clock`, `captured_unix_ms`, every
`seconds_left`: the `ends_at`s and the `prompt`s' `time` rebuild them), and more:

| Field | Is |
|---|---|
| `room_count` | how many times the character has arrived in a room: it changes on every move, even between rooms that read alike |
| `room_description` | the room's description, as text |
| `room_exits_line` | the exits line as the game words it, `Obvious paths: north, east.` |
| `map_room` | the map's own number for the room, when Hydra has a map and names the room **without guessing**; `null` otherwise |
| `target` | the id of the creature the character targets |
| `known_spells` | the numbers of the spells the character's spell list names; `null` until the game has sent the list |

A field a runner does not know is ignored, and new ones come without a new version.

## `room` `{ id }`

A room of Hydra's map, by the map's number (`map_room`): answers `{"map": true, "room": ...}`,
`room` `null` when there is none by that number, or `{"map": false}` when Hydra runs without a
map. The room is the map's own record: `id`; `uid` (the game's numbers for it); `title`,
`description` and `paths` (lists: a room has variants); `location`, `climate`, `terrain`, `tags`;
and `exits`, each `{to, kind, cost?, dirto?}` with how it is crossed: `cmd` (a command to
send), or `steps`, `routine`, `pass` or `unported` (Hydra's travel crosses it; a runner asks
Hydra to walk). `cost` is seconds when a number; absent, the exit is impassable.

## `spell` `{ number? , name? }`

A spell of Hydra's spell table, by number or by name (ignoring case). Answers `{"spell": ...}`,
`null` when there is no such spell: `number`, `name`, `type` (the table's, verbatim),
`availability` (`all`, `self-cast`, `group`...), `costs` (`mana`, `spirit`, `stamina`, `renew`)
and `minutes` (`self`, `target`), **evaluated for the character now**: a cost the table gives
as a formula is worked out from the character's skills and stats, and `null` when what it
needs is not known. A cost the spell does not have is `0`.

## `perform` `{ line }`

Start one of Hydra's built-in behaviors, as a Lich script starts one of Lich's by name
(`plan/46` §7): `line` is a Hydra command without its symbol, `go2 bank` or `go2 228`.
Answers `{"run": n, "line": ...}`, the command as Hydra keeps it, or `{"refused": why}`: Hydra's
behaviors are not ready yet (they are once the character has logged in), or Hydra does not run
that line for a script. **It goes on whether or not the runner waits**; `listen` hears its
`ended`, by `run`. It is the runner's, not an agent's: an agent does not see it among its
operations.

## `stop` `{ run }`

Stop a built-in this runner started, as the player's own stop does. Answers
`{"stopping": true}`, or `{"refused": why}` when it is not under way; `listen` hears its
`ended`.

## `send` `{ line }`

One line, as the player would type it. A line starting with the command symbol is **Hydra's
own command** and never reaches the game; any other goes to the game at once as the script's:
no roundtime check and no queue, as Lich's `put` writes straight to the game. Answers
`{"outcome": ...}`:

| `outcome` | Means |
|---|---|
| `sent` | it went to the game; `cursor` is its `sent` event's |
| `ran` | Hydra's command, and Hydra took it |
| `unknown` | it started with the symbol, and Hydra has no such command. Nobody was told: say so, naming the script |
| `refused` | the session would not send it; `why` says why (busy: not ready, or its queue full) |
| `lost` | no connection took it: the session is gone, or between connections |

`sent` claims only that the bytes went out, never what the game made of them: read the lines
after it.

## `say` `{ text, kind?, mono? }`

Show the player `text` (at most 20000 characters) as a script's `respond` and `echo` do:
split at each line break, every piece a line, so `""` is one blank line. `kind` colours it: `info` (absent), `warn`, `error`, `debug`. `mono`
keeps its columns, never re-wrapped, for a table. It never reaches the game. Answers
`{"said": true}`.

Text only: markup a script writes (`<pushBold/>`, `<preset>`) is not drawn. Hydra never puts
text the game did not send among the game's lines (`crates/cena-session/src/notice.rs`).

## Hooks

A runner's hooks change what the player is shown and what the player's typing becomes, as Lich's
`DownstreamHook` and `UpstreamHook` do (`plan/46` §6.1), **within a time limit**: half a second
(`cena_session::script::HOOK_DEADLINE`), past which the line goes as it came. Nothing else waits
on them: the game's line reaches the character's state, the log, the player's triggers' flags and
sends, and every `line` event **on time**; only its showing waits. The cost of a hook is a line
shown a little late, never one lost.

### `hooks` `{ display, input }`

Which hooks the runner has now: call it when the first of a kind comes and when the last goes.
Answers the two back.

- **`display`**: from now on, what the player is shown of each `line` you hear waits for your
  `shown` about it. Answer **every** `line`: one you leave waits out the limit.
- **`input`**: from now on, each line the player types comes as an `input` event, and waits for
  your `input` about it. Hydra's own lines (a `;multi`'s, a relayed `;to`'s) and a script's
  `send` never do, as a Lich script's `put` never meets its hooks.

### `shown` `{ lines: [{ cursor, text }] }`

What the player is shown of the lines heard, by each `line` event's `cursor`, as many in one call
as you have: `text` the line's own to show it as it came (its links and colours kept), other text
to show in its place (a line break makes more lines), `null` or `""` to hide it. Answers
`{"answered": n}`. The player is shown the lines **in the game's order**: an answered line waits
for an earlier one. Answering a line no longer waiting does nothing.

A line shown in another's place is text, as `say`'s: markup is not drawn. The player's triggers
answer it again (its colours, a substitution, a squelch), as a frontend's highlights answer what
Lich's hooks let through; a line the player's triggers hid stays hidden whatever you answer.

Hooks see the game's text, not its markup, as `line` carries it; a Lich hook whose pattern is
markup (`<prompt`, `<pushBold/>`) does not match.

### `input` `{ asked, line }`

What the player's line becomes, by the `input` event's `asked`: `line` to use in its place (the
same line to leave it), or `null` to swallow it: nothing is sent and nothing runs. One line, no
line breaks. What you answer goes on as the player's typing would, to Hydra's own commands, to
yours (`typed`), or to the game. Answers `{"late": false}`, or `{"late": true}` when the line
already went as typed.
