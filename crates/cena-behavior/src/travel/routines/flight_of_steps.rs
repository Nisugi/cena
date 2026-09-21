//! `Routine::FlightOfSteps`: a round tower room whose four flights of steps
//! are listed in an order that changes.
//!
//! Upstream (`recognise/keys.rs`, `flight_of_steps`): `put 'look'`, read
//! lines until one lists four flights -- "a flight of ascending steps curving
//! along the northern wall, a flight of ..., ... and a flight of ...". The
//! first flight on the wanted `wall` is `climb steps`, the second `climb
//! second steps`, and so on; if none is, `echo "error: ..."` and `exit`.
//!
//! **Two deviations.** Upstream reads with `get` for ever; this looks
//! [`MAX_LOOKS`] times and then gives the exit up. And the sentence is read
//! from the answer to `look` *and* from the model's room description: in this
//! client a room's description arrives as a component, not as a line, so the
//! answer alone may never hold it. No flight on the wall is `Next::Failed`
//! rather than a stop: nothing the player could do would settle it.

use super::{Next, Seen, Solver};

/// Looks before giving up. Upstream's loop has no bound.
const MAX_LOOKS: u32 = 3;
/// What opens each flight in the room's sentence.
const FLIGHT: &str = "a flight of ";
/// The ordinals of `climb <ordinal> steps`, first to fourth.
const CLIMBS: [&str; 4] = [
    "climb steps",
    "climb second steps",
    "climb third steps",
    "climb fourth steps",
];

pub(super) struct FlightOfSteps {
    wall: String,
    looks: u32,
    climbed: bool,
}

impl FlightOfSteps {
    pub fn new(wall: String) -> Self {
        FlightOfSteps {
            wall,
            looks: 0,
            climbed: false,
        }
    }
}

/// What the sentence listing the four flights says.
#[derive(Debug, PartialEq, Eq)]
enum Read {
    /// Send this.
    Climb(&'static str),
    /// No flight is on the wall.
    Nowhere,
}

/// What a line says to send for this wall; `None` when it is not the
/// sentence listing four flights.
fn climb_for(text: &str, wall: &str) -> Option<Read> {
    let walls: Vec<&str> = text
        .split(FLIGHT)
        .skip(1)
        .filter_map(|flight| {
            let before = &flight[..flight.find(" wall")?];
            before.rsplit(' ').next()
        })
        .collect();
    (walls.len() == CLIMBS.len()).then(|| {
        let nth = walls.iter().position(|on| *on == wall);
        nth.and_then(|nth| CLIMBS.get(nth))
            .map_or(Read::Nowhere, |climb| Read::Climb(climb))
    })
}

impl Solver for FlightOfSteps {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        if self.climbed {
            return Next::Done;
        }
        if self.looks > 0 {
            let described = seen
                .state
                .room
                .description
                .as_ref()
                .map(cena_session::Runs::plain);
            let read = seen
                .answer
                .iter()
                .map(cena_session::ChunkLine::text)
                .chain(described)
                .find_map(|text| climb_for(&text, &self.wall));
            match read {
                Some(Read::Climb(climb)) => {
                    self.climbed = true;
                    return Next::Go(climb.to_owned());
                }
                Some(Read::Nowhere) => return Next::Failed,
                None => {}
            }
        }
        if self.looks >= MAX_LOOKS {
            return Next::Failed;
        }
        self.looks += 1;
        Next::Put("look".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    // A wall named before the first flight is not a flight's.
    const ROOM: &str = "Moss clings to the curved wall.  You also see a flight of ascending steps curving \
                        along the eastern wall, a flight of descending steps leading straight \
                        through the southern wall, a flight of ascending steps curving along \
                        the northern wall and a flight of descending steps curving along the \
                        northern wall.";

    #[test]
    fn it_looks_and_climbs_the_first_flight_on_the_wall() {
        let mut tower = FlightOfSteps::new("northern".into());
        assert_eq!(Scene::at(1, 2).ask(&mut tower), Next::Put("look".into()));
        assert_eq!(
            Scene::at(1, 2).answered(&["[Tower]", ROOM]).ask(&mut tower),
            Next::Go("climb third steps".into())
        );
        assert_eq!(Scene::at(2, 2).ask(&mut tower), Next::Done);
    }

    #[test]
    fn each_place_in_the_list_has_its_own_command() {
        for (wall, climb) in [
            ("eastern", "climb steps"),
            ("southern", "climb second steps"),
            ("northern", "climb third steps"),
        ] {
            assert_eq!(climb_for(ROOM, wall), Some(Read::Climb(climb)), "{wall}");
        }
        let last = ROOM.replace("the northern wall.", "the western wall.");
        assert_eq!(
            climb_for(&last, "western"),
            Some(Read::Climb("climb fourth steps"))
        );
    }

    #[test]
    fn no_flight_on_the_wall_gives_the_exit_up() {
        let mut tower = FlightOfSteps::new("western".into());
        assert_eq!(Scene::at(1, 2).ask(&mut tower), Next::Put("look".into()));
        assert_eq!(
            Scene::at(1, 2).answered(&[ROOM]).ask(&mut tower),
            Next::Failed
        );
    }

    #[test]
    fn a_line_with_fewer_than_four_flights_is_not_the_sentence() {
        assert_eq!(
            climb_for(
                "You also see a flight of ascending steps curving along the northern wall.",
                "northern"
            ),
            None
        );
    }

    #[test]
    fn it_looks_three_times_and_then_gives_up() {
        let mut tower = FlightOfSteps::new("northern".into());
        for _ in 0..MAX_LOOKS {
            assert_eq!(
                Scene::at(1, 2).answered(&["Fog."]).ask(&mut tower),
                Next::Put("look".into())
            );
        }
        assert_eq!(Scene::at(1, 2).ask(&mut tower), Next::Failed);
    }

    #[test]
    fn an_answer_is_not_read_before_anything_was_asked() {
        let mut tower = FlightOfSteps::new("northern".into());
        assert_eq!(
            Scene::at(1, 2).answered(&[ROOM]).ask(&mut tower),
            Next::Put("look".into())
        );
    }
}
