# 27a — `GameState`: the 36 fields

The spine. Every other document in the `27` set describes something reachable
from here.

Read [`27`](27-model-api.md) first for what these documents are and why the
reconnect column exists.

MEASURED 2026-09-22 against `crates/cena-model/src/state.rs`:
**36 fields, 26 of them `pub`**, and `apply` dispatches **30 `Frame` variants**.

```
grep -E "^    pub [a-z_]+:|^    [a-z_]+: " crates/cena-model/src/state.rs | wc -l   -> 36
sed -n '/pub fn apply/,/^    }$/p' crates/cena-model/src/state.rs \
  | grep -oE "Frame::[A-Za-z]+" | sort -u | wc -l                                  -> 30
```

---

## How to read the table

**Reconnect** is what `invalidate_for_reconnect` does between generations. Its
test is *what cannot have changed while we were away, or what a command taught*
— **not** "what the login burst re-sends", which was an earlier formulation and
is recorded in `state/reconnect.rs` as wrong.

- **cleared** — reset to `Unknown`/empty. Never `0` (`plan/12` §5.2).
- **kept** — survives the generation, with the reason given.
- *italic* — not a game fact; scaffolding or a session-level counter.

---

## Position and identity

| Field | Wire source | Read by | Reconnect |
|---|---|---|---|
| `room` | `Frame::RoomId`, `Component{room desc/objs/players}`, `Compass`, `RoomMeta` | Despana, travel, overwatch, creatures | **contents cleared**, via `forget_contents` — who is standing there is not durable |
| `arrivals` | *derived*: incremented per arrival | `fog::moved_from`, "did we actually move" | **kept** — *a session counter, not a game fact.* Resetting would make the first arrival after a reconnect compare equal to a count taken before: the exact false "did not move" it exists to prevent. Wraps, so growing across generations is free |
| `character` | `Component`s, `Label`s, command output (`info`, `skills`, `experience`, `profile`) | behaviors, Despana, the store | **partly** — `character.invalidate_for_reconnect()`; stats and identity are **kept**, being command-taught |
| `prompt` | `Frame::Prompt` | Despana's input line | **cleared** |

## The clock, and everything gated on it

| Field | Wire source | Read by | Reconnect |
|---|---|---|---|
| `roundtime_ends` | `Frame::RoundTime` | the queue's gate, Despana's countdown | **cleared** — `Option`, never `0`; `0` is a real epoch second |
| `cast_time_ends` | `Frame::CastTime` | the same gate | **cleared**. Separate from `roundtime_ends` as it is in Lich (`xmlparser.rb:770`): a cast time and an action roundtime run at once and expire independently |
| `game_time` | `Frame::Prompt`'s `time=` | `game_time_now()`, every gate | **cleared** — a carried clock reports a server time in the future |
| *`game_time_received`* | *local `Instant` at receipt* | `game_time_now()` only | **cleared**. *Excluded from `PartialEq`*: two replays of one recording differ by microseconds, which broke `replay_determinism` when `PartialEq` was derived |

> **`game_time_received` is a `std::time::Instant`, and that has bitten twice.**
> Tokio's `start_paused` does not control it, so a test pinning an exact server
> second to an extrapolation from it fails on wall-clock boundaries — recorded
> as an intermittent flake for a day before being measured
> (`cena-session/tests/send_now.rs`).

## Vitals and status

| Field | Wire source | Read by | Reconnect |
|---|---|---|---|
| `vitals` | `Frame::ProgressBar` | Despana, heal behaviors | **kept** — MEASURED as re-sent in every burst, so clearing opens a window where health reads `Unknown` for no reason |
| `status` | `Frame::StatusIndicator`, and text-derived statuses | behaviors, Despana | **split, and the split is the point.** Indicator-derived: **kept** — the burst re-declares all ten in one line. The six text-derived ones (`afflictions.rs`: `silenced`, `bound`, `calmed`, `cutthroat`, `sleeping`, `thorned`): **cleared**, because **no indicator exists for them**, so nothing would ever correct a stale one |
| `effects` | `Component{active spells}` | buff behaviors | **kept**, *and this was corrected*: it used to clear, citing "spells tick down in real time". They do not tick while the character is out of the world, and the burst re-declares them |
| `injured` / body | `Frame::InjuryImage` | Despana, heal | via `character` |
| `idle_warning` | text | the session | **cleared** — *a fact about the connection that just ended* |

## Hands and inventory

| Field | Wire source | Read by | Reconnect |
|---|---|---|---|
| `left_hand`, `right_hand` | `Frame::LeftHand`, `Frame::RightHand` | Despana, every behavior that holds something | **kept** — *"nothing empties them because a socket dropped"*, and the burst sends real contents (`<left exist=…>plain gift`, `<right>Empty`), not placeholders |
| `inventory` | `Frame::Container*`, `InventoryManager`, `InventoryViewItem` | loot, stow | **cleared** — container CONTENTS are not re-sent, and Lich drops them for the same reason (`inventory.rb:1014`) |
| `inventory_snapshot` | `Component{inv}` | Despana, worn-item questions | **kept** — a logged-off character gains and loses nothing, and it is point-in-time by contract either way |
| `containers` | `stow list` / `ready list` **only** | stow, ready, loot | **kept** — and the failure avoided is silent: *neither list is re-sent by the burst*, so clearing would leave a behavior with no stow container and no event ever coming to restore one |
| `bank` | `bank account` **only** | bank behavior | **kept** — silver on deposit is not connection state; nobody spends it while logged off |

## Other people, and claims about them

| Field | Wire source | Read by | Reconnect |
|---|---|---|---|
| `group` | `Component{group}` | group behaviors | **cleared** — the roster is connection state |
| `cooldowns` | text (spell lockouts, PR #1597) | buff behaviors | **cleared**, and *the contrast with `containers`/`bank` is the point*: those are facts about you that nothing changes while you are gone; these are **stamps on other people, taken against a clock this session was keeping**, and both assumptions break at once |
| `messages` | `preset id='speech'`/`whisper` on text | log, Despana | **kept** — a message is a record that something WAS SAID, not a claim about now |
| `creatures` | `Frame::CreatureStatus`, `Component{room objs}` | hunt, overwatch | **partly** — what combat did to a creature is still true of it; who is in the room is not |
| `overwatch` | text | the hiding inference | **cleared** |
| `targeting` | `Frame::DialogWidgets` `dDBTarget` | hunt, "what can I attack" | **cleared** |

## Streams and text

| Field | Wire source | Read by | Reconnect |
|---|---|---|---|
| *`streams`* | `Frame::Text`'s `stream` | Despana, the player log | **cleared** — *"half a sentence nobody will finish; the next connection's bytes are not a continuation"* |
| `stream_windows` | `Frame::StreamWindow`'s `ifClosed`/`styleIfClosed` | Despana's routing, the speech duplicate | **cleared** — MEASURED: 15 of 16 declarations arrive before the first prompt. A stale `ifClosed` **drops** text rather than showing it stale, which is why this one is not worth keeping |
| *`pending`*, *`chunk`* | *assembly state* | the chunk classifier | **cleared** |
| `unknown_tags`, *`unknown_tag_counts`* | anything the parser cannot model | criterion 8's evidence, Despana diagnostics | **kept** — a tag the parser could not model is a fact about the **protocol**, most useful across a reconnect rather than least |
| *`tally`* | *counters* | diagnostics | **kept** — *process behaviour, not a game fact* |

## Features

| Field | Wire source | Read by | Reconnect |
|---|---|---|---|
| `objectives` | `Frame::ObjectivesUpdate` | Despana, bounty | **kept** |
| `bounty` | text (`bounty.rs` + `ebounty.lic`'s messaging) | bounty behavior | **cleared** |
| `known_spells` | `Component{spells}`, link `noun` per spell | cast behaviors | **cleared** |
| `maneuvers` | text refusals only | PSM behaviors | **cleared** — the game never states a duration, so this records *"it said this was cooling, at this server second"* and nothing more |
| `learned_commands` | `Frame::CmdListUpdate` | menu resolution | **kept** — what a menu coordinate MEANS is a fact about the game, and the push is not repeated on reconnect |
| `combat` | the chunk, via the tracker | the recorder, hunt, the player log's `main/combat` tag | **partly** — a held cast belongs to a chunk the old connection never finished |

---

## The two fields excluded from `PartialEq`

`state/equality.rs` hand-writes the impl so that **adding a field is a compile
error** and whoever adds it must choose a side. Two are excluded:

- *`game_time_received`* — unreproducible across replays, as above.
- *`tally`* — a count of what this process did. *"Two states that know the same
  things are equal whether or not one has been running longer."*

Everything else, including `stream_windows`, is compared: a `streamWindow`
declaration is something the server said, and a replay of the same bytes
declares the same thing.

## What `apply` does NOT handle

30 variants reach it. The parser emits more — `StreamPush`, `StreamPop`,
`StreamResume` and the prompt barrier need **no handling here at all**, because
the parser has already stamped each `Frame::Text` with its stream by the time
the model sees it (`state/streams.rs`). They are still published, for a renderer
that wants to know a window opened.
