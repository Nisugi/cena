# Hydra's agent contract — `hydra-agent/1`

What an MCP client may rely on when it reads a Hydra character (`plan/35`, M7). Versioned
as `crates/cena-ui/WIRE.md` is: a field that changes meaning or goes bumps the version; a new
field, tool or happening kind does not, because **a client ignores what it does not know**.

Step 1 is **read-only**. Nothing here sends to the game.

## Connecting

- **Where**: `http://127.0.0.1:<port>/mcp`, MCP's streamable HTTP. The port is 47700 unless
  Hydra was started with `--agent-port N`. Loopback only; the `Host` header must be local.
- **Who**: a bearer token in the `Authorization` header — **never a tool argument**. Hydra
  keeps it, with the address, in `<data>/agent.json`; delete the file for a new one.
- **What**: `GET /health` (no token) answers `{"hydra": "<version>", "protocol":
  "hydra-agent/1"}`. Refuse a listener whose protocol you do not speak.
- **When**: only while Hydra runs with `--agent`.

Every tool answers JSON in its first text block.

## `characters`

Each character this Hydra runs: `character`, `game`, `lifecycle` (`ready`, `reconnecting`,
...), and `records` (whether its database exists yet).

## `state` `{ character }`

One consistent read of the character, and the cursor to `wait` from. The name matches
ignoring case.

| Field | Meaning |
|---|---|
| `character`, `game`, `lifecycle`, `generation` | who, which instance, where the session is in its life, which connection (it advances on every reconnect) |
| `cursor` | the last event this state includes: `wait` from here |
| `captured_unix_ms`, `clock` | when this was read, on this machine and on the game's clock (epoch seconds) |
| `room` | `id` (the game's number), `title`, `exits`; `creatures` (`id`, `noun`, `name`, `hostile`, `statuses`, `dead`), `objects`, `players` |
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
| `sent` | `line`, `origin` (`manual`, `behavior`, `script`) |
| `notice` | `text`: Hydra said something to the player |
| `lifecycle` | `state` |
| `gap` | some `sent`/`notice` were missed; the rest come from the next snapshot |
| `changed` | `fields`: every `state` field that changed, with its new value |

**`changed` makes state reconstructable**: `state` at one cursor plus every `changed` after
it gives `state` at each later snapshot (one per prompt). What only counts the clock is left
out — every `seconds_left`, `clock`, `captured_unix_ms` — and the `ends_at`s rebuild it.

**`lagged: true`** means `since` fell out of the log (the last 1000 happenings): some were
lost. Read `state` again and wait from its cursor. **`closed: true`**: the session has ended.

## `records` `{ character, sql }`

One read-only SQL statement against the character's own database (the combat recorder and
the loot ledger). At most 500 rows (`truncated` says more matched), 5 seconds. Answers
`{columns, rows, truncated, schema_version, written_unix_ms, recording}`: an empty answer
with `recording: false` may only mean nothing was kept. It cannot write (read-only open,
`query_only`), run two statements, or `ATTACH` another file.

## `capabilities` `{ character? }`

This connection's level (`observe`), the tools, `wait`'s kinds, and, given a character, its
database's tables as `SQLite` defines them.
