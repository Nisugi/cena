# 48 — M7 acceptance: the live run

**Status: READY, not run.** The runbook for `plan/35` §8 step 6, and for the *Shown* criteria
of steps 1 and 2 that only a live run can show. Only the author runs it (`CLAUDE.md`,
Credentials: the binary logs into the live game).

Steps 1-5 are built and tested against a scripted game on branch `m7-agent` (worktree
`G:\dev\Cena-m7`). None has touched the live game. This run is where they do.

**What step 6 asks** (`plan/35` §8): *a hunt ends on its rest threshold, the agent (not the
hunt) decides what is next and does it; the player stops the agent mid-act.* Parts C and D
below are that. Parts A and B are steps 1 and 2's *Shown*. Parts E and F are optional, and
worth doing while the setup is live.

Each part says what to type, what should happen, and what counts as a pass. Write the result
under **Results** at the end: pass, fail, or what surprised you, with the terminal lines.

## Before you start

1. **Build and start Hydra from the worktree**, with the agent's listener:

   ```sh
   cd G:\dev\Cena-m7
   cargo run -p cena -- --character Nisugi --agent
   ```

   Add `--web` if you want Despana too. The terminal prints two `[agent]` lines: the address
   (`http://127.0.0.1:47700/mcp`) and a ready-made `claude mcp add ...` line with the token.

2. **Connect Claude Code, once.** Paste that `claude mcp add ...` line into a terminal. The
   token is kept in `<data>/agent.json`, so the same line works on later runs. Then start
   Claude Code in any folder; it should list `hydra` among its MCP servers (`/mcp`).

3. **Watch the Hydra terminal, not Despana, for the agent.** Every approval request, every
   refusal the player is told of, and every act the agent does with its `because` is a line
   there (`[Nisugi] :: Agent: ...`, `!`). Despana does not show Hydra's notices yet
   (`plan/35` §8, step 2).

4. **The level starts at `off`** and is kept per character. Every part raises it; the last
   step lowers it again.

## Part A — reading (step 1's *Shown*)

In the game: `;agent level observe`. Hydra answers `Agent level: observe -- ...`.

Ask Claude Code, in words:

- *"What is Nisugi's status right now?"* It should use `characters` and `state`, and answer
  with the room, vitals, hands and statuses **as the game reported them**, saying nothing
  about a status the game has not reported.
- *"How many creatures has Nisugi killed in the last week, and of what kind?"* -- or any
  question of yours that no `;loot` or `;combat` report answers. It should use
  `capabilities` for the tables, then `records` with SQL.

**Pass:** both answered from the tools, and the terminal shows nothing sent to the game.

## Part B — levels and approval (step 2's *Shown*)

Still at `observe`, ask: *"Tell me, through Hydra, that the test has started."*

- The agent's `tell_player` is refused, and the terminal shows **`An agent asks to tell you
  something (N characters), because: ...`** with a request number.
- Type `;agent approve <n>`. The message appears as `Agent: ...` with its `because`.

Then `;agent level off`, and ask the agent for Nisugi's status again.

**Pass:** the approval showed the message only after you approved; at `off` the agent sees
only the name and the level, and the terminal says once that an agent was refused.

## Part C — the agent decides after a hunt (step 6)

Pick a hunting profile you trust, and make its hunt **end at its first rest** instead of
walking back out:

```text
;hunt set <profile> rest.stop_after 1
```

(`;hunt unset <profile> rest.stop_after` puts it back afterwards.) Then `;agent level
behaviors`.

Ask Claude Code: *"Start Nisugi hunting on <profile>. When the hunt ends, look at how it
ended and at Nisugi's state, decide what Nisugi should do next -- heal, walk somewhere, or
hunt again -- and do it. Tell me why."*

What should happen:

1. `perform` starts `hunt <profile>` as an operation; the terminal shows `Agent: \`hunt
   <profile>\` (operation N), because: ...`.
2. The hunt hunts. The agent waits on it (`wait`, kind `operation`) rather than polling
   every few seconds; its `progress` says `hunting`, then `resting (...)`.
3. At the first rest's end the hunt **ends**: the operation's result reads
   `completed`, `rested`, with the authority `released`.
4. **The agent, not the hunt, chooses what comes next**, and starts it with another
   `perform` (for example `heal`, `go2 <place>`, or the hunt again), saying why.

**Pass:** the second operation was the agent's choice, started after the first ended, and
its `because` says why.

## Part D — stopping the agent mid-act (step 6)

While the agent's second operation is running -- or start one, *"Walk Nisugi to the
bank"* -- type:

```text
;agent stop
```

**Pass:** Hydra answers `Agent: 1 operation(s) told to stop...`; the operation ends
`interrupted`, `stopped`; `;agent` shows nothing running; the character is not moving.

## Part E — optional: commands and the denylist

`;agent level commands`. Ask: *"Glance around as Nisugi and tell me what you see."* It should
use `command` with `glance`, and answer from the text that came back. The terminal shows
`-> [agent] glance`.

Then ask it to *"drop something Nisugi is holding"*. **Pass:** refused as `never sent: ...`,
and nothing reaches the game -- no `-> [agent] drop` line.

## Part F — optional: takeover

`;agent level takeover`. Start a hunt yourself (`;hunt <profile>`), then ask: *"Take Nisugi
over and stand guard: glance around once."*

- The hunt stops; the takeover's progress reads `holding the character`; `;agent` says `An
  agent holds this character (operation N)`.
- Type `;agent stop`. **Pass:** the takeover ends `revoked`, and a `;hunt <profile>` you type
  now starts.

## Afterwards

- `;agent level off` (or `observe`), so the agent can do nothing until you choose again.
- `;hunt unset <profile> rest.stop_after`.
- Leave Hydra running while you write the results, if anything failed: the operation's JSON
  (ask the agent for `operation` on it) says what the session saw.

## Results

*(The author's, from the run.)*

| Part | Result | Notes |
|---|---|---|
| A | | |
| B | | |
| C | | |
| D | | |
| E | | |
| F | | |
