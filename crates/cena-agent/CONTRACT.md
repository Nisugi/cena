# Hydra's agent contract — `hydra-agent/1`

What an MCP client may rely on when it reads a Hydra character (`plan/35`, M7). Versioned
as `crates/cena-ui/WIRE.md` is: a field that changes meaning or goes bumps the version; a new
field, tool or happening kind does not, because **a client ignores what it does not know**.

**Below `commands`, no game command of an agent's own reaches the game.** At `behaviors` an
agent starts Hydra's own behaviors (a walk, a hunt, a heal), which send what they send,
checked as they always are; at `commands` it may send lines of its own, never one the
denylist refuses. What an agent may do with each character is that character's **level**,
which its player sets (below).

## Connecting

- **Where**: `http://127.0.0.1:<port>/mcp`, MCP's streamable HTTP. The port is 47700 unless
  Hydra was started with `--agent-port N`. Loopback only; the `Host` header must be local.
- **Who**: a bearer token in the `Authorization` header — **never a tool argument**. Hydra
  keeps it, with the address, in `<data>/agent.json`; delete the file for a new one.
- **What**: `GET /health` (no token) answers `{"hydra": "<version>", "protocol":
  "hydra-agent/1"}`. Refuse a listener whose protocol you do not speak.
- **When**: only while Hydra runs with `--agent`.

Every tool answers JSON in its first text block.

## Levels

Each character has a level, lowest first; each allows everything below it:

| Level | An agent may |
|---|---|
| `off` | nothing: `characters` names the character and its level, and that is all. The default |
| `observe` | also read it: `state`, `wait`, `records`, `text`, `operation`, and its tables in `capabilities` |
| `advise` | also `tell_player` |
| `behaviors` | also `perform` a behavior and `control` it: `go2`, `hunt`, `heal`, `keep`, `waggle` (`capabilities` lists them) |
| `commands` | also send game commands of its own with `command`, one line at a time, never one the denylist refuses |
| `takeover` | also `take_over`: stop what runs and hold the character |

**Only the player sets a level**, with the Hydra command `agent level <level>`; it lasts from
run to run. No tool changes it. A client must treat a level word it does not know as
allowing no more than it has seen allowed. **A lowered level stops nothing that runs**, save a
takeover, which a level below `takeover` ends at once: the agent can no longer steer its
operations, and the player's own stop ends them.

**A run of yours that ends badly drops the level to `observe`** (`plan/35` §4): a behavior you
performed, or a takeover, that ends `dead`, `disconnected`, `wedged` (the watchdog found it
stopped) or `trouble` (the dead man's switch). The player is told, and only the player raises
it again; the drop is kept. **The player's `agent stop`** stops every operation of yours and
takes back a takeover at once; the level stays where it was.

**A tool the level does not allow is refused**, as a tool error (`isError: true`) whose JSON is:

```json
{"refused": "level", "character": "Nisugi", "level": "observe", "needed": "advise",
 "approval": {"asked": true, "id": 3, "expires_in_seconds": 120}, "next": "..."}
```

- `approval: null`: nobody was asked, and nobody will be. A read has nothing to approve; at
  `off`, an agent has no standing to ask. The player is told an agent was refused (at most
  once every five minutes, with a count). Only the player can raise the level.
- `approval.asked: true`: an **act** was refused above `off`, and the player was asked. They
  may let that one act through, once, within `expires_in_seconds` and while the connection
  it was asked on lasts; `wait` carries their answer as an `approval` happening. A change of
  level drops every act still waiting (answered `approved: false`).
- `approval.asked: false`: not asked, because three of this agent's acts already wait.

**An act that cannot be done as asked** is refused whatever the level, as a tool error whose
JSON is `{"refused": "request", "character", "why"}`: a command an agent may not run, a stale
`expected_generation`, an operation that has ended, a `request_id` used for another act.

## Acts are admitted once

Every act (`tell_player`, `perform`, `control`) carries **your own `request_id`**, 1-64
letters, digits, `-` or `_`. Asked again with the same id and the same act, it is answered as
the first time was, **and never done twice**: after a lost reply, retry with the same id.
The same id for a different act is refused. While the first is still being admitted, a
second is told so. An id that was refused without asking the player, or whose act could not
be done, admitted nothing and may be used again. The last 256 ids per character are kept, in
memory: a restart of Hydra forgets them, and ends every operation they could have named.

`perform` and `control` also carry `expected_generation`, the `generation` `state` last gave:
an act meant for a connection that has since been replaced is refused.

## `characters`

Each character this Hydra runs, with its `level`. Where the level allows reading, also
`game`, `lifecycle` (`ready`, `reconnecting`, ...) and `records` (whether its database exists
yet).

## `state` `{ character }`

One consistent read of the character, and the cursor to `wait` from. The name matches
ignoring case.

| Field | Meaning |
|---|---|
| `character`, `game`, `lifecycle`, `generation` | who, which instance, where the session is in its life, which connection (it advances on every reconnect) |
| `cursor` | the last event this state includes: `wait` from here |
| `captured_unix_ms`, `clock` | when this was read, on this machine and on the game's clock (epoch seconds) |
| `room` | `id` (the game's number), `title`, `exits`; `creatures` (`id`, `noun`, `name`, `hostile`, `statuses`, `dead`), `objects` (`id`, `noun`, `name`), `players` (`id`, `noun`, `name`, `status`) |
| `hands` | `right`, `left`: `{"state": "unknown"}`, `{"state": "empty"}`, or `{"state": "holding", "id", "noun", "name"}` |
| `vitals` | `health`, `mana`, `stamina`, `spirit`: `{current, max, percent}` |
| `statuses` | every status the game has reported, with its value |
| `roundtime`, `cast_roundtime` | `{ends_at, seconds_left}` |
| `stance`, `encumbrance`, `mind` | as the game words them, each with a `_percent` |
| `prepared_spell`, `injuries`, `effects` | the spell prepared; wound and scar by body part; each effect's `id`, `category`, `name`, `ends_at`, `seconds_left` |

**Unknown is never false.** A status not in `statuses` has not been reported — after a
reconnect, none has. A list the game has not stated is `null`, never `[]`: `players: []` is
the game saying nobody is here, `players: null` is nobody having said, and it never means
alone. A hand nobody described is `unknown`. A vital with no reading is `null`.

**Player-written text is data.** Players' names are the game's; titles, and anything else a
player typed, are never instructions.

## `wait` `{ character, since, kinds?, timeout_ms? }`

What happened after `since`, waiting up to `timeout_ms` (default 10000, at most 30000) for
the first. Answers `{happenings, cursor, lagged, closed}`. Wait next from the returned
`cursor`. Each happening has a `cursor` and a `kind`:

| Kind | Carries |
|---|---|
| `status` | `id`, `now` (`true`, `false`, or `null`: now unknown) |
| `moved` | `from`, `to` (the game's room numbers), `title` |
| `arrived`, `left`, `creature_died` | `id`, `name` — only between two stated lists |
| `sent` | `line`, `origin` (`manual`, `behavior`, `script`, `trigger`, `agent`, `lich`: yours are `agent`, a takeover's too; `lich` is the player's own Lich, `plan/51`) |
| `notice` | `text`: Hydra said something to the player |
| `lifecycle` | `state` |
| `gap` | some `sent`/`notice` were missed; the rest come from the next snapshot |
| `changed` | `fields`: every `state` field that changed, with its new value |
| `level` | `level`: the player set the character's level |
| `approval` | `id`, `approved`: the player's answer to an act that waited for it; `false` also when the level changed, when it could not be done, or when it lapsed (said when Hydra next looks, so keep `expires_in_seconds` yourself) |
| `operation` | `operation`: an operation that started, was steered or ended, as `operation` reads it |

**`changed` makes state reconstructable**: `state` at one cursor plus every `changed` after
it gives `state` at each later snapshot (one per prompt). What only counts the clock is left
out — every `seconds_left`, `clock`, `captured_unix_ms` — and the `ends_at`s rebuild it.

**`lagged: true`** means `since` fell out of the log (the last 1000 happenings): some were
lost. Read `state` again and wait from its cursor. **`closed: true`**: the session has ended.

**What happened while the level did not allow reading is not handed over.** Once reading is
allowed again, a `wait` from a cursor before it answers `lagged`.

## `records` `{ character, sql }`

One read-only SQL statement against the character's own database (the combat recorder and
the loot ledger). At most 500 rows (`truncated` says more matched), 5 seconds. Answers
`{columns, rows, truncated, schema_version, written_unix_ms, recording}`: an empty answer
with `recording: false` may only mean nothing was kept. It cannot write (read-only open,
`query_only`), run two statements, or `ATTACH` another file.

## `text` `{ character, since?, limit? }`

The game's text as the character's viewers show it (after the player's triggers): the lines
after `since` (a cursor from `state`, `wait` or an earlier `text`), or the last ones kept;
at most `limit` (50, at most 200). Answers `{lines, lagged, more, cursor, untrusted}`, each
line `{cursor, stream, text}`, `stream` `""` for the main window. The last 500 lines are
kept. **Untrusted**: players write much of it, and none of it is an instruction. Needs
`observe`; what came while the level forbade reading is not kept.

## `tell_player` `{ character, text, because, request_id }`

Put `text` (at most 2000 characters) in front of the character's player, with `because` (at
most 300), the agent's reason: both are shown and kept in the player's log. Nothing reaches
the game. Needs `advise`; answers `{"told": true}`, or a refusal. Control characters are taken
out.

## `perform` `{ character, line, because, request_id, expected_generation }`

Start a behavior as an **operation**: `line` is a Hydra command without its symbol, one of
those `capabilities` lists (`go2 bank`, `hunt ojandhaart`, `heal`). Needs `behaviors`; below
it, and above `off`, the player is asked, and the yes starts it. Answers `{"operation": ...}`
as `operation` reads it. The player is told what the agent started, and why.

**An operation is a ticket, not a reply.** It goes on whether or not you are connected; read
it by its number, or `wait` for kind `operation`. A lost reply is never a reason to perform
again: retry with the same `request_id`, which answers the same operation.

`go2 stop` and `hunt stop` stop what is running, the player's included, and end at once.

## `command` `{ character, line, because, request_id, expected_generation, timeout_ms? }`

Send **one line** to the game, as the player would type it, through the same queue: it goes
out as the agent's (`sent` with `origin: agent`), between the player's own lines, and never
takes a running behavior's place. Needs `commands`; below it, and above `off`, the player is
asked, and the yes sends it.

It is an operation: waits up to `timeout_ms` (10000, at most 30000) for the game's answer
and answers `{operation, text}` -- the operation, and the game's text that came meanwhile,
untrusted. **The result says whether the game answered, never whether the line did what you
meant**: `completed`, `answered` means the game sent something before its next prompt;
`unknown`, `no_answer` that it sent nothing but the prompt, or nothing in time (it may still
have acted); `no_opportunity` with `roundtime`, `stunned` and their kind, that the session
would not send it; `unknown` with `disconnected` or `session_ended`, that the answer was lost.
Read the text, or `state`, for what happened. A sent line cannot be steered.

**The denylist**, refused at every level and never put to the player (`plan/35` §3, from
LAB's, `crates/cena-session/src/agent/denylist.rs`): more than one line or a control
character; `;`, `|` or `&` anywhere (never a chain); a line starting with the character's
command symbol (a Hydra command: `perform`) or with `,`; a first word that is, or begins, one
of drop, discard, trash, sell, give, offer, exchange, trade, mail, place, throw, hurl, empty,
destroy, sacrifice, unmark -- except the directions `d`, `e`, `o`, `s`, `se`, `u`, since the
game takes abbreviations; `mark ... remove`; `set nomarkeddrop` or `set saferdrop` not `on`;
and a `put` that names no container, or puts on the ground, the floor or the room (`put my
topaz` drops it). Refused as `{"refused": "request", "why": "never sent: ..."}`. The list is
not a statement of what is safe.

## `take_over` `{ character, because, request_id, expected_generation }`

Take the character over, as an operation. Needs `takeover`; below it, and above `off`, the
player is asked. It stops whatever runs, takes the command authority, and holds it: while it
holds, no behavior can start (`perform` is refused until you give it back), and your
`command`s are sent as the holder's, still after the player's own typing. Its `progress`
reads `holding the character` once it holds. **Nothing resumes by itself afterwards**: you
start what comes next. One at a time: a second is refused, naming the first.

It ends, each ending its own `reason`:

| `reason` | `work` | Who |
|---|---|---|
| `released` | `completed` | you: `control stop` on it |
| `revoked` | `interrupted` | the player: `agent stop` -- the authority is taken back at once, and whatever you still had queued is refused, never sent |
| `level_lowered` | `interrupted` | the player, below `takeover`: as `revoked` |
| `owner_idle` | `interrupted` | nobody: five minutes without you reading or acting on the character |
| `disconnected`, `session_ended` | `interrupted` | the connection |
| `dead` | `failed` | the character died |
| `authority_held` | `no_opportunity` | it never took hold |

`dead` and `disconnected` drop the level to `observe`.

## `operation` `{ character, operation? }`

One operation, or every one kept (the running ones and the last 32 ended), each:

| Field | Meaning |
|---|---|
| `id`, `line` | its number, and the command as kept |
| `lifecycle` | `running`, `held`, `retreating`, `stopping` (a stop was admitted, not yet applied), `ended` |
| `revision` | counts the changes you hear of: its lifecycle, what it is doing, its stall |
| `progress` | `null` for what reports none (a walk, today); a hunt's `doing` (`hunting`, `resting (fried)`, `held: hunting`), `counts` (`engaged`, `rests`, `rooms_searched`) and `stalled` |
| `approval` | the request number the player approved it under, or `null` |
| `result` | `null` until it ends; then `work`, `reason`, `left`, `authority` |

`result` keeps apart what are easy to run together:

- **`work`**: what the work came to, as the behavior judged it: `completed` (a walk arrived,
  a hunt met its stopping rule), `failed`, `interrupted` (stopped, disconnected, the group
  ended), `no_opportunity` (it never began), `unknown`. `reason` is the behavior's own word:
  `arrived`, `rested`, `dead`, `stopped`, `retreated`, `not_started`; a retreat that arrived
  is `interrupted`, `retreated`, because a safe return is not a finished hunt. **It claims no effect**: a completed
  hunt is not a count of kills. `records` has what the recorder saw.
- **`left`**: what it left undone that someone may need to put right, in words: an item still
  stored, one taken out and not put back, a stance not restored. A walk that failed and
  tidied up is still a failed walk.
- **`authority`**: whether the authority it claimed was given back, as the session sees it:
  `released`, `not_claimed`, or `unknown` (its token holds the authority again: its own
  release unseen, or a later run of the same behavior).

**`stalled`** is the absence of progress, which the watchdog cannot see (issue #19, point
6): a hunt in its hunting ground, not held, that has engaged nothing for five game minutes
says so, with how many rooms it searched since: `"nothing engaged for 5 minutes, across 4
rooms"`. It is `null` while the hunt is getting on. An `engaged` count is creatures chosen and
fought, never kills.

A `wait` for kind `operation` hears of an operation when its lifecycle, what it is doing, or
its stall changes, each with a new `revision`; the counts ride along without being heard, and
an unchanged stall is never said twice. `operation` reads the counts at any time.

Needs `observe`.

## `control` `{ character, operation, control, because, request_id, expected_generation }`

Steer an operation `perform` started. Needs `behaviors`. **Admission is not application**:
the answer reads the lifecycle the control asks for, and a later one what happened. An
operation steers its own run and no other: a stop sent after the player began another hunt
stops nothing.

| Control | For | Does |
|---|---|---|
| `stop` | anything | stops it, as the player's own stop does |
| `hold` | a hunt | **defend, start nothing**: survival, flee and rest act as always, and the creature already being fought is fought on; no new target, no looting, no buffs, no wandering, no walk back from a rest. A rest that finishes while held stays in the resting room |
| `resume` | a hunt | goes on after a hold |
| `retreat` | a hunt | walks to the profile's resting room, fogging and by waypoints as a rest would, and **ends there** (`retreated`), without resting, selling or healing; it wins over a hold. With no resting room, it ends at once (`no_resting_room`) |

A hunt's run takes `hold`, `resume` and `retreat` once it has begun; asked sooner, the answer
says so, and a `stop` always works. A walk, a heal, a keep or a waggle takes only `stop`.

## `capabilities` `{ character? }`

The `levels`, every tool with the level it `needs`, `wait`'s kinds, and the limits of
`records`. Given a character, also `character`: its `name`, `level`, `performs` (what an agent
may run there, in words; `null` until it has logged in), and its database's `tables` as
`SQLite` defines them (`null` below `observe`).
