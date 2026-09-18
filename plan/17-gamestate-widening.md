# 17 — Widening `GameState`, from both references

**Status: DRAFT, unapproved.** This proposes a change to `plan/12` §7.1's In column, which is
the author's boundary to move. Written 2026-09-18 after Milestone 1 closed.

---

## 0. Why, in one paragraph

Every frame `plan/16` needs is **already parsed and already thrown away**:

| Frame | Parsed at | In `GameState`? |
|---|---|---|
| `StatusIndicator { id, active }` | `crates/cena-protocol/src/parser/thin.rs:109` | **no** |
| `Prompt { time, text }` | `parser/dispatch.rs:293` | text only — **`time` dropped** |
| `Buffs` / `Cooldowns` / `ActiveSpells` rows | `frame.rs:229` | **no** |

`plan/12` §7.1 scoped M1 to room, prompt, hands, roundtime and vitals. That was right for M1.
It is now the single bottleneck: instant actions, roundtime gating and confirm-by-effect all
wait on the model catching up to the protocol.

---

## 1. This is a PORT, not a design

> **AUTHOR, 2026-09-18:** *"lich is the source of truth. Vellum recreated a bunch of stuff from
> lich in Rust because it needed to for frontend stuff. So they are both good sources."*

Everything below is read out of `reference/`. Where the two disagree in emphasis, Lich says what
the wire **means** and Vellum shows what the structure **looks like in Rust**.

> **Recorded because it cost time.** Before reading either, I spent an hour of conversation
> designing a clock-skew calibration scheme — phase detection off prompt-second ticks, error
> bounds, the lot. Vellum solves it in six lines (§3). `CLAUDE.md` already said to read Vellum
> first; I wrote the equivalent rule for Lich earlier the same evening and then did not follow
> either. The cost was not wasted code — nothing was built — but it was the author answering four
> rounds of questions that `reference/` answers better.

---

## 2. Status indicators — port `StatusInfo`

`reference/VellumFE/src/core/state.rs:339-376`. A `BTreeMap<String, bool>`, and three decisions
worth taking whole:

1. **Ids are normalised**: `Icon` prefix stripped, lowercased, so callers may pass either form.
2. **Unknown reads `false`** — "the game never told us, so it is not happening."
3. **`is_known` exists separately**, distinguishing *reported inactive* from *never reported*.

Point 3 is `plan/12` §5.2's "`Unknown` is a first-class value" arriving independently at the same
answer, and it is exactly what a reconnect needs: after generation `n+1`, indicators are
unreported until the server says otherwise, and a confident `false` would be a lie.

`BTreeMap` not `HashMap` — deterministic iteration, which `crates/cena-arch-tests` already
prefers for criterion 7's sake.

**The eleven ids** (Lich `lib/constants.rb:72`, `ICONMAP`): `IconKNEELING`, `IconPRONE`,
`IconSITTING`, `IconSTANDING`, `IconSTUNNED`, `IconHIDDEN`, `IconINVISIBLE`, `IconDEAD`,
`IconWEBBED`, `IconJOINED`, `IconBLEEDING` — plus `IconPOISONED` and `IconDISEASED`, which
`plan/15` records as on-the-wire and absent from the wiki. Cena passes ids through verbatim, so
the list is documentation rather than a filter.

---

## 3. Server time — port `game_time_now`, delete nothing else

**This is the part I would otherwise have got wrong.** `reference/VellumFE/src/core/state.rs`:

```rust
pub fn update_game_time(&mut self, prompt_time: i64) {
    self.game_time = prompt_time;
    self.game_time_received = Some(std::time::Instant::now());
}

/// Server "now", extrapolated: the last prompt's timestamp plus how long
/// ago it arrived on the local clock. Timers keep flowing between lines.
pub fn game_time_now(&self) -> i64 {
    self.game_time + self.game_time_received.map(|at| at.elapsed().as_secs() as i64).unwrap_or(0)
}

pub fn in_roundtime(&self) -> bool {
    self.roundtime_end.map_or(false, |end| self.game_time_now() < end)
}
```

Two fields and three functions. Why it is right:

- **Both sides are in server time**, so clock skew cancels rather than needing correction.
- **`Instant`, not `SystemTime`** — monotonic, unaffected by NTP steps or DST.
- **It flows between prompts**, which is the author's onset/offset point: a prompt is only sent
  when something happens, so anything that waits for one to learn that roundtime ended waits
  forever.

`server_time_offset` (`core/messages/element.rs:866`) is a second, simpler thing:
`server_time - local_time`, recomputed on **every prompt**, used where a caller needs server
time from a local timestamp. No averaging, no calibration model — the latest sample, because
prompts are frequent and the offset barely moves.

Lag is sampled **periodically**, not per prompt (`LAG_CHECK_INTERVAL_SECS`), and kept separate
from the offset. Worth copying: lag is a diagnostic, the offset is load-bearing, and conflating
them makes the load-bearing one noisy.

### 3a. What `plan/16` §4 asked for, resolved

The clock-skew item can be closed. It was posed as "compare prompt time to our time and watch
for changes", and the answer is that you do not need to watch for changes at all: extrapolate
from the last sample and compare within one clock. Skew becomes an unused quantity, and lag
becomes a number to display rather than to correct for.

---

## 4. Effects — the shape, and the one thing Lich adds

Vellum has `data::ActiveEffect` and an `EffectCategory`. Lich adds the rule that neither the
wire nor a Rust struct states, in `lib/gemstone/infomon/status.rb`:

```ruby
def self.stunned?   # indicator-backed
  XMLData.indicator['IconSTUNNED'] == 'y'
end

def self.calmed?    # text-derived, AND confirmed
  Infomon.get_bool("status.calmed") && (Effects::Debuffs.active?('Calm') || ...)
end
```

**That `&&` is the design.** A text-derived condition has the same offset problem as a prompt
flag — the text says it started and never says it stopped — so Lich requires the effect still be
listed before believing its own parse. `bound?`, `calmed?`, `cutthroat?`, `silenced?`,
`sleeping?` and `thorned?` are all this shape.

This is also the author's instant-action rule (`plan/16` §2, "verify from buffs or cooldowns")
arriving from the other direction, which is decent evidence it is the right rule.

**One collection, not two** (author, `plan/16` §3.2). Provenance is a field on an entry, not a
reason to split storage: a behavior asking "am I buffed?" must not have to know which mechanism
tracks which ability.

---

## 5. Proposed `GameState` delta

```rust
pub struct GameState {
    // ... everything that is there today, unchanged ...

    /// Indicator id -> active. Ported from Vellum's `StatusInfo`.
    pub status: StatusInfo,

    /// The last `<prompt time=>`, and when it arrived locally.
    /// Together these give `game_time_now()`.
    game_time: Option<u32>,
    game_time_received: Option<std::time::Instant>,

    /// Active effects, from BOTH the dialog refills and parsed text.
    pub effects: Effects,
}
```

Plus `game_time_now()`, `in_roundtime()`, and typed accessors over `status`.

**`roundtime_ends` needs no change** — it is already `Option<u32>` of the absolute end. What
changes is that `in_roundtime()` can finally answer honestly, because there is a server clock to
compare it against.

---

## 6. Cost, and what it does not include

| In | Out |
|---|---|
| `StatusInfo` + the 13 ids | condition *evaluation* (Vellum's `conditions.rs` is 1,010 lines of frontend concern) |
| `game_time` / `game_time_now` / `in_roundtime` | lag display |
| `Effects`, one collection, both sources | the text patterns that populate the text-derived half |
| folding all three in `state.rs` | `send_now` and the instant-action table (`plan/16` §1) |

The last Out matters: this is the **prerequisite** for `plan/16`, not `plan/16` itself. It ends
with a `GameState` that can answer "am I in roundtime", "am I stunned" and "is this buff up" —
and nothing yet that acts on the answers.

---

## 7. The author's call

Adopting this widens `plan/12` §7.1's In column from "room, hands, roundtime, vitals" to include
status indicators, server time and effects. §7.1 currently lists "stats, skills, PSMs, spells,
inventory" as Out; **stats, skills, PSMs and inventory stay Out.** Spells move In only in the
sense of "the active-effect list", not spell data.

Amend `plan/12` §2/§7.1 and this document in the same commit, as the previous three amendments
did.
