//! What the game says of the world rather than the character: when the next
//! pulse comes, and the world events under way. Both parsed as frames and,
//! until `plan/49` Stage B step 6, dropped here for want of a place.
//!
//! Ported from `VellumFE`'s reading of the same frames
//! (`reference/VellumFE/src/core/messages/element.rs:2398`, `:2490`): a
//! `<pulse min= max=>` bounds when the *next* pulse arrives, anchored on the
//! server clock at arrival; a `<worldEvent expires=>` lapses that many
//! minutes after it arrived, and lapsed events are pruned when a new one
//! comes. Here they are kept whole and filtered when read, so nothing is
//! lost between two events.

/// The last pulse the game announced, and so when the next is due.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pulse {
    /// The server second it arrived; `None` before the game's clock is
    /// known.
    pub at: Option<u32>,
    /// The fewest seconds until the next pulse.
    pub min: u32,
    /// The most seconds until the next pulse.
    pub max: u32,
    /// Whether the next pulse is a mana pulse.
    pub mana: bool,
}

impl Pulse {
    /// The seconds until the next pulse can come and must have come, at
    /// server second `now`: `(0, 0)` once it is overdue.
    #[must_use]
    pub fn due(&self, now: u32) -> Option<(u32, u32)> {
        let at = self.at?;
        let passed = now.saturating_sub(at);
        Some((
            self.min.saturating_sub(passed),
            self.max.saturating_sub(passed),
        ))
    }
}

/// One world event the game announced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldEvent {
    /// Where, as the game names it; `None` when it did not say.
    pub realm: Option<String>,
    /// What, in the game's words.
    pub text: String,
    /// The server second it lapses; `None` when the game gave no expiry, or
    /// its clock was not yet known.
    pub expires_at: Option<u32>,
}

/// The world's pulse and events, as the game has told them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct World {
    /// The last pulse; `None` before the first.
    pub pulse: Option<Pulse>,
    /// Every event announced, oldest first.
    events: Vec<WorldEvent>,
}

/// The most events kept, lapsed or not: a bound on what a long run holds.
const MAX_EVENTS: usize = 32;

impl World {
    /// A `<pulse>` arrived at server second `now`.
    pub(crate) fn pulsed(&mut self, now: Option<u32>, min: u32, max: u32, mana: bool) {
        self.pulse = Some(Pulse {
            at: now,
            min,
            max,
            mana,
        });
    }

    /// A `<worldEvent>` arrived at server second `now`: lapsed ones go.
    pub(crate) fn announced(
        &mut self,
        now: Option<u32>,
        realm: &str,
        expires_min: Option<u32>,
        text: &str,
    ) {
        if let Some(now) = now {
            self.events
                .retain(|event| event.expires_at.is_none_or(|at| at > now));
        }
        self.events.push(WorldEvent {
            realm: (!realm.is_empty()).then(|| realm.to_owned()),
            text: text.to_owned(),
            expires_at: now
                .zip(expires_min)
                .map(|(now, min)| now.saturating_add(min.saturating_mul(60))),
        });
        if self.events.len() > MAX_EVENTS {
            self.events.remove(0);
        }
    }

    /// The events under way at server second `now`, oldest first: every
    /// one, when the clock is not known.
    pub fn events(&self, now: Option<u32>) -> impl Iterator<Item = &WorldEvent> {
        self.events
            .iter()
            .filter(move |event| now.zip(event.expires_at).is_none_or(|(now, at)| at > now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pulse bounds the next: due between its least and most seconds,
    /// counting down, and overdue at none.
    #[test]
    fn a_pulse_says_when_the_next_is_due() {
        let mut world = World::default();
        assert_eq!(world.pulse, None, "before the first");
        world.pulsed(Some(1_000), 46, 75, true);
        let pulse = world.pulse.clone().expect("pulsed");
        assert!(pulse.mana);
        assert_eq!(pulse.due(1_000), Some((46, 75)));
        assert_eq!(pulse.due(1_050), Some((0, 25)));
        assert_eq!(pulse.due(2_000), Some((0, 0)));
        world.pulsed(None, 46, 75, false);
        assert_eq!(
            world.pulse.and_then(|pulse| pulse.due(1_000)),
            None,
            "no clock"
        );
    }

    /// An event lapses its minutes after it came; a lapsed one is not
    /// under way, and goes when the next arrives.
    #[test]
    fn a_world_event_lapses() {
        let mut world = World::default();
        world.announced(
            Some(1_000),
            "Wehnimer's Landing",
            Some(10),
            "An invasion begins!",
        );
        world.announced(Some(1_000), "", None, "The moons align.");
        let under_way = |world: &World, now| -> Vec<String> {
            world.events(now).map(|event| event.text.clone()).collect()
        };
        assert_eq!(
            under_way(&world, Some(1_599)),
            ["An invasion begins!", "The moons align."]
        );
        assert_eq!(under_way(&world, Some(1_600)), ["The moons align."]);
        assert_eq!(under_way(&world, None).len(), 2, "no clock: every one");
        let realms: Vec<Option<&str>> = world
            .events(None)
            .map(|event| event.realm.as_deref())
            .collect();
        assert_eq!(realms, [Some("Wehnimer's Landing"), None], "none said");
        // Minutes no clock can hold: never lapsing, not a wrapped second
        // in the past, nor a panic in a debug build.
        world.announced(Some(1_000), "", Some(u32::MAX), "Forever.");
        assert!(under_way(&world, Some(u32::MAX - 1)).contains(&"Forever.".to_owned()));
        world.events.pop();
        // The second it lapses, it goes.
        world.announced(Some(1_600), "", None, "Another.");
        assert_eq!(world.events.len(), 2, "the lapsed one went");
        for n in 0..MAX_EVENTS {
            world.announced(None, "", None, &format!("event {n}"));
        }
        assert_eq!(world.events.len(), MAX_EVENTS);
    }

    /// The game state keeps the frames it once dropped; a reconnect forgets
    /// the pulse, timed from its connection, and keeps the world's events;
    /// two states told different worlds are not the same state.
    #[test]
    fn the_game_state_keeps_the_world() {
        use crate::state::GameState;
        use cena_protocol::Frame;
        let mut state = GameState::default();
        state.apply(&Frame::Pulse {
            mana: true,
            min: 46,
            max: 75,
        });
        state.apply(&Frame::WorldEvent {
            realm: "Icemule Trace".to_owned(),
            expires_min: None,
            text: "Snow falls.".to_owned(),
        });
        assert_eq!(
            state.world.pulse.as_ref().map(|pulse| pulse.mana),
            Some(true)
        );
        assert_eq!(state.world.events(None).count(), 1);
        assert_ne!(
            state,
            GameState::default(),
            "the world is part of the state"
        );
        state.invalidate_for_reconnect();
        assert_eq!(state.world.pulse, None, "a new connection pulses anew");
        assert_eq!(state.world.events(None).count(), 1, "the world went on");
    }
}
