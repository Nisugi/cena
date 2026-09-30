//! The character's own stun, as a timer beside roundtime and cast time (the
//! author, 2026-09-30: *"we need a stuntime timer like the casttime and
//! roundtime timers.. they're not exactly given to us like the other two
//! though, we have to calculate it. Usually we're told a stun in rounds,
//! and a round is 5 seconds"*).
//!
//! MEASURED in the author's Lich logs (`E:\Gemstone\dev\lich-5\logs`,
//! 2026-09-30): `You are stunned for 2 rounds!`, and after a crit's own
//! lines the same without `for`, `You are stunned 2 rounds!`, indented when
//! a crit says it, and `1 rounds` among them. `VellumFE`'s pattern and
//! Profanity's both miss the second. A round is five seconds
//! ([`STUN_ROUND_SECONDS`], Lich's `creature.rb:333`), and a stun only
//! lengthens what is left, never shortens it, as Lich's estimate of a
//! creature's does (`creature.rb:468`).
//!
//! **It ends when the `IconSTUNNED` indicator goes dark** (`state.rs`), which
//! those logs show every time (98 times) where no line of text says so (none
//! of `You are no longer stunned`). A stun the game gives no rounds for is
//! the indicator alone, and so is a stun still on after its estimate ran
//! out: the rounds are the game's, the seconds are ours.

use super::GameState;
use super::creatures::instance::STUN_ROUND_SECONDS;

/// The rounds `text` says the character is stunned for.
fn rounds(text: &str) -> Option<u32> {
    let rest = text.trim_start().strip_prefix("You are stunned ")?;
    let rest = rest.strip_prefix("for ").unwrap_or(rest);
    let (count, rest) = rest.split_once(' ')?;
    rest.starts_with("round").then_some(())?;
    count.parse().ok()
}

impl GameState {
    /// An `<indicator>`: the status it sets, and what its going dark ends.
    pub(super) fn apply_indicator(&mut self, id: &str, active: bool) {
        self.status.set(id, active);
        // `GROUP_EMPTIED` (`group.rb:603-605`): the indicator going dark is
        // the game saying you are in no group. See `Group::emptied`.
        if id == "IconJOINED" && !active {
            self.group.emptied();
        }
        if id == "IconSTUNNED" && !active {
            self.stun_ends = None;
        }
    }

    /// A line of the chunk that closed at `at`: a stun's rounds, told.
    pub(super) fn read_stun(&mut self, text: &str, at: Option<u32>) {
        if let (Some(rounds), Some(at)) = (rounds(text), at) {
            let ends = at.saturating_add(rounds.saturating_mul(STUN_ROUND_SECONDS));
            self.stun_ends = Some(self.stun_ends.map_or(ends, |was| was.max(ends)));
        }
    }

    /// Seconds of stun left by the rounds the game gave, as
    /// [`Self::roundtime_remaining`] is; `0` with none running, or when the
    /// estimate has run out while the indicator stays lit. `None` until the
    /// clock is known.
    #[must_use]
    pub fn stun_remaining(&self) -> Option<u32> {
        self.stun_remaining_after(self.elapsed_since_prompt())
    }

    /// [`Self::stun_remaining`] against a supplied interval. See
    /// [`Self::game_time_after`] for why the seam exists.
    #[must_use]
    pub fn stun_remaining_after(&self, elapsed: u32) -> Option<u32> {
        let now = self.game_time_after(elapsed)?;
        Some(self.stun_ends.map_or(0, |ends| ends.saturating_sub(now)))
    }
}

#[cfg(test)]
mod tests {
    use super::rounds;

    /// Both of the game's ways of saying it, indented or not, `round` or
    /// `rounds`; a stun with no count, or any other line, none.
    #[test]
    fn the_rounds_are_read_either_way() {
        assert_eq!(rounds("You are stunned for 2 rounds!"), Some(2));
        assert_eq!(rounds("   You are stunned for 1 rounds!"), Some(1));
        assert_eq!(rounds("You are stunned 3 rounds!"), Some(3));
        assert_eq!(rounds("You are stunned 1 round!"), Some(1));
        assert_eq!(rounds("You are still stunned."), None);
        assert_eq!(rounds("You are stunned!"), None);
        assert_eq!(rounds("The kobold is stunned for 2 rounds!"), None);
    }
}
