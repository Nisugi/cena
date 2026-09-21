//! `Puzzle::AltarLevers`: the altar of room 18893.
//!
//! Upstream (`upstream_scripts/altar_levers.rb`): `look altar` and take the
//! line that begins "There is a strange grid on the altar."; in it `Yellow
//! 3`, `Blue 1` and so on give each coloured lever its row. Then, lever by
//! lever in upstream's order -- yellow, blue, brown, red, green -- `pull
//! <colour> lever` until the "You pull" line names that row: `first` to
//! `fifth`. Then `go step`, `go passage`, `go opening`.
//!
//! Deviations: a grid that cannot be read is an error upstream; here it is
//! [`Next::Failed`]. Upstream pulls for ever at a lever that never says its
//! row; here every pull counts toward [`MAX_TURNS`]. A move that fails ends
//! the routine, and the trip plans again, where upstream walks on.

use cena_session::ChunkLine;

use super::{MAX_TURNS, Next, Seen, Solver};

const GRID: &str = "There is a strange grid on the altar.";
/// The levers as the grid labels them and as they are pulled, in the order
/// upstream pulls them.
const LEVERS: &[(&str, &str)] = &[
    ("Yellow", "yellow"),
    ("Blue", "blue"),
    ("Brown", "brown"),
    ("Red", "red"),
    ("Green", "green"),
];
/// The grid's rows as a pull names them. Upstream's `number`.
const ROWS: &[(char, &str)] = &[
    ('1', "first"),
    ('2', "second"),
    ('3', "third"),
    ('4', "fourth"),
    ('5', "fifth"),
];
const WAY_OUT: &[&str] = &["go step", "go passage", "go opening"];

/// The row the grid gives this lever: the digit after `Yellow `.
fn row_of(grid: &str, label: &str) -> Option<&'static str> {
    let label = format!("{label} ");
    grid.match_indices(&label).find_map(|(at, _)| {
        let digit = grid[at + label.len()..].chars().next()?;
        ROWS.iter()
            .find(|(number, _)| *number == digit)
            .map(|(_, row)| *row)
    })
}

#[derive(Default)]
pub(super) struct AltarLevers {
    /// Where each of [`LEVERS`] must stand, once the grid is read.
    rows: Vec<&'static str>,
    turns: u32,
    at: At,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    Looked,
    /// Pulling this one of [`LEVERS`].
    Pulling(usize),
    /// How many of [`WAY_OUT`] have been sent.
    Leaving(usize),
}

impl AltarLevers {
    fn read(seen: &Seen<'_>) -> Option<Vec<&'static str>> {
        let grid = seen
            .answer
            .iter()
            .map(ChunkLine::text)
            .find(|text| text.starts_with(GRID))?;
        LEVERS
            .iter()
            .map(|(label, _)| row_of(&grid, label))
            .collect()
    }

    fn pull(&mut self, seen: &Seen<'_>, lever: usize, pulled: bool) -> Next {
        let (Some((_, colour)), Some(row)) = (LEVERS.get(lever), self.rows.get(lever)) else {
            self.at = At::Leaving(0);
            return self.next(seen);
        };
        let set = pulled
            && seen
                .answer
                .iter()
                .map(ChunkLine::text)
                .any(|text| text.starts_with("You pull") && text.contains(row));
        if set {
            return self.pull(seen, lever + 1, false);
        }
        if self.turns >= MAX_TURNS {
            return Next::Failed;
        }
        self.turns += 1;
        self.at = At::Pulling(lever);
        Next::Put(format!("pull {colour} lever"))
    }
}

impl Solver for AltarLevers {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => {
                self.at = At::Looked;
                Next::Put("look altar".to_owned())
            }
            At::Looked => {
                let Some(rows) = Self::read(seen) else {
                    return Next::Failed;
                };
                self.rows = rows;
                self.pull(seen, 0, false)
            }
            At::Pulling(lever) => self.pull(seen, lever, true),
            At::Leaving(_) if !seen.ok => Next::Done,
            At::Leaving(sent) => match WAY_OUT.get(sent) {
                Some(command) => {
                    self.at = At::Leaving(sent + 1);
                    Next::Go((*command).to_owned())
                }
                None => Next::Done,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    const ALTAR: &str = "There is a strange grid on the altar.  Five rows are labeled with \
        numbers and five columns are labeled with colors.  A red glow illuminates the \
        following squares: Yellow 3, Blue 1, Brown 5, Red 2, Green 4.";

    fn put(command: &str) -> Next {
        Next::Put(command.to_owned())
    }

    fn hears(altar: &mut AltarLevers, line: &str) -> Next {
        Scene::at(1, 2).answered(&[line]).ask(altar)
    }

    fn pulled(row: &str) -> String {
        format!("You pull the lever and it clicks into the {row} position.")
    }

    #[test]
    fn each_lever_is_pulled_to_its_row_and_then_the_way_out_is_walked() {
        let mut altar = AltarLevers::default();
        assert_eq!(Scene::at(1, 2).ask(&mut altar), put("look altar"));
        assert_eq!(hears(&mut altar, ALTAR), put("pull yellow lever"));
        assert_eq!(
            hears(&mut altar, &pulled("second")),
            put("pull yellow lever")
        );
        assert_eq!(hears(&mut altar, &pulled("third")), put("pull blue lever"));
        assert_eq!(hears(&mut altar, &pulled("first")), put("pull brown lever"));
        assert_eq!(hears(&mut altar, &pulled("fifth")), put("pull red lever"));
        assert_eq!(
            hears(&mut altar, &pulled("second")),
            put("pull green lever")
        );
        assert_eq!(
            hears(&mut altar, &pulled("fourth")),
            Next::Go("go step".into())
        );
        assert_eq!(
            Scene::at(3, 2).ask(&mut altar),
            Next::Go("go passage".into())
        );
        assert_eq!(
            Scene::at(4, 2).ask(&mut altar),
            Next::Go("go opening".into())
        );
        assert_eq!(Scene::at(2, 2).ask(&mut altar), Next::Done);
    }

    #[test]
    fn only_a_pull_names_the_row() {
        let mut altar = AltarLevers::default();
        Scene::at(1, 2).ask(&mut altar);
        hears(&mut altar, ALTAR);
        let other = "Someone says, \"the third one\"";
        assert_eq!(hears(&mut altar, other), put("pull yellow lever"));
    }

    #[test]
    fn every_row_of_every_lever_is_read_off_the_grid() {
        for (at, (label, _)) in LEVERS.iter().enumerate() {
            for (digit, row) in ROWS {
                let grid = format!("{GRID}  A red glow illuminates Mauve 9, {label} {digit}.");
                assert_eq!(row_of(&grid, label), Some(*row));
                // And it is what that lever is pulled to.
                let mut altar = AltarLevers {
                    rows: vec![row; LEVERS.len()],
                    turns: 0,
                    at: At::Pulling(at),
                };
                let next = LEVERS.get(at + 1).map_or_else(
                    || Next::Go("go step".into()),
                    |(_, colour)| put(&format!("pull {colour} lever")),
                );
                assert_eq!(hears(&mut altar, &pulled(row)), next);
            }
        }
    }

    #[test]
    fn a_grid_that_cannot_be_read_cannot_be_crossed() {
        let mut altar = AltarLevers::default();
        Scene::at(1, 2).ask(&mut altar);
        assert_eq!(hears(&mut altar, "I could not find that."), Next::Failed);

        let mut altar = AltarLevers::default();
        Scene::at(1, 2).ask(&mut altar);
        let short = format!("{GRID}  A red glow illuminates Yellow 3, Blue 1, Brown 5, Red 2.");
        assert_eq!(hears(&mut altar, &short), Next::Failed);
    }

    #[test]
    fn a_lever_that_never_says_its_row_is_given_up() {
        let mut altar = AltarLevers::default();
        Scene::at(1, 2).ask(&mut altar);
        for _ in 0..MAX_TURNS {
            assert_eq!(hears(&mut altar, ALTAR), put("pull yellow lever"));
        }
        assert_eq!(hears(&mut altar, ALTAR), Next::Failed);
    }

    #[test]
    fn a_move_that_fails_ends_it() {
        let mut altar = AltarLevers {
            rows: Vec::new(),
            turns: 0,
            at: At::Leaving(1),
        };
        let mut stuck = Scene::at(1, 2);
        stuck.failed = true;
        assert_eq!(stuck.ask(&mut altar), Next::Done);
    }
}
