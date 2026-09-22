//! What the game says when a move does not simply work: a **stateless
//! classifier** over one line of text (`plan/12` §3a), for the Travel
//! behaviour's move recovery (`plan/24` stage 2).
//!
//! # Where the lines come from
//!
//! Every pattern here is **Lich's**, from `reference/lich-5/lib/common/move.rb`
//! (the `move` method's `elsif line =~` ladder), in the same order, so a line
//! two patterns could claim is claimed by the one Lich would have picked.
//! They are the game's own text, collected by that script's maintainers over
//! years (author, 2026-09-21); nothing here was invented or cut from a log.
//! The patterns are Ruby's and are used as written: the two dialects agree on
//! everything these use.
//!
//! Lich also matches the other game's "retreat first" lines in this ladder.
//! That game is deferred (`CLAUDE.md`), and they are left out.
//!
//! # What this does not do
//!
//! It names what a line *is*. What to do about it -- which remedy, how many
//! tries, whether the exit is banned -- is the walker's
//! (`cena-behavior::travel`), which has the state this file must not.

use std::sync::OnceLock;

use regex::Regex;

/// What one line says about the move that was just sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveFeedback {
    /// Hidden or invisible, and this way in wants the walker seen. Remedy:
    /// `unhide`.
    MustUnhide,
    /// One of several doors: this was the wrong one. Remedy: the next
    /// ordinal (`go door` -> `go second door`).
    WrongDoor,
    /// **The exit is wrong for the map**: there is no such way from here.
    /// Lich returns `false`, and a caller may drop the exit.
    NoSuchWay,
    /// Refused, but the exit is real: a guard, a ticket, a closed shop. Lich
    /// returns `nil` -- keep the exit, give up on it for now.
    Denied,
    /// A skill roll failed and may succeed next time: a climb lost, a swim
    /// pushed back, vertigo, fog. Remedy: wait out the roundtime, try again.
    /// `fell` is true where the failure leaves the walker on the ground.
    FailedRoll { fell: bool },
    /// The move worked, though the room did not change in the usual way:
    /// the long swims of Sailor's Grief.
    Swam,
    /// A climb that needs the hands free as well as another try.
    FailedRollHandsFull,
    /// Too injured to climb. Remedy: Sigil of Resolve, if known.
    TooInjured,
    /// `go` was sent and the game wants `climb`.
    NeedsClimb,
    /// `climb` was sent and the game wants `go`.
    NeedsGo,
    /// Dragging someone, and it did not work.
    CannotDrag,
    /// Remedy: empty the hands, and give them back after.
    HandsFull,
    /// Remedy: `open` it, once.
    Closed,
    /// `...wait N seconds.` -- roundtime, with the seconds.
    Wait(u32),
    /// Remedy: `stand`.
    MustStand,
    /// "still recovering from your recent..." Remedy: wait two seconds.
    StillRecovering,
    /// Too many commands in flight. Remedy: wait a second.
    TypeAhead,
    /// Remedy: wait for the stun to pass.
    Stunned,
    /// A floating disk would not follow. Remedy: just try again.
    DiskWobbled,
    /// Something of the walker's is on the ground. Remedy: `stow feet`.
    ItemAtFeet,
    /// A boltstone trap fired: stunned, knocked down.
    Trapped,
    /// Held fast. Remedy: wait to regain control.
    Held,
    /// No light. **Lich treats this as success**, and is the reference for
    /// what the wire means (`plan/21` §2c).
    PitchDark,
}

/// Lich's patterns, in Lich's order. `move.rb`, the `loop` in `move`.
const LADDER: &[(&str, MoveFeedback)] = &[
    (
        r"^You can't enter .+ and remain hidden or invisible\.|if he can't see you!$|^You can't enter .+ when you can't be seen\.$|^You can't do that without being seen\.$|^How do you intend to get .*? attention\?  After all, no one can see you right now\.$",
        MoveFeedback::MustUnhide,
    ),
    (
        r"^You (?:take a few steps toward|trudge up to|limp towards|march up to|sashay gracefully up to|skip happily towards|sneak up to|stumble toward) a rusty doorknob",
        MoveFeedback::WrongDoor,
    ),
    (
        // Ruby's `(?! as far away as you can get)` lookahead is handled in
        // `classify`, since this dialect has none.
        r"^You can't go there|^You can't (?:go|swim) in that direction\.|^Where are you trying to go\?|^What were you referring to\?|^I could not find what you were referring to\.|^How do you plan to do that here\?|^You take a few steps towards|^You cannot do that\.|^You settle yourself on|^You shouldn't annoy|^You can't go to|^That's probably not a very good idea|^Maybe you should look|^You are already|^You walk over to|^You step over to|The [\w\s]+ is too far away|You may not pass\.|become impassable\.|prevents you from entering\.|Please leave promptly\.|is too far above you to attempt that\.$|^Uh, yeah\.  Right\.$|^Definitely NOT a good idea\.$|^Your attempt fails|^There doesn't seem to be any way to do that at the moment\.$",
        MoveFeedback::NoSuchWay,
    ),
    (
        r#"^[A-z\s-] is unable to follow you\.$|^An unseen force prevents you\.$|^Sorry, you aren't allowed to enter here\.|^That looks like someplace only performers should go\.|^As you climb, your grip gives way and you fall down|^The clerk stops you from entering the partition and says, "I'll need to see your ticket!"$|^The guard stops you, saying, "Only members of registered groups may enter the Meeting Hall\.  If you'd like to visit, ask a group officer for a guest pass\."$|^An? .*? reaches over and grasps [A-Z][a-z]+ by the neck preventing (?:him|her) from being dragged anywhere\.$|^You'll have to wait, [A-Z][a-z]+ .* locker|^As you move toward the gate, you carelessly bump into the guard|^You attempt to enter the back of the shop, but a clerk stops you.  "Your reputation precedes you!|you notice that thick beams are placed across the entry with a small sign that reads, "Abandoned\."$|appears to be closed, perhaps you should try again later\?$"#,
        MoveFeedback::Denied,
    ),
    (
        r#"^You grab [A-Z][a-z]+ and try to drag h(?:im|er), but s?he (?:is too heavy|doesn't budge)\.$|^Tentatively, you attempt to swim through the nook\.  After only a few feet, you begin to sink!  Your lungs burn from lack of air, and you begin to panic!  You frantically paddle back to safety!$|^Guards(?:wo)?man [A-Z][a-z]+ stops you and says, "(?:Stop\.|Halt!)  You need to make sure you check in|^You step into the root, but can see no way to climb the slippery tendrils inside\.  After a moment, you step back out\.$|^As you start .*? back to safe ground\.$|^You stumble a bit as you try to enter the pool but feel that your persistence will pay off\.$|^A shimmering field of magical crimson and gold energy flows through the area\.$|^You attempt to navigate your way through the fog, but (?:quickly become entangled|get turned around)|^Trying to judge the climb, you peer over the edge\.\s*A wave of dizziness hits you, and you back away from the .*\.$|^You approach the .*, but the steepness is intimidating\.$|^You make your way (?:up|down) the .*\.\s*Partway (?:up|down), you make the mistake of looking down\. Struck by vertigo, you cling to the .* for a few moments, then slowly climb back (?:up|down)\.$|^You pick your way up the .*, but reach a point where your footing is questionable.\s*Reluctantly, you climb back down.$"#,
        MoveFeedback::FailedRoll { fell: false },
    ),
    (
        r"^Climbing.*(?:plunge|fall)|^Tentatively, you attempt to climb.*(?:fall|slip)|^You start up the .* but slip after a few feet and fall to the ground|^You start.*but quickly realize|^You.*drop back to the ground|^You leap .* fall unceremoniously to the ground in a heap\.$|^You search for a way to make the climb .*? but without success\.$|^You start to climb .* you fall to the ground|^You attempt to climb .* wrong approach|^You run towards .*? slowly retreat back, reassessing the situation\.|^You attempt to climb down the .*, but you can't seem to find purchase\.|^You start down the .*, but you find it hard going.\s*Rather than risking a fall, you make your way back up\.",
        MoveFeedback::FailedRoll { fell: true },
    ),
    (
        r"^(?:You swim .*, (?:cutting through|navigating)|You swim .*, struggling against|Your lungs burn and your muscles ache)",
        MoveFeedback::Swam,
    ),
    (
        r"^You begin to climb up the silvery thread.* you tumble to the ground",
        MoveFeedback::FailedRollHandsFull,
    ),
    (
        r"^You are too injured to be doing any climbing!$",
        MoveFeedback::TooInjured,
    ),
    (
        r"^You(?:'re going to| will) have to climb that\.",
        MoveFeedback::NeedsClimb,
    ),
    (r"^You can't climb that\.", MoveFeedback::NeedsGo),
    (r"^You can't drag", MoveFeedback::CannotDrag),
    (
        r"^Maybe if your hands were empty|^You figure freeing up both hands might help\.|^You can't .+ with your hands full\.$|^You'll need empty hands to climb that\.$|^It's a bit too difficult to swim holding|^You will need both hands free for such a difficult task\.",
        MoveFeedback::HandsFull,
    ),
    (
        r"(?:appears|seems) to be closed\.$|^You cannot quite manage to squeeze between the stone doors\.$",
        MoveFeedback::Closed,
    ),
    // `Wait` is filled in by `classify`, which reads the seconds.
    (
        r"^(?:\.\.\.w|W)ait ([0-9]+) sec(?:onds)?\.$",
        MoveFeedback::Wait(0),
    ),
    (
        r"will have to stand up first|must be standing first|^You'll have to get up first|^But you're already sitting!|^Shouldn't you be standing first|^That would be quite a trick from that position\.  Try standing up\.|^Perhaps you should stand up|^Standing up might help|^You should really stand up first|You can't do that while sitting|You must be standing to do that|You can't do that while lying down|^You must be standing|^You can't do that from that position",
        MoveFeedback::MustStand,
    ),
    (
        r"^You're still recovering from your recent",
        MoveFeedback::StillRecovering,
    ),
    (
        r"^The ground approaches you at an alarming rate|You go flying down several feet, landing with a",
        MoveFeedback::FailedRoll { fell: true },
    ),
    (r"^Sorry, you may only type ahead", MoveFeedback::TypeAhead),
    (r"^You are still stunned\.$", MoveFeedback::Stunned),
    (
        r"you slip (?:on a patch of ice )?and flail uselessly as you land on your rear(?:\.|!)$|You wobble and stumble only for a moment before landing flat on your face!$|^You slip in the mud and fall flat on your back!$",
        MoveFeedback::FailedRoll { fell: true },
    ),
    (
        r"^You flick your hand (?:up|down)wards and focus your aura on your disk, but your disk only wobbles briefly\.$",
        MoveFeedback::DiskWobbled,
    ),
    (
        r"^You dive into the fast-moving river, but the current catches you and whips you back to shore, wet and battered\.$|^Running through the swampy terrain, you notice a wet patch in the bog|^You flounder around in the water.$|^You blunder around in the water, barely able|^You struggle against the swift current to swim|^You slap at the water in a sad failure to swim|^You work against the swift current to swim",
        MoveFeedback::FailedRoll { fell: false },
    ),
    (
        r"^(?:You notice .* at your feet, and do not wish to leave it behind|As you prepare to move away, you remember)",
        MoveFeedback::ItemAtFeet,
    ),
    (
        r"The electricity courses through you in a raging torrent, its power singing in your veins!  Spent, the boltstone apparatus shatters into glinting fragments\.|The lightning strikes you in an agonizing eruption of liquid radiance!",
        MoveFeedback::Trapped,
    ),
    (
        r"^You don't seem to be able to move to do that\.$",
        MoveFeedback::Held,
    ),
    (
        r"^It's pitch dark and you can't see a thing!",
        MoveFeedback::PitchDark,
    ),
];

fn ladder() -> &'static Vec<(Regex, MoveFeedback)> {
    static LADDER_BUILT: OnceLock<Vec<(Regex, MoveFeedback)>> = OnceLock::new();
    LADDER_BUILT.get_or_init(|| {
        LADDER
            .iter()
            .filter_map(|(pattern, feedback)| Some((Regex::new(pattern).ok()?, *feedback)))
            .collect()
    })
}

/// How many of `LADDER`'s patterns compiled. A test holds this to the
/// ladder's length: a pattern that failed to compile would otherwise be a
/// class of line the walker silently never sees.
#[must_use]
pub fn patterns_compiled() -> (usize, usize) {
    (ladder().len(), LADDER.len())
}

/// What this line says about a move, if anything. The first pattern of
/// Lich's ladder that matches wins, as it does there.
#[must_use]
pub fn classify(line: &str) -> Option<MoveFeedback> {
    for (pattern, feedback) in ladder() {
        let Some(found) = pattern.captures(line) else {
            continue;
        };
        return Some(match feedback {
            MoveFeedback::Wait(_) => MoveFeedback::Wait(found.get(1)?.as_str().parse().ok()?),
            // Ruby: `^You are already(?! as far away as you can get)`.
            MoveFeedback::NoSuchWay
                if line.starts_with("You are already as far away as you can get") =>
            {
                continue;
            }
            other => *other,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_compiles() {
        let (built, written) = patterns_compiled();
        assert_eq!(built, written);
    }

    #[test]
    fn lines_are_named_as_lich_names_them() {
        for (line, expected) in [
            ("You can't go there.", MoveFeedback::NoSuchWay),
            (
                "I could not find what you were referring to.",
                MoveFeedback::NoSuchWay,
            ),
            ("You may not pass.", MoveFeedback::NoSuchWay),
            ("An unseen force prevents you.", MoveFeedback::Denied),
            ("The gate appears to be closed.", MoveFeedback::Closed),
            ("...wait 3 seconds.", MoveFeedback::Wait(3)),
            ("Wait 1 sec.", MoveFeedback::Wait(1)),
            ("You must be standing first.", MoveFeedback::MustStand),
            ("You are still stunned.", MoveFeedback::Stunned),
            (
                "You'll need empty hands to climb that.",
                MoveFeedback::HandsFull,
            ),
            (
                "You're going to have to climb that.",
                MoveFeedback::NeedsClimb,
            ),
            ("You can't climb that.", MoveFeedback::NeedsGo),
            (
                "Sorry, you may only type ahead 1 command.",
                MoveFeedback::TypeAhead,
            ),
            (
                "It's pitch dark and you can't see a thing!",
                MoveFeedback::PitchDark,
            ),
            (
                "You can't enter the bank and remain hidden or invisible.",
                MoveFeedback::MustUnhide,
            ),
            (
                "You slip in the mud and fall flat on your back!",
                MoveFeedback::FailedRoll { fell: true },
            ),
            (
                "You struggle against the swift current to swim north.",
                MoveFeedback::FailedRoll { fell: false },
            ),
        ] {
            assert_eq!(classify(line), Some(expected), "{line}");
        }
    }

    /// Order is Lich's, and it decides. "appears to be closed, perhaps you
    /// should try again later?" is a shop that is shut (keep the exit), not a
    /// door to open -- and only the ladder's order says so.
    #[test]
    fn the_first_pattern_that_matches_wins() {
        assert_eq!(
            classify("The shop appears to be closed, perhaps you should try again later?"),
            Some(MoveFeedback::Denied)
        );
    }

    #[test]
    fn rubys_lookahead_is_kept() {
        assert_eq!(
            classify("You are already standing."),
            Some(MoveFeedback::NoSuchWay)
        );
        assert_eq!(
            classify("You are already as far away as you can get!"),
            None
        );
    }

    #[test]
    fn ordinary_text_says_nothing_about_a_move() {
        for line in ["[Town Square, Central]", "Obvious paths: north, south", ""] {
            assert_eq!(classify(line), None, "{line}");
        }
    }
}
