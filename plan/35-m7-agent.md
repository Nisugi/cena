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
   *Shown:* an MCP client connects and answers "what is my character's status", live,
   author present -- and a question of the author's about the database that no report
   answers yet.
2. **Levels and notices.** The setting, the check, refusal as a notice with an approve link,
   `because` in the player log, the badge and the level control in Despana.
3. **Behaviors.** `perform`, `operation` and `control` over Hydra commands; behavior endings
   as events. Hold and resume need building in hunt first.
4. **Commands**, with the denylist.
5. **Takeover.** `Origin::Agent`, `take_over`, and the level dropping after a bad run.
6. **Acceptance, live, author present:** a hunt ends on its rest threshold, the agent (not
   the hunt) decides what is next and does it; the player stops the agent mid-act.

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
