//! The nervous system's rank, and whether it is a wound or a scar.
//!
//! The injury window reports the nerves as `<image id='nsys' name='Nsys2'>`:
//! a rank, **not** `Injury2` or `Scar2`, so the image alone does not say which
//! it is (`reference/lich-5/lib/common/xmlparser.rb:816-872`; `VellumFE`'s
//! `reference/VellumFE/src/core/messages.rs:35-44`, whose doll "never showed
//! convulsions" until it read the prefix). Lich sends `health` each time a
//! rank above 0 arrives and reads one of its six nerve lines
//! (`xmlparser.rb:843-860`).
//!
//! # Hydra works it out instead (the author, 2026-09-29, `plan/55` §4a)
//!
//! > *"hydra has something that lich doesn't. It knows EVERYTHING."*
//!
//! Two facts decide it. **Damage only makes a wound**: lightning, an
//! overcast, a blow. **Only a herb makes a scar**: eating one heals a wound a
//! rank and leaves a scar as deep as the wound was, and the scar heals a rank
//! at a time after. The window shows the wound over the scar, so healing a
//! rank-2 wound reads `Nsys2`, `Nsys1`, `Nsys2`, `Nsys1`, `Nsys0`:
//!
//! | Rank | What happened | Wound | Scar |
//! |---|---|---|---|
//! | 2 | damage: a rise is a wound | 2 | 0 |
//! | 1 | a herb: the wound down a rank, a scar of 2 under it | 1 | 2 |
//! | 2 | a herb: the wound gone, the scar showing | 0 | 2 |
//! | 1 | a herb: the scar down a rank | 0 | 1 |
//! | 0 | a herb: whole | 0 | 0 |
//!
//! So a rank is read at the prompt that ends its chunk, when the chunk's
//! lines say whether a herb was eaten. Only when that does not explain it --
//! the first rank after logging in, which may be an old scar; a fall with no
//! herb, which an empath or a spell may make; a herb step that is not the
//! next one -- is Hydra **confused**, the rank shown as a wound (the author's
//! order, wounds over scars) and `health` asked once
//! ([`Character::take_nerve_question`]). A player's own `health` settles it
//! the same way.

use super::{Character, Injury};

/// The nerves as the injury window last ranked them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Nerves {
    /// The last `Nsys<n>` rank read; 0 when whole or never shown.
    pub rank: u8,
    /// Whether the wound and scar under that rank are known, worked out or
    /// said by `health`, rather than guessed.
    pub settled: bool,
    /// A rank has been read since the character logged in: the next one is a
    /// step from it, not a first sighting.
    known: bool,
    /// Ranks the window gave in the chunk under way, read at its prompt.
    seen: Vec<u8>,
    /// Confused since last asked: `health` should be sent, once.
    asking: bool,
}

/// The six lines `health` gives the nerves, each with its kind and rank
/// (`xmlparser.rb:843-860`, verbatim fragments).
const LINES: [(&str, Track, u8); 6] = [
    ("a case of uncontrollable convulsions", Track::Wound, 3),
    ("a case of sporadic convulsions", Track::Wound, 2),
    ("a strange case of muscle twitching", Track::Wound, 1),
    ("a very difficult time with muscle control", Track::Scar, 3),
    ("constant muscle spasms", Track::Scar, 2),
    ("developed slurred speech", Track::Scar, 1),
];

/// Which of the two a nerve line names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Track {
    Wound,
    Scar,
}

/// Where the `health` report's table starts: its description lines are the
/// ones above.
const TABLE: &str = "Maximum Health Points:";

/// How eating or drinking a herb begins (`doses.rs` reads the same lines for
/// what is left of it).
const HERB: [&str; 2] = ["You take a bite of your ", "You take a drink from your "];

/// The nerve line among a `health` report's description, if the lines are
/// one and it has one.
fn nerve_line(lines: &[String]) -> Option<(Track, u8)> {
    let table = lines.iter().position(|line| line.contains(TABLE))?;
    lines[..table].iter().find_map(|line| {
        let line = line.trim_start();
        if !line.starts_with("You") {
            return None;
        }
        LINES
            .iter()
            .find(|(text, ..)| line.contains(text))
            .map(|(_, track, rank)| (*track, *rank))
    })
}

/// Whether `line` is one of `health`'s six nerve lines about the character:
/// what a session leaves out of the reply to a `health` it sent itself, as
/// Lich returns `nil` for them (`xmlparser.rb:843-860`).
#[must_use]
pub fn is_nerve_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("You") && LINES.iter().any(|(text, ..)| line.contains(text))
}

/// The wound and scar a herb leaves, from `wound` and `scar`, if the window's
/// `rank` is the step it shows next.
fn herb_step(wound: u8, scar: u8, rank: u8) -> Option<(u8, u8)> {
    if wound > 0 {
        // The wound down a rank; a scar as deep as it was.
        let scar = scar.max(wound);
        let wound = wound - 1;
        let shown = if wound > 0 { wound } else { scar };
        (shown == rank).then_some((wound, scar))
    } else {
        (scar > 0 && rank + 1 == scar).then_some((0, rank))
    }
}

impl Character {
    /// `Nsys<rank>` from the injury window: kept until the chunk's prompt.
    pub(super) fn apply_nerve_rank(&mut self, rank: u8) {
        self.nerves.seen.push(rank);
    }

    /// Read the chunk's nerve ranks, knowing whether it ate a herb; then a
    /// `health` report among `lines`, if it is one and says.
    pub(crate) fn consume_nerves(&mut self, lines: &[String]) {
        let herb = lines.iter().any(|line| {
            let line = line.trim_start();
            HERB.iter().any(|start| line.starts_with(start))
        });
        let (was, seen) = (self.nerves.rank, std::mem::take(&mut self.nerves.seen));
        let moved = seen.iter().any(|rank| *rank != was);
        for rank in seen {
            self.read_nerve_rank(rank, herb);
        }
        // **A herb that leaves the rank showing where the nerves' herb would
        // too** cannot be worked out: a rank-1 wound healed leaves a rank-1
        // scar, `Nsys1` either way, and the herb may have been another
        // part's. Kept as the wound, the heal ate wound herbs for a scar 20
        // times (the crate review of 2026-10-01, MO-C-7). Asked instead.
        let Injury { wound, scar } = self.nsys();
        if herb
            && !moved
            && self.nerves.known
            && self.nerves.settled
            && herb_step(wound, scar, was).is_some_and(|step| step != (wound, scar))
        {
            self.nerves.settled = false;
            self.nerves.asking = true;
        }
        if let Some((track, rank)) = nerve_line(lines) {
            let known = self.nsys();
            self.set_nsys(match track {
                Track::Wound => Injury {
                    wound: rank,
                    scar: known.scar,
                },
                Track::Scar => Injury {
                    wound: 0,
                    scar: rank,
                },
            });
            self.nerves.settled = true;
            self.nerves.asking = false;
        }
    }

    /// One rank, as [`nerves`](self)'s table reads it.
    fn read_nerve_rank(&mut self, rank: u8, herb: bool) {
        let (was, first) = (self.nerves.rank, !self.nerves.known);
        self.nerves.rank = rank;
        self.nerves.known = true;
        if rank == 0 {
            self.set_nsys(Injury::default());
            self.nerves.settled = true;
            return;
        }
        if rank == was && !first {
            return;
        }
        let Injury { wound, scar } = self.nsys();
        let worked_out = if first {
            None
        } else if herb {
            herb_step(wound, scar, rank)
        } else if rank > was {
            Some((rank, scar))
        } else {
            None
        };
        if let Some((wound, scar)) = worked_out {
            self.set_nsys(Injury { wound, scar });
            self.nerves.settled = true;
        } else {
            self.set_nsys(Injury { wound: rank, scar });
            self.nerves.settled = false;
            self.nerves.asking = true;
        }
    }

    fn nsys(&self) -> Injury {
        self.injuries.get("nsys").copied().unwrap_or_default()
    }

    fn set_nsys(&mut self, injury: Injury) {
        if injury.is_hurt() {
            self.injuries.insert("nsys".to_owned(), injury);
        } else {
            self.injuries.remove("nsys");
        }
    }

    /// Whether the nerves are confused since last asked, and `health` should
    /// be sent to settle them: `true` once per confusion.
    pub fn take_nerve_question(&mut self) -> bool {
        std::mem::take(&mut self.nerves.asking)
    }
}

#[cfg(test)]
mod tests {
    use super::{Track, herb_step, nerve_line};

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn each_line_names_its_kind_and_rank_only_above_the_table() {
        let report = "You have a case of sporadic convulsions.\n\n   Maximum Health Points:   158";
        assert_eq!(nerve_line(&lines(report)), Some((Track::Wound, 2)));
        let scar = "You have developed slurred speech.\n   Maximum Health Points:   158";
        assert_eq!(nerve_line(&lines(scar)), Some((Track::Scar, 1)));
        // Not a report: someone else's convulsions, in the room.
        assert_eq!(
            nerve_line(&lines("Ada has a case of sporadic convulsions.")),
            None
        );
        // Below the table is not a description.
        let after = "   Maximum Health Points:   158\nYou have constant muscle spasms.";
        assert_eq!(nerve_line(&lines(after)), None);
    }

    /// The author's sequence, step by step, and a step that is not next.
    #[test]
    fn a_herb_takes_the_next_step_or_none() {
        assert_eq!(herb_step(2, 0, 1), Some((1, 2)));
        assert_eq!(herb_step(1, 2, 2), Some((0, 2)));
        assert_eq!(herb_step(0, 2, 1), Some((0, 1)));
        assert_eq!(herb_step(3, 0, 1), None, "two ranks at once is not a step");
        assert_eq!(herb_step(0, 0, 1), None, "nothing to heal");
    }
}
