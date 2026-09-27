# 35 — M7: the agent protocol

**Status: PROPOSED 2026-09-24, author asked for it** (*"let's write up a plan"*). M7 is
`12` §8's seventh row, *"agent protocol (`07`, safety per §5.5 + §6.3)"*: a program outside
Hydra -- an LLM, an MCP client, a script, a person with a terminal -- observes a character
and acts for it, through the same seams a behavior uses.

Every line below is marked. **AUTHOR** is a decision the author gave, quoted and dated.
Everything else is **PROPOSED**: mine, reasoned from the code and from the one working
prior art, and open until the author says otherwise. (`12` §8's milestone order is itself a
proposal of mine; the *goal* is the author's.)

## 0. What is decided

**AUTHOR, 2026-09-17** (`research/07` "Decisions taken"; the goal re-confirmed 2026-09-18):

- **Capability:** the agent *plays the character*: it perceives, decides and acts.
- **Topology:** **external, over a socket.** Hydra ships no model client, no API key
  handling, no vendor SDK.
- **Safety:** the same limits as everything else, **plus an audit trail**. No privileged
  path to the game.

**AUTHOR, 2026-09-24**, answering four questions:

1. **Transport.** Asked whether MCP-shaped and transport-agnostic was right. §2 proposes it.
2. **Control is a setting of levels:** *"agent control level = this, this this, this this
   this, and it can be set. if an agent tries to send a command they don't have access to,
   they get a notice."* (§3)
3. **The agent takes over:** *"agent is expected to take over if something goes wrong I
   would think."* (§4; the split proposed there is confirmed below.)
4. **What it reads: all of the character's statuses:** *"roundtime, stunned, dead, prone,
   kneeling (posture), hidden, poisoned, diseased. All of the statuses we track for a
   character."* (§5)

**AUTHOR, 2026-09-24, second round**, answering the first draft's open questions:

1. **The takeover split stands:** *"that's fine yes"*, with a question -- what if the
   behavior has never seen the emergency? §4 answers it.
2. **Levels persist:** *"persistent"*. No level is granted per run.
3. **The denylist starts from LAB's list:** *"we can start with that one"*.
4. **An agent's single command interleaves**, as typing does.
5. **In-game questions to the agent: leave the door open.** *"I'm not sure what I would ask,
   but others do, so I guess at the very least leave the door open and make it easy if we
   don't add it now."* §6 records how.

**Not decided:** when. M6 is not closed (M6c under way, M6d and M6e ahead, then the live
acceptance hunt, `plan/30` §7), and M4 is not accepted. Nothing here is built before M6
closes unless the author moves it.

## 1. Prior art: Lich Agent Bridge

`reference/lich-agent-bridge` (<https://github.com/elanthia-online/lich-agent-bridge>,
ATARI, alpha, 10 commits 2026-09-06 to 09-14, **GPL-3.0-only**) is an agent bridge for
Lich, used live. About 58k lines: a Ruby bridge inside Lich, a Python hub, a TypeScript
MCP adapter. It is the only working answer to M7's questions, and it reached `research/07`'s
shape independently.

Its `wiki/project/Safety.md` is the document to read. It records lessons that came from
running it, and one of them changes §4.

**Take the designs, not the code.** GPL-3.0-only code copied in would carry that licence
if Hydra is ever distributed. §10 tables what is taken and what is left.

## 2. Transport: MCP over loopback HTTP

**PROPOSED.** `research/07` §8 Q1 recommended "MCP-shaped, transport-agnostic"; LAB built
exactly that and it works.

- **MCP**, because any MCP client -- Claude Code among them -- drives a character with no
  adapter written for it.
- **Streamable HTTP on loopback, not stdio.** Hydra is already running with its sessions;
  stdio would have the client launch Hydra. The listener sits beside Despana's (`cena-web`,
  `crates/cena-web/src/server.rs`), loopback only, with a pairing token as a bearer header
  and **never a tool argument** (LAB's rule). A health route identifies Hydra and the
  protocol version, so a client can refuse a listener it does not speak.
- **Transport-agnostic is a layering rule, not a trait.** The agent core -- vocabulary,
  levels, audit -- contains no MCP type; a thin layer maps MCP tools onto it. A transport
  trait with one implementor is what `plan/05` §-1 refuses. A second transport, if one is
  ever wanted, is a second thin layer.
- **Pull, not push.** An MCP client cannot be relied on to wake a model for a notification
  the server sends. So observation is a tool, `wait(since, kinds, timeout)`, bounded per
  call. It maps directly onto what is built: `SessionObserver::subscribe` numbers every
  event and says `Lagged` rather than dropping silently (`crates/cena-session/src/observation.rs`).
- **One endpoint for every character**; each tool names its character; levels are per
  character. That is the hub's shape (`plan/29`).

**To measure before choosing:** an MCP server library against a hand-written JSON-RPC
layer over the listener that already exists. Choose by what the library pulls into the
build and whether `cena-web`'s server can host it.

**MEASURED 2026-09-27, and chosen: `rmcp` 3.4.1**, the official Rust SDK, with `server`,
`macros` and `transport-streamable-http-server`. A throwaway crate's lockfile against the
workspace's (`cargo generate-lockfile`, then `comm -23` of the two crate lists): **32 crates
new to Hydra**, chiefly `chrono`, `futures`, `uuid`, `schemars` (tool input schemas) and
proc-macro helpers; it resolves against the same axum 0.8 `cena-web` uses, with no second copy.
Its `StreamableHttpService` is a tower `Service` (`transport/streamable_http_server/tower.rs:1075`
in the crate), so an axum router mounts it with `nest_service`; it also checks the `Host`
header, localhost by default. Chosen over hand-written JSON-RPC because it tracks the MCP
specification's revisions, which a hand-written layer would have to follow by hand, and
because its server-to-client stream is the push `plan/46` §4.1 needs.

## 3. Control levels

**AUTHOR:** levels, set as a setting; an act above the level gets a notice.
**PROPOSED:** the levels themselves, and the rules around them.

| Level | The agent may |
|---|---|
| **Off** (default) | nothing: no connection for this character |
| **Observe** | read state, wait on events, read the catalog |
| **Advise** | also put a notice in front of the player; nothing reaches the game |
| **Behaviors** | also run Hydra commands: start, stop, hold or resume a behavior (`;hunt`, `;go2`) |
| **Commands** | also send game commands, one line at a time, through the same queue, round trip and gates, minus a denylist |
| **Takeover** | also preempt a running behavior and hold the authority |

Each includes everything above it. **Behaviors is the line that matters.** Below
Commands, everything an agent does goes through curated code whose outcome is checked, so
"may hunt but must not drop my sword" is Behaviors.

- **The player sets it; the agent never does.** At the terminal, the hub card, or the
  character's page. `research/07` Rule 4.5, and LAB's full access, which *"cannot be
  enabled remotely"*.
- **Stored in the character's settings file, and it persists** (AUTHOR), as an `agent`
  section beside `commands` (`crates/cena-session/src/settings_store.rs`, one file with a
  section per system). LAB ends its full access with the session; the author chose
  otherwise, and the level is what the player last set, at every level.
- **Lowering and raising does not restore approvals.** LAB: turning actions off cancels
  auto-approval, and turning them on again does not quietly bring it back.
- **Enforced in the session, not in the agent crate.** The level is checked where the
  authority is, so no path around it exists; a rule not enforced is a wish (`plan/05` §0).

### Above its level: a notice, and a way to say yes

**AUTHOR:** the agent gets a notice. **PROPOSED:** two things happen.

1. The agent gets a **typed refusal naming the level it needed**, so it can tell the player
   "I need Commands for that" instead of guessing.
2. The player gets a **notice** in that character's stream, with an **[approve] link**.
   Approval applies to that one act, bound to its generation and a short expiry (LAB), and
   grants nothing further.

A refusal only the agent sees is invisible to the one person who can change the level, and
repeated attempts are what the player most needs to see. The link is the first caller of
the command link `Notice` was built to leave room for and does not yet have
(`crates/cena-session/src/notice.rs`, "What is built, and what is not").

### The denylist, even at Commands

**PROPOSED from LAB**, whose top mode still refuses: dropping, giving or trading away,
selling, destroying, unmarking, disabling a drop guard, arbitrary scripts, and multi-line or
chained input. Hydra's version: one line; no command symbol at the Commands level (a `;`
line is a Behaviors act); and the verbs above. **AUTHOR: start from LAB's list.** It grows
the way the hunt's guard vocabulary does, from a real case, never ahead of one.

## 4. Taking over

**AUTHOR:** the agent is expected to take over if something goes wrong.

**PROPOSED: two kinds of wrong, split by speed.** LAB's rule, learned live: *"Keep policy
and survival decisions deterministic; model availability is not a safety mechanism"*, and
*"A remote model must not be the emergency response mechanism."* Its controllers even
search for a fight *"without agent round trips"*. A model round trip is seconds; in combat
that is the difference.

- **Emergencies stay with the behavior.** Hunt's Survival and Flee (M6b, `plan/30` §3) get
  the character out, deterministically, with no model in the loop.
- **Then the agent takes over.** A behavior already ends with a typed reason:
  `HuntEnd::Finished(Ending)` when the machine decides, `Stopped(BehaviorError)` when the
  session does -- cancelled, dead, disconnected, authority held
  (`crates/cena-behavior/src/hunt/drive.rs`, `crates/cena-behavior/src/error.rs`). Those
  become events the agent waits on, so it is **on call rather than watching every round**.
  It decides what happens next: the part a hunt cannot.
- **Preempting mid-hunt remains**, at Takeover, for what the agent sees and the hunt does
  not: `SessionHandle::preempt`, cooperative for `PREEMPT_GRACE` and then by force -- the
  hub's stop, already built. It is not the safety net.
- **Steering short of taking over:** hold, resume and retreat on a running behavior, at
  Behaviors (LAB's controller controls; `12` §4.1 already names pause). **Hunt has none of
  them yet** (`grep -nE '"(pause|hold|resume)"' crates/cena-behavior/src` finds nothing).
- **The player outranks the agent.** Typing interleaves with an agent exactly as it does
  with a hunt; an explicit stop preempts the agent; a badge on the hub card and the
  character's page shows who holds the authority.
- **After a takeover, the behavior does not resume by itself.** The agent restarts it, so
  there is no hidden state about what was interrupted.
- **A run that ends badly drops the level.** An agent-held run that ends in death, a
  disconnect or the watchdog firing drops that character to Observe until the player
  raises it again. LAB's version: a failed handoff stays visible and blocks the next.

**AUTHOR, 2026-09-24:** the split stands.

### An emergency the behavior has never seen

> **AUTHOR, 2026-09-24:** *"what if the behavior hasn't seen the emergency before and
> doesn't know how to handle it? I mean they'd probably be dead before it was caught but its
> a question."*

**Hunt reacts to symptoms, not causes**, so most unseen emergencies are not unseen by it.
MEASURED, `crates/cena-behavior/src/hunt/engine.rs` (the policy table in its module docs)
and `profile.rs` (`wounded_eval`, typed):

| Symptom | Arm | Response |
|---|---|---|
| dead; knocked down | Survival | stop; stand, unless stunned or webbed |
| too many creatures; a creature the profile always flees | Flee | leave the room |
| bleeding; health at or below N; a wound that stops casting or ranged | Rest | walk to the rest room |
| fried, encumbered, out of mana | Rest | the same |

A new creature ability that hurts, a new trap that wounds, a new spell that knocks you down:
each arrives as a symptom Hunt already reads, and it responds without knowing the cause.

**And the bestiary has often seen the cause before the symptom.** Since 2026-09-24 it
carries every creature line Lich's templates record (`crates/cena-model/tools/extract_creatures.rb`),
including **162 trigger lines** keyed by what they warn of -- `bind`, `web`, `silence`,
`calm`, `confusion`, `mindwipe`, `stunning_shout` -- and **281 casting preparations**
across 169 creatures. A match names the effect (`creature_message::classify`, its `key`).
So a hunt can answer the warning, not only the wound, for any creature the bestiary knows;
that is also what `kswole.lic` needs, and it keeps 134 of these by hand.

**Two kinds get through, and they need different answers.**

1. **Too fast for anything.** Dead in one round. No behavior and no model saves that; the
   author's own point. The answer is afterwards: the ending is typed (`Dead`), the wire log
   has every line, and the case becomes a rule -- a flee name, a threshold, a guard. That is
   how curated behaviors grow: from real cases, as `plan/30` §8 says of the guard vocabulary.
2. **No symptom Hunt reads.** Nothing drops, nothing it checks turns on, and yet something
   is wrong. This is the agent's case: **novelty, not speed** -- the one place a model's
   judgement is worth its round trip. PROPOSED: three signals, each an event the agent
   waits on, each cheap because the fact already exists:
   - **A status Hunt does not react to comes on.** The agent sees every status (§5); Hunt
     reads a handful. Any other onset during a hunt is an escalation.
   - **The room changed and Hunt did not move it.** Swept away, teleported, dragged.
     Departures carry a direction and arrivals do not (`state/departure.rs`, `CLAUDE.md`),
     so "moved without a departure of ours" is readable.
   - **No progress.** The watchdog fires, or commands are refused again and again.

   The agent, at Takeover, preempts; at a lower level, it tells the player (Advise). Either
   way the case is recorded, and the next time it is a rule.

## 5. What the agent reads

**PROPOSED:** a projection of its own. **AUTHOR:** it carries every status tracked for the
character.

Not the session's own events: `Event` and `Frame` derive no `Serialize`
(`crates/cena-session/src/actor/event.rs`, `crates/cena-protocol/src/frame/vocabulary.rs`),
they carry every frame, and serializing them would make internal types a public contract.
Not `cena-ui`'s `SessionView` either: it is built for rendering -- styled runs, story
history -- and a model reading it reads a screen, pays per token, and meets player-written
text mixed into the structure.

So `cena-agent` gets its own vocabulary, versioned the way `cena-ui` is (`WIRE_VERSION`,
`crates/cena-ui/WIRE.md`), with a contract document of its own.

### Statuses: all of them, by construction

The author named roundtime, stunned, dead, prone, kneeling, hidden, poisoned and diseased,
then *"all of the statuses we track for a character"*. MEASURED, that is three sources:

| Source | What | Where |
|---|---|---|
| the game's indicators | Lich's `ICONMAP`, 11: kneeling, prone, sitting, standing, stunned, hidden, invisible, dead, webbed, joined, bleeding | `crates/cena-model/src/status.rs` (`StatusInfo`) |
| the same store | poisoned, diseased | `StatusInfo`, as above |
| text only, no indicator | bound, calmed, cutthroat, silenced, sleeping, thorned | `crates/cena-model/src/state/afflictions.rs` (`Affliction::ALL`), stored into `StatusInfo` under `id()` |
| computed | roundtime and cast roundtime remaining | `crates/cena-model/src/state/clock.rs` |

Posture is four of the indicators (standing, sitting, kneeling, prone); stance is separate
and goes beside it.

- **The projection passes `StatusInfo` through whole**, not a list chosen here. A status
  the model learns reaches the agent with no edit in `cena-agent`, and a test asserts that
  every id the model writes into `StatusInfo` -- the indicators, poisoned, diseased and
  `Affliction::ALL` -- appears in the projection. A curated list is the drift
  this rules out.
- **Unknown is absent, never `false`.** `StatusInfo::is_known` already tells "reported
  inactive" from "never reported" (`plan/12` §5.2). After a reconnect nothing is known,
  and the agent is told so rather than told the character is fine.
- **Onset and offset both.** Indicators carry both edges (`status.rs`, "These have both
  edges"), so a status change is an event the agent can wait on.

### The rest of the state

Facts first, each already in the model: room (id, uid, title, exits), vitals, stance, hands
with their item ids, injuries, active effects with time remaining, the creatures and objects
in the room with their ids (creatures with their statuses), mind, encumbrance, which behavior is running and who holds the
authority, and the agent's own level. LAB's snapshot is a cross-check: its fields
(`reference/lich-agent-bridge/lich/lich-state-core.rb`) are this list, and nothing on it
is missing here.

### Events, and player text

- **Few and meaningful, filterable by kind:** a behavior started or ended and why, a status
  changed, died, spoken or whispered to, a creature arrived or left, lagged, the level
  changed, an approval granted or refused.
- **Player-written text only in fields typed as untrusted** -- a speaker and a text, never
  mixed into structure -- so a player who names themselves *"ignore previous instructions"*
  is a data problem, not an instruction. `research/07` §6's injection test pins it.
- **Full game text is a separate tool the agent asks for**, bounded, marked untrusted
  (`research/07` §8 Q3). The agent chooses to pay for it.
- **Every response is size-capped**, as `cena-ui` caps a line (`MAX_LINE_BYTES`); LAB caps
  evidence per result and per turn.

## 6. The verbs

**PROPOSED.** `research/07`'s three -- observe, act, ask -- as tools:

| Tool | Level | What |
|---|---|---|
| `characters` | Observe | the characters this Hydra runs, with their levels |
| `state` | Observe | the projection, now |
| `wait` | Observe | events after a cursor, bounded by a timeout; `Lagged` when the cursor fell out of the window |
| `text` | Observe | recent game text, untrusted, bounded |
| `capabilities` | Observe | what this character's level permits and which Hydra commands exist, **generated**, not a hand-kept list |
| `records` | Observe | a read-only query over the character's database (§6, "Records") |
| `tell_player` | Advise | a notice in the character's stream |
| `perform` | Behaviors | run a Hydra command; returns an **operation id** |
| `operation` | Behaviors | watch an operation to its end, by id and cursor |
| `control` | Behaviors | hold, resume, retreat or stop one operation |
| `command` | Commands | one game line |
| `take_over` | Takeover | preempt the running behavior and hold the authority |

Every act carries:

- **`expected_generation`**: a stale one is refused. Browser commands already carry the
  generation (`crates/cena-ui/WIRE.md`, `command`).
- **`because`**: the agent's stated reason, written to the player log and shown on the
  character's page (`research/07` Rule 4.2).
- **An answer that tells sent from done.** A game command is `Sent`, which is not a receipt;
  an operation ends with its behavior's typed result. LAB: *"Report sent-but-unverified
  separately from evidence-backed success."*

**Operation ids** are LAB's, and the one thing `research/07` lacked: an act is a ticket the
agent watches, so a dropped connection is never a reason to send it again.

### Records: the reports nobody has written yet

> **AUTHOR, 2026-09-24:** *"I guess you could ask it to give you stats on database info that
> we haven't built reports for yet to see if a report is worth it. That's one thing I could
> use it for I suppose."*

The character's database is one file, `{game}_{name}_combat.db`
(`crates/cena-session/src/combat_recorder/worker.rs`), holding the combat recorder and the
loot ledger (`plan/34` Stage 2). A report is a query someone decided was worth naming; the
agent can run the query first, so the decision is made on an answer rather than a guess.

**PROPOSED:** `records(character, sql)`, at Observe, because reading your own history is
observation.

- **Read-only by construction**: its own connection, opened read-only and with
  `PRAGMA query_only`, never the recorder's. A write fails in `SQLite`, not in a check of
  ours.
- **Bounded**: a row cap and a time limit (`SQLite`'s progress handler), like every other
  answer (§5).
- **The schema is discoverable**: `capabilities` returns it, generated from the database
  rather than written down, so a migration reaches the agent with no edit.
- **SQL, not a query vocabulary.** The point is the question nobody anticipated, and a
  vocabulary could only ask the anticipated ones. It is the character's own data, local; what
  leaves the machine is what the player's chosen model sees, as with everything else (LAB's
  privacy note, `README.md`).

A query asked twice is a report asking to be written. The audit trail (§6, `because`) makes
that visible: the same question in the player log is the evidence.

### Questions from inside the game: the door left open

**AUTHOR:** not built now; easy to add. Nothing below is built in M7. It is recorded so that
adding it later is three small pieces, each an extension of something M7 builds anyway:

1. **A Hydra command** -- a word on the `;` line (the claimant desk,
   `crates/cena-session/src/command/claimant.rs`) -- that takes the rest of the line as the
   question.
2. **An event**, "the player asked", carrying the question as untrusted text (§5), which the
   connected agent is already able to `wait` on.
3. **The answer is `tell_player`** (Advise), a notice in that character's stream.

No model client, so the author's 2026-09-17 decision holds: the question reaches whatever
agent is connected, and with none connected, the command says so.

Two things M7 must get right for the door to stay open, named so step 1 does not close it
by accident: **a client ignores an event kind it does not know** (the contract says so, as
`crates/cena-ui/WIRE.md`'s fixture carries unknown values on purpose), so a new kind is not
a breaking change; and **`tell_player` stands alone**, never only the answer to an
operation.

## 7. Where it lives

- **`cena-agent`**, a new crate: the vocabulary, the projection, the MCP layer. It depends on
  `cena-session`; `crates/cena-arch-tests/tests/layering.rs`'s `ALLOWED_EDGES` gains its row
  when it is created, and the binary gains an edge to it.
- **In `cena-session`:** the level and its check; a fourth `Origin`, `Agent`, beside
  `Manual`, `Behavior` and `Script` (`crates/cena-session/src/command/verdict.rs`) so a log
  can tell "the agent sent this" from "the player typed this", the reason `Script` exists.
  **An agent's single command interleaves** (AUTHOR), queueing as `Script` does and for its
  reason; only Takeover holds the authority.
- **The binary owns the listener's lifetime**, as it owns Despana's, and reaches the table
  of sessions for multi-character tools the way the hub does.
- **Despana, not a new frontend,** carries the player's side: the driving badge, the level
  control, the audit trail, the approve links. A second frontend would be a second place to
  look for the stop button.
- **Architecture tests with the rules:** the level is checked on every agent send path; the
  crate edges; no `Frame` or `Event` crosses the agent wire.

## 8. Steps

Each ends in something demonstrable, as `12` §8 asks.

1. **Read-only.** `cena-agent`, the MCP listener, Observe: `characters`, `state`, `wait`,
   `capabilities`, `records`, and the status projection with its every-status test.
   **BUILT 2026-09-27** (`crates/cena-agent`, contract `CONTRACT.md`, `hydra-agent/1`; the
   binary's `--agent`, port 47700, the token kept in `<data>/agent.json`). `wait` is the
   difference between the session's own snapshots after each prompt, as Despana stays
   current, never a second model. Tests: `crates/cena-agent/tests/agent.rs` (every status
   passed through; an unstated list is `null`; `changed` rebuilds state; the whole path over
   MCP behind the token, against a scripted game) and `records.rs` (read-only, capped, no
   `ATTACH`, the last proved by raising the limit and watching the test fail). *Shown*, live
   with the author: not yet.
   *Shown:* an MCP client connects and answers "what is my character's status", live,
   author present -- and a question of the author's about the database that no report
   answers yet.
2. **Levels and notices.** The setting, the check, refusal as a notice with an approve link,
   `because` in the player log, the badge and the level control in Despana.
   **BUILT 2026-09-27**, on the command line; Despana's part waits (below).
   - **The level** is `cena_session::agent::Level`, kept in the character's settings file
     (`{"agent": {"level": "observe"}}`), `off` until the player raises it: `;agent level
     observe`, read back at every login. **Three levels exist, not six**: `off`, `observe`,
     `advise`. Each later step adds its level beside the tools it allows, so a player can never
     set a level today that means more after an update.
   - **The check is in the session.** An agent acts only through a `Door`
     (`crates/cena-session/src/agent.rs`), each of whose acts checks the level; `cena-agent`
     holds a door and never a `SessionHandle`, which
     `crates/cena-arch-tests/tests/layering.rs` (`the_agent_acts_only_through_the_door`)
     asserts. A read is checked by the tool that reads, against the same level, because what
     it reads from is an observer and changes nothing.
   - **A refusal** is a tool error the model reads: the level, the level needed, and whether
     the player was asked. At `off` nobody is asked and the agent sees only the character's
     name and level; a refused read asks nobody either. The player is told either way, at most
     once every five minutes with a count, so an agent polling at `off` cannot fill the stream.
   - **An act above the level, and above `off`, waits for the player**: a notice with the
     request's number (`;agent approve 3`, `;agent deny 3`), saying what the act is (for a
     message, its length and never its words, which the level has not allowed yet) and the
     agent's `because`. The yes is that act, once, on the connection it was asked on, within
     two minutes; at most three wait; a change of level drops them all. The agent reads the
     answer in `wait` (`approval`), with the level's changes (`level`).
   - **`tell_player`** is the first act: `advise`, with a required `because`, both shown and
     kept in the player log (the log's `hydra` tag). Nothing reaches the game.
   - **What happened while reading was not allowed is not handed over**: once the level
     allows it again, a `wait` from before answers `lagged`.
   - **Not built, and why.** Despana's badge, level control and clickable approve link: the
     page today is a demo of Despana (author, 2026-09-26), and it shows no notices at all
     (`grep -rn Notice crates/cena-web/src` finds nothing), so a refusal has no way onto it yet;
     `;agent` typed in its command box works as at the terminal. The request id
     of issue #19 point 4 waits for step 3's first act that reaches the game: a lost reply to
     `tell_player` shows a message twice, and nothing worse. `expected_generation` likewise:
     an approval is already bound to its connection.
   - Tests: `crates/cena-agent/tests/agent.rs` (`at_off_...`, `an_act_above_the_level_...`,
     over MCP against a scripted game), `crates/cena/src/agent.rs` (the level kept and read
     back; approve once, deny), `server.rs` (`every_tool_has_a_level`). Mutations: an unplaced
     tool, the notice unthrottled, and a `SessionHandle` in `cena-agent` each turn a test red;
     the first version of the `lagged` rule read the level when the watcher got to an event
     rather than where it stood in the stream, and its test caught it.
   *Shown*, live with the author: not yet (with step 1's).
3. **Behaviors.** `perform`, `operation` and `control` over Hydra commands; behavior endings
   as events. Hold and resume need building in hunt first.

   **AUTHOR, 2026-09-27**, asked three questions, took the recommendation each time:
   - *Which Hydra commands at `behaviors`?* **Start and steer only**: `go2 <place>`,
     `go2 stop`, `hunt <profile>` (and `quick`, `bounty`), `hunt stop`, `heal` (and `stock`,
     `fill`), `keep`, `waggle`. Not `multi`/`foreach` (any game line), not `sc` (casts at
     anyone, players included), not a group hunt (it starts other characters, whose levels
     are theirs), not `agent`, and nothing that writes a setting or a profile: a hunt
     profile's steps are game commands, so editing one would get round the Commands level.
   - *Hold?* **Defend, start nothing**: survival, flee and rest still act; no new target, no
     looting, no buffs, no wandering, until resume or stop.
   - *Retreat?* **The rest room, then end**: walk to the profile's rest room now and end the
     hunt there, so the agent decides what is next (LAB's meaning).

   Built in three parts: **3a** operations, **3b** hold, resume and retreat, **3c** progress
   and its absence (issue #19, point 6).

   **3a BUILT 2026-09-27.**
   - **The fourth level, `behaviors`.** An agent performs a Hydra command as an **operation**
     (`crates/cena-session/src/operation.rs`): a ticket read by its number, or waited on
     (kind `operation`), never a reason to send again. The session holds no command
     knowledge: the binary registers a `Performer` (`crates/cena/src/perform.rs`, the
     allowlist above), as it registers the command desk.
   - **Each request admitted once** (issue #19, point 4): every act carries the caller's
     `request_id`; the same id and act again answers as the first time and is never done
     twice, the same id for another act is refused, a duplicate while the first is being
     admitted is told so, and an approved request's id then names its operation. Kept in
     memory, the last 256 per character; a restart forgets them, as it ends the operations.
     `perform` and `control` carry `expected_generation`.
   - **A result that keeps work, leftovers and release apart** (issue #19, point 3): the
     behavior's own verdict (`completed`, `failed`, `interrupted`, `no_opportunity`,
     `unknown`) with its word for why, read off `HuntEnd` and `Travelled`
     (`crates/cena-behavior/src/operation.rs`); what a walk left undone (an item still
     stored, one taken out, a stance not restored); and whether the authority was released,
     as the session sees it. No effect is claimed: a completed hunt is not a count of kills.
   - **One run's controls.** Each desk now hands back the controls of the run it started
     (`Underway`, `Steering`), so an agent's stop never stops a hunt the player began since
     (`crates/cena-behavior/tests/travel_desk.rs`, `an_old_walks_stop_...`, which the desk's
     own stop fails).
   - **The approval rechecked at execution** (issue #19, point 5): an approved `control` on an
     operation that has since ended is not done, and the agent is told so.
   - Mutations: the request-id lookup forgetting everything turns two tests red; the desk's
     own stop in place of a run's turns the steering test red. The steering test's first
     version passed that mutant too, because the second walk had arrived before the stop:
     rewritten so it is still walking.
   - **A lowered level stops nothing that runs.** It takes the agent's control away; the
     player's own stop ends the operation. (Issue #19's hard revocation is step 5's.)

   **3b BUILT 2026-09-27**: `hold`, `resume` and `retreat`, a hunt's alone
   (`crates/cena-behavior/src/hunt/steer.rs`).
   - **Hold**, as the author chose it: every arm above the fighting ones acts as always
     (incidents, survival, the group, a rest the character needs, flee), and **the creature
     already being fought is fought on**, the author's words being *no new target*; nothing
     else is begun: no target, loot, buff, wander, return to the hunting rooms, or walk back
     from a rest. A rest that finishes while held stays in the resting room and is counted
     once, when resumed. Issue #19 point 5 asks that hold say which survival actions remain
     active: that list is it.
   - **Retreat** wins over a hold and over a rest in progress: the target dropped, the walk to
     the resting room as a rest walks it (fog, waypoints), and `Ending::Retreated` there
     without resting, selling or healing. Its operation reads `interrupted`, `retreated`: a
     safe return is not a finished hunt (issue #19, point 3).
   - **Read once a tick, by the driver.** The run's controls (`Steering`) are shared with the
     machine; `Hunt::heed`, called by the driver before each tick, takes them in, so
     `Hunt::tick` still reads only what it is given.
   - The operation's lifecycle says what was asked (`held`, `retreating`); its result says
     what happened. A heal, keep, waggle or walk refuses them in words; a hunt asked before
     its run has begun says so.
   - Tests: `crates/cena-behavior/tests/hunt_steer.rs` (five, one driven through the real
     controls and `heed`), the lifecycle in `crates/cena-session/tests/agent_operations.rs`,
     a walk's refusal over MCP. Mutations: without the hold arm two tests go red; without the
     held rest, one; without the retreat, one.

   **3c BUILT 2026-09-27**: progress and its absence (issue #19, point 6; §4's third
   escalation signal, "no progress").
   - A behavior reports `Progress` through the `Reporter` its operation is started with
     (`crates/cena-session/src/operation.rs`): what it is doing, what it keeps count of, and
     `stalled` when nothing is coming of it. The hunt's (`crates/cena-behavior/src/hunt/progress.rs`):
     its phase (`held: ` when held), creatures engaged, rests, rooms searched; **stalled** when
     it is in the hunting ground, not held, and has engaged nothing for 300 game seconds,
     naming the rooms searched since. That is the case the watchdog cannot see: a loop that
     beats while it wanders empty rooms.
   - **Heard only when it changes** (the issue's "an unchanged incident does not cause
     repeated identical model requests"): an operation's `revision` counts its lifecycle,
     what it is doing and its stall; counts ride along unheard.
   - The run's `Steering` carries the progress (a `watch`), the driver reports after each tick,
     and the binary's performer forwards it for as long as the run goes on.
   - **Not built**: a walk's progress; §4's other two signals (a status Hunt does not react to,
     which the agent already sees as a `status` happening; a move Hunt did not make); and the
     issue's recovery states and attribution of an unexpected move. Each is named here so it
     is found when its case arrives.
   - Tests: `a_hunt_says_when_it_gets_nowhere` (`crates/cena-behavior/tests/hunt_steer.rs`) and
     `progress_is_heard_when_it_changes_and_not_otherwise`
     (`crates/cena-session/tests/agent_operations.rs`). Mutations: every report heard turns the
     second red; a stall that ignores a hold, the first.
4. **Commands**, with the denylist. **BUILT 2026-09-27.**
   - **The fifth level, `commands`**, and `command`: one line to the game through the same
     queue as the player's typing, as **`Origin::Agent`** -- queued as `Script` and `Trigger`
     are, never attendance, never preempting (AUTHOR 2026-09-24: an agent's single command
     interleaves). It was listed under step 5; sending needs it now, so it came now.
   - **A game command is an operation**: its result is the round trip -- `answered` when the
     game sent anything before its next prompt (the matcher typed input uses), `no_answer`,
     the session's refusals (roundtime, stunned...) -- never whether the line did what was
     meant (LAB: *"Report sent-but-unverified separately from evidence-backed success"*). The
     tool waits for the answer and hands back the game's text that came meanwhile.
   - **`text`** (Observe, §6's): the game's lines as viewers see them (`Event::Line`, M8's),
     the last 500 kept per character, untrusted, and hidden while the level forbids reading
     as every happening is.
   - **The denylist** (`crates/cena-session/src/agent/denylist.rs`) is LAB's, at `016bcc9`
     (`src/lich_agent_bridge/actions.py`: `_FORBIDDEN`, `evaluate`), checked at the door
     before the level, so a denied line is never asked of the player. **Two holes closed,
     each on a documented case** (the author's rule: from a real case): `put` with no
     container drops (`reference/wiki_clean/Verb_DROP.txt`: `>put my topaz` answers *You
     drop a clear topaz.*), and verbs are abbreviated (`Verb_EXPERIENCE.txt`: *"commonly
     abbreviated to 'EXP'"*), so a first word that begins a denied verb is denied, save the
     six directions that do. `;` is denied anywhere, as LAB denies it, though only a leading
     symbol is Hydra's.
   - No write-time gate: an agent's line goes as the player's does, and the game answers a
     line sent in roundtime with its own `...wait`. `Gate::Act` is a behavior's, for actions
     whose target it chose.
   - Tests: the denylist's own (LAB's list whole, the two holes, what is not on it), and over
     MCP a command answered with its text, five denied lines sending nothing, the one line
     sent as `agent`, and a command approved below the level. Mutations: the denylist
     skipped at the door lets `drop sword` out; sent as `Manual`, the origin test fails.
5. **Takeover.** `Origin::Agent`, `take_over`, and the level dropping after a bad run.
   **BUILT 2026-09-27** (the author approved the rest of the steps: *"I approve the rest of the
   steps"*). `crates/cena-session/src/agent/takeover.rs`.
   - **The sixth level, `takeover`**, and `take_over`, an operation: it stops what runs -- the
     binary's behaviors through its performer's new `halt`, then the session's own preempt,
     which takes the authority from a holder that does not let go -- and claims the authority
     under the agent's own token (`AuthorityToken(7)`). While it holds, no behavior can start,
     and the agent's lines are the holder's: `Origin::Agent` now carries the token it holds,
     so they queue as a behavior's do and are refused, never sent, once the authority is
     taken back (issue #19 point 5's *"revoke while an action waits ... no old send occurs
     afterward"*). The player's typing still goes first.
   - **Each ending its own** (point 5's *"graceful retreat and hard revocation produce
     different, explicit results"*): `released` by the agent; `revoked` by the player's new
     **`;agent stop`**, which also stops every running agent operation and takes the authority
     back at once, synchronously, before the takeover notices; `level_lowered` the same;
     `owner_idle` after five minutes without the agent touching the character (point 5's
     *"bounded owner liveness"* and *"model hangs do not block status or stop"*);
     `disconnected`, `session_ended`, `dead`. One takeover at a time (*"two clients cannot
     both own takeover"*). Nothing resumes by itself.
   - **A run that ends badly drops the level** to Observe: an agent's behavior or takeover
     ending `dead`, `disconnected`, `wedged` or `trouble`. `wedged` is new:
     `BehaviorError::Wedged`, the watchdog's preemption, which read as a plain stop until
     now. The binary keeps every level change in the settings file, so the drop survives a
     restart.
   - Not bound to one MCP client: Hydra has one agent token, so "which client" is the token's
     holder; a second client with the same token is the same caller.
   - Tests: `crates/cena-session/tests/agent_takeover.rs` (three, against a running scripted
     game with time paused: held, refused, sent as the holder's, released; revoked at once;
     a lowered level, five idle minutes, a death that drops the level). Mutations: the
     player's stop waiting for the takeover to notice turns two red; the holder's line sent
     without its token, one; no level drop, one.
6. **Acceptance, live, author present:** a hunt ends on its rest threshold, the agent (not
   the hunt) decides what is next and does it; the player stops the agent mid-act.

### Issue #19: LAB's author's review, 2026-09-27, and where each point lands

`https://github.com/Nisugi/cena/issues/19` (therealatari) reviewed this plan against LAB's
recent work. Its eight points, as taken:

| # | Point | Taken | Where |
|---|---|---|---|
| 1 | observation completeness and replay | yes: a list not stated is `null`; capture time; `changed` rebuilds state between reads; `lagged` forces a resync. Per-fact freshness is model work, later | step 1, BUILT |
| 7 | records' evidence envelope | yes: schema version, when written, whether recording; `ATTACH` refused (it could open another file); table descriptions later | step 1, BUILT |
| 2 | current eligibility, apart from the catalogue | yes, but not opaque offers: Hydra re-checks target, profile revision and authority revision at send time (the write-time gate already checks the target) | step 3 |
| 3 | results that separate work, evidence and recovery | yes | step 3 |
| 4 | a caller's request id, so a lost reply cannot admit twice | yes | before the first mutating tool |
| 5 | permission apart from ownership; exact, expiring, single-use approvals; hold, retreat, stop and revoke distinct | yes | steps 2, 4, 5 |
| 6 | progress, and its absence, as events | yes; §4's third escalation signal made concrete | step 3's acceptance |
| 8 | one contract for agents and scripts | the core and meanings, yes. Send-and-wait for **scripts** keeps Lich's semantics on purpose (`plan/46` §3: a match after the cursor); an agent's operations report the behavior's typed result instead | throughout |

Its M7a-M7d staging is this section's steps; "M7b" stays the Ruby bridge (`plan/46` §10).

## 9. Questions

**Answered 2026-09-24** (§0): the takeover split stands; levels persist; the denylist starts
from LAB's; an agent's single command interleaves; in-game questions stay a door left open.

**Open:**

1. **The slot.** The author: *"I don't know when it should come, what's M8"*. `12` §8's M8
   is *"remaining behaviors; customization surface (per `11` -- highlights first, §6a.4)"*.
   Of the five behaviors `CLAUDE.md` names -- Hunt, Loot, Heal, Bounty, Travel -- Travel is
   built, Hunt is M6b, and the hunt's shares of Loot and Heal became M6c and M6d; what
   remains is Bounty, and the customization surface: highlights first (`12` §6a.4: *"the
   primary combat perception channel"*), then keybinds, macros and layouts through one
   settings chain (§6a.2, §6a.3). Neither milestone needs the other. PROPOSED: M7 first,
   because its first step is small and is useful the day it lands (§6, "Records").
2. **The three escalation signals (§4)**: a status Hunt does not read, a move Hunt did not
   make, no progress. PROPOSED, not yet seen by the author.
3. **`records` takes SQL (§6).** PROPOSED, on a read-only connection.
4. **The connection as a scripting surface.** The author, 2026-09-25: *"not the agent, the
   connection"*. Scripts in any language, through bridges, with Ruby first:
   [`plan/38-scripting-bridge.md`](38-scripting-bridge.md) (PROPOSED). It asks this plan
   for push beside `wait`, send-and-wait, lines, script output and stores (its §4).

`12` §8's M9, *"DragonRealms adapter"*, contradicts `12` §9d and the settled decision
against a `GameAdapter`, and should go from the table.

## 10. Taken from LAB, and left

| LAB | Here | Why |
|---|---|---|
| MCP, loopback Streamable HTTP, token never a tool argument | **taken** (§2) | |
| operation ids, watched by cursor | **taken** (§6) | |
| generation on every act, stale fails closed | **taken** (§6) | Hydra has the generation already |
| sent vs verified | **taken** (§6) | Hydra's `Sent` already says it |
| typed capabilities first, raw commands a separate grant | **taken** (§3) | the Behaviors/Commands line |
| propose, approve, dispatch | **taken** (§3) | the notice gets a yes |
| denylist even at full access | **taken** (§3) | |
| the dangerous grant ends with the session | **left** | the author chose persistent levels (§3) |
| hold / resume / retreat | **taken** (§4) | needs hunt support |
| a failed handoff blocks the next | **taken**, simplified (§4) | the level drops |
| survival is deterministic, never the model | **taken** (§4) | |
| game text is data, never policy | **taken** (§5) | |
| the model built in (Codex CLI, OpenAI, llama.cpp) | **left** | Hydra ships no model client (§0) |
| `lab.execute_code`: TypeScript in an isolated V8 | **left** | an embedded runtime; scripting is deferred (`CLAUDE.md`) |
| wiki mirror and passage retrieval | **left** | separate from M7; an agent can use a wiki tool beside Hydra's |
| per-lane ownership (movement, combat, inventory) | **left** | Hydra has one authority; no second caller asks for lanes |
| the Ruby bridge re-validating every instruction | **left** | LAB crosses a process boundary; Hydra's authority is inside the session |
