//! Which characters a spell has locked out.
//!
//! Ports the tracking half of Lich PR #1597. The five spells that declare a
//! cooldown, and the two cast mechanics that fill them, are pinned in
//! `tests/spells.rs`; this is what the tracker does with them.

use cena_model::state::cooldowns::Cooldowns;
use cena_model::state::group::Member;

/// A group member, as the wire's links give them.
fn member(id: &str, noun: &str) -> Member {
    Member {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: noun.to_owned(),
    }
}

/// 215 Heroism: a group cooldown of 180 seconds.
const HEROISM: u16 = 215;
/// 140 Wall of Force: a target cooldown of 270 seconds.
const WALL: u16 = 140;

mod group_castings {
    use super::{Cooldowns, HEROISM, member};

    #[test]
    fn everyone_grouped_is_stamped() {
        // The caster is told nothing about who a group casting landed on, so
        // everyone grouped at the time is stamped optimistically.
        let mut held = Cooldowns::default();
        let group = [member("-1", "Ryeka"), member("-2", "Nisugi")];
        assert_eq!(held.record_group(HEROISM, &group, 1000), 2);
        assert_eq!(held.remaining(HEROISM, "Ryeka", 1000), 180);
        assert_eq!(held.remaining(HEROISM, "Nisugi", 1000), 180);
    }

    #[test]
    fn a_member_still_locked_out_is_skipped_not_restamped() {
        // **Lich's rule, and porting it wrong would be a real bug**
        // (`group.rb:201`): a member still on cooldown did not receive this
        // casting, so their own cooldown keeps running. Re-stamping would
        // extend it every time someone else got a buff -- a groupmate who
        // never becomes castable.
        let mut held = Cooldowns::default();
        let ryeka = [member("-1", "Ryeka")];
        held.record_group(HEROISM, &ryeka, 1000);
        assert_eq!(held.remaining(HEROISM, "Ryeka", 1100), 80, "guard: running");

        // A second casting 100 seconds later, while she is still locked out.
        assert_eq!(
            held.record_group(HEROISM, &ryeka, 1100),
            0,
            "nobody was stamped"
        );
        assert_eq!(
            held.remaining(HEROISM, "Ryeka", 1100),
            80,
            "her own cooldown, not 180 again"
        );
    }

    #[test]
    fn a_member_whose_cooldown_expired_is_stamped_again() {
        let mut held = Cooldowns::default();
        let ryeka = [member("-1", "Ryeka")];
        held.record_group(HEROISM, &ryeka, 1000);
        assert_eq!(held.record_group(HEROISM, &ryeka, 1180), 1, "expired");
        assert_eq!(held.remaining(HEROISM, "Ryeka", 1180), 180);
    }

    #[test]
    fn someone_who_joined_after_the_casting_reads_as_ready() {
        // Lich's comment (`group.rb:175`): "Someone who joins after a casting
        // has no stamp and reads as ready, which is correct."
        let mut held = Cooldowns::default();
        held.record_group(HEROISM, &[member("-1", "Ryeka")], 1000);
        assert!(held.ready(HEROISM, "Latecomer", 1000));
    }

    #[test]
    fn a_spell_with_no_group_cooldown_stamps_nobody() {
        // Wall of Force has a TARGET cooldown and no group one. Stamping the
        // group for it would invent a lockout the game does not apply.
        let mut held = Cooldowns::default();
        assert_eq!(
            held.record_group(super::WALL, &[member("-1", "Ryeka")], 1000),
            0
        );
        assert!(held.is_empty());
    }

    #[test]
    fn a_spell_with_no_cooldown_at_all_stamps_nobody() {
        let mut held = Cooldowns::default();
        assert_eq!(held.record_group(101, &[member("-1", "Ryeka")], 1000), 0);
        assert!(held.is_empty());
    }
}

mod target_castings {
    use super::{Cooldowns, HEROISM, WALL};

    #[test]
    fn the_character_it_landed_on_is_stamped() {
        let mut held = Cooldowns::default();
        assert!(held.record_target(WALL, "Ryeka", 1000));
        assert_eq!(held.remaining(WALL, "Ryeka", 1000), 270);
    }

    #[test]
    fn it_counts_whoever_cast_it_and_whether_or_not_they_are_grouped() {
        // `group.rb:212`'s comment: "seeing someone else put a target on
        // cooldown is as useful as doing it yourself, and the target need not
        // be in the group." So this takes a noun, not a member.
        let mut held = Cooldowns::default();
        held.record_target(WALL, "AStranger", 1000);
        assert!(!held.ready(WALL, "AStranger", 1000));
    }

    #[test]
    fn a_target_casting_does_restamp() {
        // Unlike the group case. A target casting NAMES who it landed on, so
        // there is no optimism to correct -- the game said it landed, and the
        // cooldown starts then.
        let mut held = Cooldowns::default();
        held.record_target(WALL, "Ryeka", 1000);
        held.record_target(WALL, "Ryeka", 1100);
        assert_eq!(
            held.remaining(WALL, "Ryeka", 1100),
            270,
            "the new casting's full duration"
        );
    }

    #[test]
    fn a_spell_with_no_target_cooldown_stamps_nobody() {
        let mut held = Cooldowns::default();
        assert!(!held.record_target(HEROISM, "Ryeka", 1000));
        assert!(held.is_empty());
    }
}

mod reading {
    use super::{Cooldowns, HEROISM, member};

    #[test]
    fn nothing_recorded_reads_as_ready() {
        // **Zero, not `None`.** Lich's answer (`group.rb:235`) and the right
        // one: a character nobody has seen buffed is castable, and an
        // `Option` would make every caller handle a case that means "yes".
        let held = Cooldowns::default();
        assert_eq!(held.remaining(HEROISM, "Anyone", 1000), 0);
        assert!(held.ready(HEROISM, "Anyone", 1000));
    }

    #[test]
    fn remaining_never_goes_negative() {
        let mut held = Cooldowns::default();
        held.record_group(HEROISM, &[member("-1", "Ryeka")], 1000);
        assert_eq!(held.remaining(HEROISM, "Ryeka", 9999), 0, "long expired");
        assert!(held.ready(HEROISM, "Ryeka", 9999));
    }

    #[test]
    fn two_spells_are_tracked_apart() {
        let mut held = Cooldowns::default();
        held.record_group(HEROISM, &[member("-1", "Ryeka")], 1000);
        assert!(
            held.ready(219, "Ryeka", 1000),
            "Spell Shield is a different lockout"
        );
    }
}

mod over_a_game_state {
    use super::{HEROISM, member};
    use cena_model::GameState;
    use cena_protocol::Parser;

    /// A state with a server clock and a group.
    fn grouped(at: u32, members: &[&str]) -> GameState {
        let mut parser = Parser::new();
        let mut state = GameState::default();
        for frame in parser.parse_line(&format!("<prompt time=\"{at}\">&gt;</prompt>")) {
            state.apply(&frame);
        }
        for (index, noun) in members.iter().enumerate() {
            let id = format!("-{index}");
            for frame in parser.parse_line(&format!(
                "<a exist=\"{id}\" noun=\"{noun}\">{noun}</a> joins your group."
            )) {
                state.apply(&frame);
            }
        }
        for frame in parser.parse_line(&format!("<prompt time=\"{at}\">&gt;</prompt>")) {
            state.apply(&frame);
        }
        state
    }

    #[test]
    fn the_group_a_casting_would_actually_affect() {
        let mut state = grouped(1000, &["Ryeka", "Nisugi"]);
        assert_eq!(state.spell_cooldown_ready(HEROISM).len(), 2, "guard: both");

        state
            .cooldowns
            .record_group(HEROISM, &[member("-0", "Ryeka")], 1000);
        let ready = state.spell_cooldown_ready(HEROISM);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].noun, "Nisugi", "Ryeka is locked out");
    }

    #[test]
    fn without_a_server_clock_nothing_reads_as_ready() {
        // A cooldown cannot be judged without a clock, and answering "ready"
        // would have a behavior cast into a lockout. §5.2 in its operational
        // form: the honest answer is to say nothing is known to be castable.
        let mut state = GameState::default();
        assert_eq!(state.game_time_now(), None, "guard: no clock");
        for frame in cena_protocol::Parser::new()
            .parse_line(r#"<a exist="-1" noun="Ryeka">Ryeka</a> joins your group."#)
        {
            state.apply(&frame);
        }
        assert!(state.spell_cooldown_ready(HEROISM).is_empty());
        assert_eq!(state.spell_cooldown_left(HEROISM, "Ryeka"), None);
    }

    #[test]
    fn the_cooldowns_are_cleared_on_a_reconnect() {
        // **The contrast with the stow list and the bank balance**, which are
        // kept. Those are facts about YOU that nothing changes while you are
        // gone. These are stamps on other people, taken against a clock this
        // session was keeping -- and both break at once.
        //
        // Keeping them would have a behavior skip a groupmate who is long
        // since castable: nothing looks wrong, a buff just never lands.
        let mut state = grouped(1000, &["Ryeka"]);
        state
            .cooldowns
            .record_group(HEROISM, &[member("-0", "Ryeka")], 1000);
        assert!(!state.cooldowns.is_empty(), "guard: known first");

        state.invalidate_for_reconnect();
        assert!(state.cooldowns.is_empty());
    }
}
