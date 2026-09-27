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
| `HYDRA_SCRIPTS` | the folder the player's scripts are in |
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
| `line` | `cursor`, `stream` (`""` is the main window; `thoughts`, `speech`...), `text`: a line of game text **as the game sent it**, before the player's triggers and `;sorter`. A line the player squelched is still here |
| `sent` | `cursor`, `line`, `origin` (`manual`, `behavior`, `script`, `trigger`, `agent`): a line went out to the game. Yours are `script`, at the cursor `send` answered |
| `prompt` | `cursor`, `time` (the game's clock, epoch seconds, or `null`), `text` (`>`, `R>`...): the end of a chunk |
| `typed` | `line`: the player typed a command for the runner, without the symbol (`trollspeak say hi`, `k trollspeak`) |
| `lifecycle` | `cursor`, `state` (`ready`, `reconnecting`...), `generation` (advances on every reconnect) |
| `lagged` | `missed`: that many of the session's events were missed on the way here; a line a script waits for may be among them |

`cursor` is the session's own, which only increases, across reconnects too: **a line whose
cursor is above the one `send` answered came after your line**. Which lines: every finished line
of every stream, the prompt aside. Lich's scripts see fewer (not the inventory, bounty, society
or spell-window copies, `inventory/13` §1.1); choosing is the runner's.

**`lagged: true`** in the answer means events after `since` were let go for room (the last 4096
are kept). **`closed: true`**: the character's session ended, or the runner was dismissed;
nothing more will come.

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

Show the player `text` (at most 20000 characters), each line of it a line, as a script's
`respond` and `echo` do. `kind` colours it: `info` (absent), `warn`, `error`, `debug`. `mono`
keeps its columns, never re-wrapped, for a table. It never reaches the game. Answers
`{"said": true}`.

Text only: markup a script writes (`<pushBold/>`, `<preset>`) is not drawn. Hydra never puts
text the game did not send among the game's lines (`crates/cena-session/src/notice.rs`).
