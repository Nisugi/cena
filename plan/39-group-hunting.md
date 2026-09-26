# 39 — Group hunting: bigshot's head and tail, measured and staged

**Status: PROPOSED 2026-09-25; ten of §7's eleven questions ANSWERED 2026-09-26 (§8).
Stages 0 and 1 are built.** The author asked for *"all of bigshot"* before M6's live run (2026-09-25),
and has several characters and a test server to run a group on. `plan/30` §4 recorded the
author's group design and §8 put groups after M6; **the author moved them before the live run**
(§8, question 1). This plan measures what bigshot's group does, maps it onto Hydra's sessions,
and stages it. `plan/12` wins any contradiction.

Labels: **AUTHOR** is quoted and dated. **MEASURED** gives the line or the command.
**INFERRED** is reasoned from code that was read. **UNVERIFIED** is game behavior that no
source here records. Everything else is **PROPOSED**.

---

## 0. Measured

`reference/scripts/scripts/bigshot.lic`, clone `7fd0b97` (2026-09-17), version 5.16.1
(`:11`), 10,246 lines. Line numbers below are the clone's. Commands run in that directory.

| What | Where | Size | Measured by |
|---|---|---|---|
| `Event`, the order vocabulary | `:741-786` | 31 types | `awk 'NR>=754 && NR<=758' bigshot.lic \| grep -oE ':[A-Z][A-Z_0-9]+' \| sort -u \| wc -l` |
| `Group`, the leader's DRb object | `:924-1296` | 373 lines, 38 methods | `awk 'NR>=924 && NR<=1296 && /^    def /' bigshot.lic \| wc -l` |
| `head`, the leader's start | `:9906-10002` | 97 lines | |
| `tail`, the follower's loop | `:10003-10242` | 240 lines | |
| calls on `@followers` | 95 lines | 99 calls, 15 methods; `add_event` 48, `all_present?` 22 | `grep -o '@followers\.[a-z_]*' bigshot.lic \| sort \| uniq -c` (16 names, one is `nil?`) |
| orders the leader raises | 48 sites | 26 types; `FOLLOW_NOW` at 14 | `grep -oE '@followers\.add_event\(:[A-Z_0-9]+' bigshot.lic \| sort \| uniq -c` |
| orders the tail handles | `:10104-10230` | 30 types: all but `STAY_QUIET`, never raised or handled | `grep -E 'event\.type == :[A-Z_0-9]+' bigshot.lic \| grep -oE ':[A-Z][A-Z_0-9]+' \| sort -u \| wc -l` |
| lines that branch on a role | §0d | 21 | `grep -c 'solo?\|leading?\|following?' bigshot.lic` gives 25; 4 are the definitions |
| waits on followers | `:7241` ... `:9467` | 20, **none with a deadline**; 2 break early for a wounded member (`:7515`, `:7532`) | `grep -cE "(sleep ?\(?0\.5\)? while\|wait_while\|until\|while) *\(?\{? *!?@followers\." bigshot.lic` |
| `Group` methods nothing calls | | 12 of 38 | a grep per method over `reference/scripts/scripts/*.lic` |

### 0a. How the leader drives the followers: DRb, not LNet

MEASURED: `grep -ci "lnet\|bigshot_followers" bigshot.lic` is 0. The transport is DRb,
Ruby's remote objects (`require 'drb'`, `:576`).

1. **`;bigshot head [N]`**: `group open` (`:9908`), serve a `Group` (`:9912`), wait up to
   60 s for the game's group to reach N (`:9927-9981`), then whisper `... rallying at <uri>`
   to the group (`:9990`) until everyone has registered, count down 3 s (`:9999`), `lead`.
2. **`;bigshot tail`**: hook the stream for `rallying at (.*)\."` (`:10014-10019`), or take
   `link <uri>`; register with `group.add_member(bs)` (`:10053`), up to 60 tries 0.5 s apart
   (`:10008-10009`).
3. **The leader asks.** Every `Group` question is a synchronous remote call on each follower,
   through `member_online` (`:954-966`), which drops a follower whose call raises.
4. **The leader orders.** `add_event` pushes an `Event` onto each follower's stack
   (`:1049-1053`, `:2872-2882`); the tail loop takes one at a time. An `ATTACK` is dropped
   when the stack holds more than five (`:2874`), and skipped when stale: raised in another
   room, or more than 15 s ago (`:781-783`, `:10137`).
5. **Roles are three readings of one variable.** `solo?` is a leader whose group is empty
   (`@followers.size == 1`, `:7199-7202`); `following?` is `@followers.nil?` (`:7210-7212`);
   `leading?` is `!following?` (`:7205-7207`). On a follower every `@followers.x` returns nil
   instead of raising, because Lich answers any method on nil with nil
   (`reference/lich-5/lib/common/class_exts/nilclass.rb:9-11`). MEASURED. So solo is a kind
   of leading, and a follower runs the leader's code with its group calls silently void.
   INFERRED.

### 0b. The settings

Seven keys under "MA Grouping" (`:3539-3546`), `quiet_followers` under "Monitoring"
(`:3548-3551`), and two elsewhere. The first eight are read only on the leader's code paths, so
**the leader's profile decides them** (the `ma_looter` tooltip says so, `:2199`);
`troubadours_rally` and `disable_commands` are read in `attack`, which a follower runs too, so
**each member's own profile decides those**. INFERRED from where each is read.

| Key | Default | Read at | Does |
|---|---|---|---|
| `independent_travel` | false (`:3540`) | `:7238-7263`, `:7291` | on: the leader disbands and waits for the game group to empty; followers walk to the rally rooms, then the hunting room, alone. Off: the leader walks with a gather after each rally room |
| `independent_return` | false (`:3541`) | `:7478-7496` | on: followers get `PREP_REST`, `LEAVE_GROUP`, `FOG_RETURN`, `GO2_WAYPOINTS`, `GO2_RESTING_ROOM`; the leader disbands. Off: unhide, pull a sitting member by id up to three times (`:7500-7508`), a gather after each waypoint unless someone is wounded (`:7511-7520`) |
| `group_deader` | false (`:3542`) | `:3929-3935` | a grouped leader pauses on a dead group member. bigshot's plain `deader` (`:3921`) is any dead player, leader or solo only |
| `ma_looter` | nil (`:3543`) | `:7109-7112`, `:7125` | the looter, matched as a regex against the group's names; checked **before** `never_loot`, so a named looter wins over it. INFERRED |
| `never_loot` | [] (`:3544`) | `:7118`, `:7130` | exact names that never loot |
| `random_loot` | false (`:3545`) | `:7115-7127` | the member with the most headroom under its own encumbrance limit (`:1240-1249`, `:8798-8801`); a tie goes to `ma_looter`, else at random. Off: the leader, unless on `never_loot` (`:7130-7131`) |
| `final_loot` | false (`:3546`) | `:9435-9438` | the leader loots the room before leaving it; solo too |
| `quiet_followers` | **true** (`:3551`) | `:7525-7549` | at the resting room the leader runs its own resting prep and scripts first, the followers after: *"Prevents you from looking like you have mindless bots following while selling loot"* (tooltip, `:2214`). Skipped when someone is wounded |
| `troubadours_rally` | false (`:3472`) | `:6698-6712`, `:7786` | before each routine command, with 1040 known: cast it on self when webbed, asleep, stunned or frozen, and on the first group member here showing an ailment |
| `disable_commands` | [] (`:3515`) | `:7166-7168` | a fried member of a group runs this routine instead of its own (tooltip, `:2141-2143`) |

**Not a setting:** `HELP_GROUP_KILL` is written by `group_assist` (`:1083-1087`, `:8819-8822`)
and read nowhere; bigshot's own comment at `:7572` asks *"this doesn't seem to do
anything?"*. MEASURED: `grep -n HELP_GROUP_KILL` finds `:2724`, `:3255`, `:8821`.

### 0c. What the group decides together

- **Rest.** `should_rest?` (`:9046-9077`) merges each follower's `ready_to_rest?`
  (`:9007-9041`) with the leader's. **Fried rests the group only when every member is fried**
  (`:9060`). A wounded rest waits while a group member here is stunned (`:9065-9067`,
  `group_member_stunned?` `:6717-6730`). A rest for dread, bounty, fried, mana or encumbrance
  loots once more first; a wounded one leaves at once (`:9071-9073`).
- **Hunt again.** `should_hunt?` (`:8979-9002`): the leader's `ready_to_hunt?` (`:8947-8974`)
  and every follower's. The leader shows each member's reason while it waits (`:7577-7593`).
- **Loot.** Only the leader decides (`:7809`), only when no follower is still looting
  (`:7813`), and names the looter by `ma_looter` (`:7104-7135`); a follower named gets
  `PREP_REST` and `LOOT` (`:7842-7845`). The leader waits for the looting to finish before
  it leaves a room or rests (`:7466`, `:9431`).
- **A lost member.** The leader drops a follower whose DRb call raises (`:954-966`) and hunts
  on. A follower whose loop raises, a lost leader included, leaves the game group, walks to
  its own resting room and stops (`:10231-10240`). **There is no handover.** MEASURED.

### 0d. Where a role changes what bigshot does

| Line | In | Gate | What changes |
|---|---|---|---|
| `:3921`, `:3929` | `check_for_deaders_prone` | `leading?`; `!solo? && leading?` | only a leader pauses for a dead player; `group_deader` for a dead member |
| `:7106` | `ma_looter` | `solo?` | solo loots itself |
| `:7166` | `find_routine` | `!solo? && fried?` | `disable_commands` |
| `:7230`, `:7573` | `pre_hunt`, `rest` | `!solo? && leading?` | `group_assist(true)`: no effect (above) |
| `:7247`, `:7266`, `:7526`, `:7552` | `pre_hunt`, `rest` | `unless solo?` | `disband group`; `group open` |
| `:7270`, `:7529`, `:7555` | `pre_hunt`, `rest` | `!solo? && leading?` | unhide, so followers can find the leader |
| `:7300` | `pre_hunt` | `!solo? && leading?` | at the hunting room: group open, gather, pull or pause for deaders, signs, wait until sneaky followers hide and roundtime clears (`:7300-7333`) |
| `:7391`, `:7773` | `do_hunt`, `attack` | `!solo? && leading?` | a follower missing, or the game's group broken: call them back; between fights wait for them, mid-routine without stopping |
| `:7809` | `need_to_loot?` | `leading?` | only the leader decides to loot |
| `:8573` | `should_flee?` | `!leading? && checkpcs.empty?` | a follower alone in a room leaves it |
| `:9090` | `add_overkill` | `!solo? && leading?` | `FOLLOWER_OVERKILL`: followers count the extra kill |
| `:9312` | `prepare_for_movement` | `leading? && !solo?` | `PREP_REST` or `FOLLOW_NOW`, then wait out the followers' roundtime |
| `:9435` | `bs_wander` | `leading?` | `final_loot` |

Three more read the leader by name: `bigclaim?` is true for anyone but the leader (`:7094`),
so followers never check the room's claim; `attack` breaks off when the leader has left a
follower's room (`:7781-7784`); a follower loots when `Char.name == @DESIGNATED_LOOTER`
(`:7838`).

### 0e. What the port should not copy

- **Twenty waits with no deadline** (table above), plus two on the game group emptying
  (`:7248`, `:7488`). A follower still registered but never arriving holds the leader for
  good. `plan/12` §5.5: every wait has a deadline.
- **The rest-prep barrier reads the last rest's answer.** `rest_prep_done?` (`:8767-8769`)
  stays true from the previous rest until the follower reaches `RESTING_PREP_COMMANDS`
  (`:10202-10203`), and the leader tests it right after queuing that order (`:7541-7560`).
  INFERRED from the code; eohunter found it and fixed it (`scripts/eohunter/group.rb`,
  `Hub#broadcast`).
- **Dead weight:** 12 `Group` methods with no caller in the scripts clone (`group_size`,
  `leader_claim?`, `in_group?`, `has_bounty?` with its own *"Fixme: this won't work"*
  (`:1055`), `add_leader_event`, `do_command`, `do_put`, `client_do`, `pub_send`,
  `group_bandit_hunting`, `any_saturated?`, `emergency_rest?`); `STAY_QUIET`;
  `HELP_GROUP_KILL`.
- **"Rally" is three things:** the DRb handshake (`rallying at`), rally rooms
  (`rallypoint_room_ids`), and Troubadour's Rally (1040). Proposed words for Hydra: *muster*
  for gathering the group, *rally rooms*, and 1040 by its name; into the glossary
  (`plan/05` §8) when built.

---

## 1. The author's design

AUTHOR (`plan/30:238-244`, the answer to Q3, 2026-09-24):

> *"the ones still online would continue to fight and keep the room clear to keep it safe
> waiting for the reconnect. Eventually someone would have to take over and decide from
> there: if the disconnected one is out of game and x amount of time has passed, then take
> over and continue the hunt or rest if needed. If the disconnected one is still in game,
> then keep clearing the room, after x amount of time, everyone would need to leave the group,
> join back up on who is the new leader, the leader add the disconnected loligagger to their
> group, then go rest."*

`plan/30:246-248` adds that it needs a solo hunt that survives a reconnect first, and that the
acceptance hunt is solo (author, N4). Neither bigshot nor eohunter has a handover (§0c, §2):
this is the part with no code to port.

---

## 2. eohunter: the port that already exists

Hunt ports eohunter, the author's own (`plan/30:25`). eohunter ported bigshot's group as its
M3, **built, not live** (`C:\Gemstone\eohunter\docs\roadmap.md`, the M3 table; commit
`fd131b2`): `scripts/eohunter/group.rb`, 1,609 lines (`wc -l`). **Read it before bigshot**,
for the reason `CLAUDE.md` gives for reading VellumFE first: it is a working port of the same
rules by the same author, and the hazards live in the port.

- **It turned bigshot's calls around.** The leader serves a `Hub`; each follower pushes one
  `Report` per tick (the answers to all of the leader's questions at once) and pulls its
  `Order`s. The leader never makes a remote call, so a follower cannot stall it.
- **Liveness is silence:** a follower with no report for 10 s is lost, a leader with no
  heartbeat for 15 s is lost (`Hub::REPORT_STALE`, `HEARTBEAT_STALE`). Every order carries a
  hunt id.
- **A follower is the same engine with three behaviors swapped:** `Assist` for Engage (the
  leader's target first), `Follow` for Wander (to the leader's room, then `join`), `Orders`
  for Rest (the leader's rest, one step a tick). The leader adds `Muster` at priority 15:
  hold for a stunned member, a member in roundtime, a missing follower, the movement barrier.
  That is the row `plan/30:133` left out of M6.
- **One rule it added:** `fried_trigger`, "any", "all" or names (`Group::Policy`); bigshot
  hard-codes "all" (`:9060`).
- **Designed there, not built** (`docs/roadmap.md`): corpse recovery for a dead member, and a
  timeout on the movement barrier, where the author's stated intent is that *"the leader
  should go rest and wait rather than hunt on"* and *"nobody gets left behind"*.

---

## 3. One process: what changes

| bigshot, one Lich per character | Hydra, every member in one process |
|---|---|
| A DRb URI whispered to the group, a hook watching for it, 60 tries to register: 337 lines of `head` and `tail` | The leader's command names characters Hydra is already running (`crates/cena-host/src/table.rs`). No URI, no whisper, no registration. |
| The leader's questions are synchronous remote calls; a slow follower stalls the leader | Each member publishes a report every tick; the leader reads the latest and never awaits a follower. A wedged follower's report goes stale and blocks nothing, which is `plan/12` §5.5's isolation. |
| Orders are a queue, with a staleness rule, a flood cap and a rubber band re-sent from 14 sites | **The leader publishes its state instead**: phase, room, target, looter, which rest this is. A follower's tick reads it. The engine already re-derives intent every tick (`crates/cena-behavior/src/hunt/engine.rs`, module docs), so a follower needs what the leader is doing now, not the history of what it said. Staleness, the flood cap and eohunter's acks become numbered state, and the rest-prep race (§0e) cannot happen: a report says which rest it prepared for. |
| One signal of loss: a DRb exception (bigshot); silence (eohunter) | **Three signals, kept apart:** the session's lifecycle (`Reconnecting`, `Closed`: `crates/cena-session/src/lifecycle.rs`), the member's behavior ending or wedging (the desk's `HuntEnd`; the watchdog), and whether the character is still in the game (the other members' room rosters). The author's two branches, *out of game* and *still in game*, need exactly the first and third told apart. |
| Each script owns its character | Unchanged in shape: each member's hunt holds its own session's authority (`plan/12` §4.1; the hunt desk's token, `crates/cena/src/hunt.rs:45`). **The leader never sends on a follower's session.** Typing on a follower interleaves as it does now; a stop on a follower's page stops that follower. |
| Separate machines drop one at a time | **One network.** Losing it takes every member to `Reconnecting` together. The group is invalidated on a reconnect (`plan/12:441`; `crates/cena-model/src/state/reconnect.rs:221`) and must be formed again afterwards. The author's design covers one member lost; in one process the common case is all of them (question 9). |
| Followers move with the leader | Unchanged: the game carries a group along an ordinary move, and travel already waits at the crossings that do not (`crates/cena-behavior/src/travel/drive/deeds.rs:241`, `FOLLOW_WAIT`). INFERRED from those crossings and from bigshot's own walk home, which moves its followers with nothing but `FOLLOW_NOW` and a gather (`:7498-7522`). A leader's walk is travel's walk; a follower walks only to catch up (`group_all_followers`, `:9335-9347`). |

---

## 4. What Hydra already has, and the gaps

| Built | Where |
|---|---|
| The game's group, by `exist` id: members, leader, the join/leave/add/remove/lead lines; emptied when `IconJOINED` goes dark; cleared on a reconnect | `crates/cena-model/src/state/group.rs`, `state.rs`, `state/reconnect.rs` |
| The room's claim with the group subtracted, and a stranger's disk | `state/claim.rs`; the hunt's `hold_room` (`crates/cena-behavior/src/hunt/engine.rs`) |
| A group member sitting or prone is pulled up | `hunt/react.rs`, `players` |
| Group spell cooldowns | `state/cooldowns.rs` |
| Travel waiting for followers at a crossing | `travel/drive/deeds.rs` |
| Rally rooms, solo | `hunt/profile.rs` (`rooms.rally`), `hunt/import/rest.rs` |
| One hunt per session, a watchdog beside it; the engine pure, `tick(&GameState, Here, now) -> Said` | `hunt/desk.rs`, `hunt/engine.rs` |
| N sessions in one process, each with a handle and an observer | `cena-host` (`plan/29`) |
| A scripted game for driver tests, and two sessions in one test | `crates/cena-platform/src/answering.rs`; `crates/cena-behavior/tests/hunt_loot_drive.rs`; `crates/cena-session/tests/isolation.rs` |

**Gaps, MEASURED 2026-09-25** (in a working tree other agents are editing: the functions are
named so the lines can drift):

1. ~~**The hunt ends on a reconnect.**~~ **CLOSED 2026-09-25** (Stage 0 below). `fold_into`
   mapped `StateChanged(Reconnecting)` to `BehaviorError::Disconnected`, and the desk released
   the authority. SE-4 keeps the authority across a reconnect, but no hunt was alive to use it,
   and M6's own acceptance says *"A reconnect mid-hunt keeps the hunt"* (`plan/30:677`).
2. ~~**"Am I the leader?" has no answer.**~~ **CLOSED 2026-09-26** (Stage 1 below).
   `designates you as the new leader` and `You are leading` set the leader to `None`, which
   also meant nobody had said. The leader is now `Leader::Unknown`, `You` or `Other(Member)`.
3. ~~**Lines Lich reads that the model does not.**~~ **CLOSED 2026-09-26** (Stage 1 below).
   `Your group status is currently open|closed` (Lich's `STATUS`), `X's group status is
   closed` (the refusal `Group.add` reads), `X joins Y's group.`, `You have no group to
   disband.` (`reference/lich-5/lib/gemstone/group.rb:307-309`, `:509-525`), and the eight
   `HOLD_*_SECOND`/`_THIRD` the port had set aside. All are read now.
4. `react.deader` ends the hunt for any member; bigshot pauses the leader only (`:3921`).
5. Every group key with a value imports as *not imported*
   (`crates/cena-behavior/src/hunt/import.rs:118`). Troubadour's Rally is not built.

---

## 5. The shape (PROPOSED)

- **Who owns the group: `cena-behavior`, in a `group` module**, beside the hunt it belongs
  to. It holds the party's rules, pure: the looter, the merged rest and hunt decisions, who
  is present, each wait's deadline, the handover's clock. The binary (`crates/cena/src/`)
  resolves names to sessions through the host table and starts each member's hunt desk with
  its role. **No new crate edge:** `cena-behavior` already depends on `cena-session`, whose
  `SessionHandle` and `SessionObserver` are all a party needs (`ALLOWED_EDGES`,
  `crates/cena-arch-tests/tests/layering.rs`). Not in `cena-host`, which knows nothing of
  behaviors; not a new crate, for one behavior (`plan/05` §−1, rule of three).
- **The board.** One per group, shared by the members' desks. Per member, a
  `tokio::sync::watch` of its report: phase and rest reason, the reason it is not ready to
  hunt, room, roundtime, hidden and sneaky, looting, fried, encumbrance headroom, and the
  number of the rest it has prepared for. One `watch` of the leader's state: phase, room,
  target, the looter and its corpses, the rest's number, and the leader's rooms (hunting,
  resting, rally, waypoints: bigshot's `group.hunting_id` and siblings, `:1098-1116`).
  Writers never wait for readers, and a reader takes the latest (`plan/12` §5.5, *no shared
  lock on a hot path*).
- **The engine gains a role: solo, lead or follow.** `tick` stays pure: the board's snapshot
  is an argument, as `Here` is. Solo is unchanged. **Lead** adds Muster between Flee and
  Rest, merges the members' reasons into Rest, and picks the looter in Loot. **Follow**
  swaps Engage for Assist (the leader's target while it stands, else its own choice by its
  own ranks, `:10143-10176`), Wander for Follow (to the leader's room, then `join #<id>`,
  `:9335-9347`), and Rest for the leader's phase (prep and resting commands on the leader's
  rest number; walks only to catch up, or alone when `independent_*`); it loots only when
  assigned (`:10195-10199`). Survival, Flee, Maintain and the reactions stay each member's
  own. Each member keeps its own profile (routines, signs, thresholds, loot, heal) and takes
  its rooms from the leader.
- **How a leader's decision reaches a follower:** the leader's tick publishes; the follower's
  next tick reads it and sends on the follower's own session, through its own gate. Nothing
  crosses sessions except data.

---

## 6. Stages

Each ends green, committed, and demonstrable on its own. **§8 revises these with the
author's answers**; where the two disagree, §8 wins.

**Stage 0 — a hunt that survives a reconnect** (M6's own, named here because Stage 6 cannot
start without it). **BUILT 2026-09-25.** On `Reconnecting` the driver holds instead of ending
(`Driver::link_lost` in `crates/cena-behavior/src/hunt/drive.rs`: the state invalidated, the
target, held room and room forgotten, nothing sent); a walk the drop interrupts returns to the
loop rather than ending the hunt (`drive/walk.rs`); after `Ready` it takes the state as it now
is, with the room, hands and group unknown until the burst re-teaches them (`plan/12` §5.2),
and goes on. Test: `crates/cena-behavior/tests/hunt_reconnect.rs`, a scripted session dropped
mid-hunt, nothing written for the 60 s it is away, the target taken again after `Ready`. Two
mutations KILLED: the drop ending the hunt, and the hunt sending while away.

**Stage 1 — the model's group gaps** (`cena-model`). Leading as a three-valued fact; group
open or closed; the closed refusal; `X joins Y's group`; `no group to disband`; the `HOLD`
lines. Tests on wire lines, and one fixture cut by the author from a group session (the corpus
is the author's to cut). **BUILT 2026-09-26, but for the fixture.** *Am I the leader* is
`Group::leader()`, a `Leader`: `Unknown` (a new session, and after a reconnect), `You`
(`designates you`, `You are leading`, and an emptied group, Lich's `:self`) or
`Other(Member)` (`crates/cena-model/src/state/group.rs`). The character's own `exist` id is
`Character::exist_id` (`crates/cena-model/src/state/character.rs`), from `<playerID>`, which
the parser now types as `Frame::PlayerId` rather than a `WindowHints` bag, for `EndSetup`'s
reason; a link that is you reads as `You` and is never a member. The new events: `Status`
(`Group::status()`, `None` until `group` says), `Refused` (`X's group status is closed`,
which deletes X, as `Group.add` does), `JoinedOther` (`X joins Y's group`, a member when Y is
ours) and `NoGroupToDisband`. Lich's `checked?` is `Group::checked()`: the `STATUS` line
sets it; joining a group, being added to one, having your hand taken and `no group to
disband` clear it; so does a reconnect. The eight `HOLD_*_SECOND`/`_THIRD` read as the lines
Lich handles them like: someone taking your hand as `AddedToGroup`, a third person's as
`LeaderAdded`. Tests: `crates/cena-model/tests/group_leader.rs` (14),
`crates/cena-model/tests/group_lines.rs` (25), and `crates/cena-model/tests/group.rs` (22,
updated for the three values). **41 hand mutations, all KILLED**: each new branch of the
leader, the id, the parser's `playerID`, the four new events, `checked`, and each of the eight
`HOLD` rows broken in turn.

UNVERIFIED, for the author's fixture to settle: whether the game sends the refusal's name as
a link (Lich reads it on stripped text; it is read here only as a link, as its sibling
`joins <Y's> group` is sent); whether `You are grouped with` lists you; that `STATUS` is the
last line of `group`'s answer (INFERRED from `Group.check` waiting on it,
`reference/lich-5/lib/gemstone/group.rb:152-157`); and `exist_id`'s rule,
`-(10,000,000 + playerID)`, VERIFIED on one character only (two fixtures cut from one log:
`crates/cena-protocol/tests/fixtures/login_burst.xml:3`,
`crates/cena-protocol/tests/fixtures/character_info.xml:3`). Lich stores the number and never
compares it with a link, so no source states the rule.

**Changed from Lich, because roles now read the leader** (§8, question 2): `X designates Y as
the new leader` set the leader whoever X and Y were, as Lich does
(`reference/lich-5/lib/gemstone/group.rb:632-635`), while `X adds Y` counts only when X is in
our group. If the game sends the swap to the whole room, as the model's comment says it sends
`adds`, a stranger's swap would make a stranger our leader, and the role would read *follow*.
The swap now counts only when X or Y is in our group, the guard Lich already puts on the
push beside it, and pushes both, as Lich does; `You designate Y` pushes Y (`:626`). Who
receives the swap is UNVERIFIED; the guard costs nothing if only the group does. Tests in
`crates/cena-model/tests/group.rs` (`leadership::`); five mutations KILLED (the guard
dropped, on X alone, on Y alone, either push dropped).

**Stage 2 — the party's rules, pure** (`cena-behavior/src/group/`). The report and the
leader's state; the looter (`ma_looter`, `never_loot`, `random_loot`, in bigshot's order); the
rest merge (the all-fried rule, the stunned hold, the last loot before a rest that is not for
wounds); the hunt merge; presence, per member: in the room, and in the game's group; each
wait's deadline. One test per bigshot rule, cited to its line, each with a mutation that turns
it red.

**Stage 3 — leader and follower on the engine, no game.** The role, Muster, Assist, Follow,
the follower's rest and loot on assignment, the overkill count. Demonstrated by two scripted
sessions, two desks and one board in one test, through a full cycle: the follower attacks the
leader's target, the named looter loots, both walk to rest, the leader waits for the
follower's prep **for this rest**, and both walk back.

**Stage 4 — the command and the hub.** The leader's command starts the group: `group open`,
and each follower `join #<leader's id>` (Lich's `Group.join`,
`reference/lich-5/lib/gemstone/group.rb:340-359`). One stop ends every member. The hub card
shows each member's role and the reasons bigshot printed (*"Dicate resting: fried."*).
**Live, the author present:** two characters on the test server, one per account
(`plan/29` §5 Q1), a full hunt-rest-hunt cycle.

**Stage 5 — the settings.** A `[group]` table in the leader's profile and the importer's rows
for it: independent travel and return, quiet followers, the three loot keys and final loot,
group deader, `disable_commands`, and Troubadour's Rally (1040) for self and group. A test per
key; `;hunt import` of a profile that sets them.

**Stage 6 — the author's handover.** A lost member is held for: the others keep the room clear,
with no wander and no walk to rest. After the wait, the successor leads. Out of game: take
over, hunt on or rest. Still in game: everyone leaves the group, joins the new leader, the new
leader adds the one left behind (`group #<id>`, as Lich's `Group.add` sends it,
`reference/lich-5/lib/gemstone/group.rb:298-309`), and they rest. The member who comes back
rejoins. Tests over scripted sessions with one connection dropped and the clock advanced, a
mutation per branch. **Live:** one member knocked off by a second login on its account, the
trigger `plan/29` §5b recorded.

**Not in this plan:** corpse recovery (designed in eohunter, not built; bigshot has none);
group bounties (`has_bounty?` is broken, `:1055`); working with a Lich bigshot over DRb
(eohunter does not either); bigshot's watch timers and display events, which become the hub
card.

**Cross-session control elsewhere.** `plan/35` §7 has the binary reach the session table for
multi-character tools, as the hub does, and checks an agent's level in the session
(§3). PROPOSED here: an agent starting a group needs Behaviors on every member it names.
`plan/38` §6b lets a bridge script name another character; it is not scheduled, and nothing
here depends on it.

---

## 7. Questions for the author

1. **When:** as M6f, before the live run (*"all of bigshot"*), or after it, as `plan/30` §8
   has it?
2. **Starting:** one command on the leader naming the followers (`;hunt ojandhaart with
   Dicate Kiyna`), or each follower typing its own, as bigshot's `head` and `tail`? From the
   hub too? One process makes the first possible; bigshot could not.
3. **Stopping:** does `;hunt stop` on the leader stop everyone? On a follower, does it leave
   the game's group, and does the leader wait for it, hunt on without it, or rest?
4. **Who takes over** when the leader is lost: a named second, the next in the roster, or
   something else?
5. **The x:** one wait or two (for the reconnect, then before the regroup)? Proposed: Hydra
   knows when a session gives up (`Closed`, after the reconnect ladder's cap), so *out of
   game* could start the takeover at once and *still in game* use a fixed wait. What value?
6. **Out of game or still in game:** is it the missing character's name in the others' room
   roster? How long a link-dead character stays in the room is UNVERIFIED, and so is whether
   `group #<id>` can add one, which the design's last step needs.
7. **A lost follower, not the leader:** hold and wait the same way, then rest with it grouped,
   or hunt on without it, as bigshot and eohunter both do?
8. **The one who comes back** after a handover: follows the new leader, or takes the lead back?
9. **Everyone drops together** (one network, §3): once all are `Ready`, re-form the group and
   carry on, or rest first?
10. **A dead member:** bigshot's `group_deader` pauses until the player says go; `react.deader`
    now ends the hunt. Which, and is eohunter's corpse recovery wanted in this port?
11. **`quiet_followers`:** keep bigshot's default of on?

---

## 8. Answers (AUTHOR, 2026-09-26), and what they change

Each answer is quoted as given. What follows it is PROPOSED unless labelled.

1. **When.** *"before"*. Groups are **M6f**, built before the live run.
2. **Starting.** *"I think support both?"* Both starts are supported. What makes that
   simple is question 8's answer, that passing the game's group lead passes the hunt's
   lead. **So a member's role is read off the game's group, never chosen:** leading a group
   with members is *lead*, in a group someone else leads is *follow*, anything else is
   *solo* (bigshot's roles are readings too, §0a item 5). `;hunt <profile> with A B` on the
   leader forms the group (`group open`, each named character `join #<id>` and starts its
   own hunt); `;hunt <profile>` typed on a character already in a group takes the role the
   game gives it. Each member hunts with its own profile (§5); which one a named follower
   runs, when the leader's command names only the character, is Stage 4's to settle. The hub
   start is Stage 4's too.
3. **Stopping.** *"if the leader does hunt stop and is still the leader of the group then
   everyone should stop and ride back to town with the leader unless the option to have
   followers travel indepenedently from bigshot was implemented and is enabled, then they
   would all get back to the rest room on their own. If the leader transfers group
   leadership to another member and then stops, then the group continues hunting with the
   new leader."*
   - The leader's stop, while leading, ends every member's hunt. Without
     `independent_return`, the followers stay grouped and go where the leader goes: the game
     carries a group along the leader's moves (§3). With it, each leaves the group and walks
     to its own resting room, as bigshot's lost follower does (`:10231-10240`).
   - INFERRED, to confirm: the leader's own stop stays immediate (`plan/12`'s stop
     contract), and *"ride back to town with the leader"* is the game carrying the followers
     while the leader's player takes it home. If the author means the stop itself walks the
     leader home, that is a different command from a stop.
   - Lead passed, then stop: the new leader's hunt leads (question 2's reading), and the old
     leader is now a follower whose stop is the next bullet.
   - A follower's stop (not asked directly): it **leaves the game's group**, and the rest
     hunt on without it, by question 7's *"Don't just continue hunting unless they leave the
     group"*.
4. **Who takes over.** *"a priority list or random if one not set up (the healthiest?)"*. A
   `successors` list in the leader's profile, first present member first. With none: the
   member with the most health, a tie at random.
5. **The wait.** *"60-120 seconds? While still keeping the room safe. Configurable ..."*.
   One key, `lost_wait`, default **90 s**. While it runs, the members still here fight what
   comes in and nothing else: no wander, no walk to rest (Stage 6 as written).
6. **In game or out.** Not answered: the author asked for the context the question lacked
   (*"how do you know their having problems? They stop responding? What stops responding?
   Their hydra info stops updating? They stop sending commands?"*). The context is below
   (§8a). The question is asked again there.
7. **A lost follower.** *"I think a lost follower depends ... are they incapacitated? In
   roundtime? why did they get lost or more accurately, why did they get left behind? If
   they are near by (you should know their room number) then probably go to them if they
   can't come to you immediately? Don't just continue hunting unless they leave the group.
   But if they leave the group because they died well ...."*
   **Muster learns why a member is not with the leader**, from the member's own state, which
   the board carries, and acts on the reason. bigshot and eohunter both drop a lost follower
   and hunt on (§0c, §2). The author's answer is not to.

   | Why the member is not with the leader | What the group does |
   |---|---|
   | in roundtime, stunned, webbed, prone or sitting, **in the leader's room** | holds and keeps the room clear; a sitter is pulled up (`hunt/react.rs`, as now) |
   | in another room, able to move | it walks to the leader's room and rejoins (Follow) |
   | in another room, **unable to move** (stunned, webbed, bound, and whatever else its own state says) | the leader walks the group to it, by its room id on the board (travel's walk); then as the first row |
   | its connection is lost | §8a |
   | dead | question 10 |
   | it left the game's group (its own stop, or a player's `leave`) | the group hunts on without it |

8. **The one who comes back.** *"if they come back, they'll have to find you, join your
   group again, ect. If we are allowing people to join up mid hunt, then probably let them
   follow unless they are given lead by the other person. So passing group lead is like
   passing the leader."* Joining mid-hunt is allowed: a member not in the group walks to the
   leader's room (on the board) and joins; it follows unless given the lead. The role rule of
   question 2 is this answer. **§4's gap 2, "am I the leader?", becomes the first thing
   Stage 1 builds**: every role now depends on it.
9. **Everyone drops together.** *"rest first. someone probably died."* Once every member is
   `Ready`: the group formed again, then the walk to rest, whatever the thresholds say.
10. **A dead member.** *"end the hunt, add the dead person to your group (have to empty
    hands), cast 130 to teleport away after adding them to your group, or drag them if you
    can't, if you can't drag them probably try to alert the human?"* This replaces bigshot's
    `group_deader` pause (§0b), and it is the corpse recovery eohunter designed and never
    built (§2). Every member's hunt ends. Then one member (the leader if able, else any
    member who can) takes the recovery:
    1. empties its hands, and takes the dead member's hand. That adds the dead member to its
       group: Lich's `HOLD_*` lines (`reference/lich-5/lib/gemstone/group.rb:468-505`), the
       eight that §4 gap 3 says the model set aside;
    2. casts Spirit Guide (130: `crates/cena-model/data/spells.tsv:30`) when it knows it and
       can afford it, and goes with the group;
    3. else drags the dead member;
    4. else alerts the player: the hub card, and the hub's merged feed.

    UNVERIFIED, for the stage that builds it: where Spirit Guide goes from a hunting ground,
    that it takes a held dead member along (the author's answer says it does), and `drag`'s
    command, conditions and messages. All three are game behaviour. Lich scripts and the
    corpus come before asking.
11. **`quiet_followers`.** *"sure, and toggleable."* On by default, a key in `[group]`.

### 8a. Question 6, with the context it lacked

Every member runs in the same Hydra process, so the group does not have to infer a lost
member from silence. bigshot infers it from a remote call failing (`:954-966`); eohunter
from 10 s without a report (§2). Hydra reads each member directly. A member can be "not
with the group" in five ways, and each has its own signal:

| What happened | How Hydra knows | Where |
|---|---|---|
| the member's **connection** dropped | its session publishes `Reconnecting` the moment the socket goes; its game state stops changing because nothing arrives | `crates/cena-session/src/lifecycle.rs:255` |
| Hydra **gave up** reconnecting it | `Closed`: after two losses with nothing sent between them, a fatal login error, or a stop | `lifecycle.rs:265`; `crates/cena-session/src/supervisor/retry.rs:69` (`MAX_UNATTENDED_LOSSES = 2`) |
| its **hunt is stuck**, the connection fine | the watchdog preempts a hunt whose loop has not turned in 30 s | `crates/cena-behavior/src/watchdog.rs:34`; `hunt/desk.rs:462` |
| it is connected and hunting, but **cannot act** (roundtime, stunned, webbed, dead) | its own game state; the others also see it in their room's player list (`who appears dead`, `stunned`, `lying down`) | `crates/cena-model/src/state/room.rs:97-114` |
| it is fine, but **somewhere else** | its own room id against the leader's | each session's room |

The first four are Hydra's side. **Question 6 is about the game's side of the first row.**
When Hydra's connection to a character drops, the game may keep the character standing in
the room for a while: link-dead, still attackable, still in the group. That is why the
author's design splits *out of game* (take over, hunt on or rest) from *still in game* (keep
the room clear, regroup, add the lost one, rest). Hydra's session says *our* connection is
gone. It does not say whether the game has removed the character. Two things could tell
the others:

- **The room's player list**: whether the lost member's name is still in it.
- **The logons feed**: `* <name> has disconnected.`, which Lich's `messaging.lic` reads as
  a logoff (`reference/scripts/scripts/messaging.lic:329`). Hydra carries the `logons`
  stream (`crates/cena-model/src/state/stream_windows.rs:41`), but nothing reads that line
  yet.

**Asked again:** is *"still in the room's player list"* the right test for *still in game*,
and *"gone from it, or `has disconnected` announced"* the right test for *out of game*? How
long does a link-dead character stand in the room, and is `has disconnected` sent when the
connection drops or when the game removes the character? UNVERIFIED by any source here. The
corpus could measure it: two of the author's characters in one room, one of them dropped.
That needs the author's leave to query.

### 8b. The stages, revised

- **Stage 1** (the model) adds: *"am I the leader"* as a three-valued fact, which needs the
  character's own `exist` id (question 8); the eight `HOLD_*_SECOND`/`_THIRD` lines
  (question 10); and, once question 6 is answered, `has disconnected` from the logons feed.
  **BUILT 2026-09-26** (§6), all but `has disconnected`, which still waits on question 6.
- **Stage 2** (the rules, pure) adds: the successor (`successors`, else the healthiest); the
  lost member's reason and what each reason asks (question 7's table); `lost_wait`.
- **Stage 3** (leader and follower on the engine) adds: the role read off the game's group
  (question 2); joining mid-hunt (question 8); walking the group to a member who cannot
  move (question 7).
- **Stage 4** (the command and the hub) adds: both starts (question 2); the leader's stop
  and a follower's stop (question 3).
- **Stage 5** (the settings) adds `successors`, `lost_wait` and `quiet_followers`.
- **Stage 6** (the handover) adds: the successor by Stage 2's rule, and *rest first* when
  every member dropped (question 9). It waits on question 6.
- **Stage 7, new: a dead member** (question 10): the hunts ended, the hand taken, Spirit
  Guide, the drag, the alert. It is tested over scripted sessions, one branch per step, a
  mutation each. It is live on the test server, where the author can make a character die.
