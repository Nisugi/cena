//! When two game states are equal.
//!
//! Split out of `state.rs` under Rule 4.1 -- "move code down, do not raise the
//! cap" -- when that file reached 549 of 550. It is a whole, self-contained
//! impl with one job, which makes it the right thing to move rather than an
//! arbitrary cut.
//!
//! # Why this is hand-written and not derived
//!
//! [`GameState`] holds one field that must NOT be compared:
//! `game_time_received` is a local [`Instant`](std::time::Instant), so two
//! replays of the same bytes hold different values and a derived `PartialEq`
//! would make a deterministic replay compare unequal to itself (criterion 7).
//!
//! The fields are then listed **explicitly rather than derived-minus-one**, so
//! that adding one to `GameState` is a compile error here and whoever adds it
//! has to decide which side it belongs on. That guard has fired for every
//! field added this milestone.

use super::GameState;

impl PartialEq for GameState {
    fn eq(&self, other: &Self) -> bool {
        // Every field EXCEPT `game_time_received`. Listed explicitly rather
        // than derived-minus-one so that adding a field is a compile error
        // here, and whoever adds it has to decide which side it belongs on.
        let Self {
            room,
            arrivals: _,
            prompt,
            left_hand,
            right_hand,
            roundtime_ends,
            vitals,
            objectives,
            status,
            effects,
            game_time,
            game_time_received: _,
            unknown_tags,
            unknown_tag_counts,
            idle_warning,
            streams,
            // **Included.** A `streamWindow` declaration is something the
            // server said, and a replay of the same bytes declares the same
            // thing -- so two states that disagree about what `speech` does
            // when closed are not the same state.
            stream_windows,
            // **Excluded from equality, for a different reason than
            // `game_time_received`.** That one is unreproducible; this is a
            // tally of the process's behaviour rather than a fact about the
            // game. Two states that know the same things are equal whether or
            // not one has been running longer.
            tally: _,
            // Excluded for the tally's reason: it holds a queue a consumer
            // drains and a handle the session provides, neither a fact
            // about the game. What it knows of the game -- a held cast, an
            // open assault -- is re-derived from the same chunks.
            combat: _,
            // Facts already classified and waiting for a recorder, not a
            // fact about the game.
            loot: _,
            creatures,
            pending,
            chunk,
            character,
            inventory,
            inventory_snapshot,
            learned_commands,
            group,
            containers,
            bank,
            cooldowns,
            messages,
            overwatch,
            known_spells,
            bounty,
            doses,
            targeting,
            maneuvers,
            cast_time_ends,
        } = self;
        creatures == &other.creatures
            && inventory == &other.inventory
            && character == &other.character
            && idle_warning == &other.idle_warning
            && streams == &other.streams
            && stream_windows == &other.stream_windows
            && pending == &other.pending
            && chunk == &other.chunk
            && room == &other.room
            && prompt == &other.prompt
            && left_hand == &other.left_hand
            && right_hand == &other.right_hand
            && roundtime_ends == &other.roundtime_ends
            && vitals == &other.vitals
            && objectives == &other.objectives
            && inventory_snapshot == &other.inventory_snapshot
            && group == &other.group
            && containers == &other.containers
            && bank == &other.bank
            && cooldowns == &other.cooldowns
            && messages == &other.messages
            && overwatch == &other.overwatch
            && known_spells == &other.known_spells
            && bounty == &other.bounty
            && doses == &other.doses
            && targeting == &other.targeting
            && maneuvers == &other.maneuvers
            && cast_time_ends == &other.cast_time_ends
            // `arrivals` is NOT compared: it counts how many rooms this
            // session has entered, which is bookkeeping about the session
            // rather than a fact about the world. Two states that have been
            // told the same things are equal even if one reached its room by
            // a longer walk -- and criterion 7's replay determinism is about
            // the facts, not the route.
            && learned_commands == &other.learned_commands
            && status == &other.status
            && effects == &other.effects
            && game_time == &other.game_time
            && unknown_tags == &other.unknown_tags
            && unknown_tag_counts == &other.unknown_tag_counts
    }
}
