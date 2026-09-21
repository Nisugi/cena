//! Upstream scripts this converter knows how to say in steps (`plan/21` §5
//! step 7). This file and [`crate::shape`] are the only places Ruby is read.
//!
//! # An arm is a template, not a parser
//!
//! Each arm is the upstream script **verbatim**, with holes where the edges
//! that share the shape differ. A script matches an arm exactly or it does not
//! match -- there is no Ruby grammar here and no partial understanding. When
//! upstream edits a script by one character the arm stops matching, the exit
//! goes back to `unported`, and the ratchet fails the build. That is the
//! intended failure: loud, and on the converter's side of the pipeline.
//!
//! Arms are added in the order that opens the most rooms
//! (`research/mapdb-inventory/chokepoints.py`), not the order of most edges.

use cena_map::{Action, Cond, Cost, Crossing, Pass, RoomId, Routine, Step};

/// The steps for an upstream crossing script, if an arm knows it. `from` is
/// the room the exit leaves and `to` the room it reaches, which some scripts
/// name and some only imply.
#[must_use]
pub fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    if script == ";e true" {
        return Some(Crossing::PassThrough(Pass));
    }
    icy_path(script)
        .or_else(|| plain_move(script))
        .or_else(|| put_then_move(script))
        .or_else(|| event_transport(script, from))
        .or_else(|| confluence(script, to))
        .or_else(|| minotaur_maze(script, to))
}

/// The gate for an upstream cost script, if an arm knows it.
#[must_use]
pub fn cost(script: &str) -> Option<Cost> {
    profession(script)
        .or_else(|| urchins(script))
        .or_else(|| only_when_travelling(script))
        .or_else(|| trinket_named(script))
        .or_else(|| remembered(script))
        .or_else(|| setting_or_month(script))
}

/// Every exit of the Confluence, 3,233 of them, is two statements: name a
/// goal, then call the one script that holds the search
/// (`Room[23282].wayto['23282']`, 4 KB, itself an exit from a room to itself
/// and never routed). The goal is the exit's own destination, or the word
/// `tranquility` for the exits that lead out of the plane.
fn confluence(script: &str, to: u32) -> Option<Crossing> {
    let [goal] = holes(
        script,
        &[
            ";e $mapdb_confluence_target = ",
            "; Room[23282].wayto['23282'].call",
        ],
    )?[..] else {
        return None;
    };
    let leave = match goal {
        "'tranquility'" => true,
        room if room.parse() == Ok(to) => false,
        _ => return None,
    };
    Some(Crossing::Routine(Routine::Confluence { leave }))
}

/// The minotaur maze: 497 exits whose whole configuration is a goal and a set
/// of rooms, followed by 1.2 KB of search that is **the same text on every
/// one** -- kept verbatim beside this file, so a change to it upstream stops
/// the arm matching like any other.
fn minotaur_maze(script: &str, to: u32) -> Option<Crossing> {
    const SEARCH: &str = include_str!("upstream_scripts/minotaur_maze.rb");
    let [target, rooms] = holes(
        script,
        &[
            ";e target_room_id = ",
            "; maze_rooms = [",
            &format!("]; {SEARCH}"),
        ],
    )?[..] else {
        return None;
    };
    (target.parse() == Ok(to)).then_some(())?;
    let rooms: Vec<RoomId> = rooms
        .split(',')
        .map(|room| room.trim().parse().map(RoomId))
        .collect::<Result<_, _>>()
        .ok()?;
    (!rooms.is_empty()).then_some(())?;
    Some(Crossing::Routine(Routine::MinotaurMaze { rooms }))
}

fn always(action: Action) -> Step {
    Step { action, when: None }
}

fn gated(when: Cond, then: &str) -> Option<Cost> {
    let then: f64 = then
        .parse()
        .ok()
        .filter(|then: &f64| then.is_finite() && *then >= 0.0)?;
    Some(Cost::Gated {
        when,
        then,
        otherwise: None,
    })
}

/// `holes`, trying the template as written and then with `"` for `'`:
/// upstream quotes both ways and means nothing by it.
fn quoted<'s>(script: &'s str, parts: &[&str]) -> Option<Vec<&'s str>> {
    holes(script, parts).or_else(|| {
        let doubled: Vec<String> = parts.iter().map(|part| part.replace('\'', "\"")).collect();
        let doubled: Vec<&str> = doubled.iter().map(String::as_str).collect();
        holes(script, &doubled)
    })
}

/// `move 'X'`, written as a script where a plain command would have done, and
/// the same with a trailing `waitrt?` -- waiting out roundtime is part of what
/// `Action::Move` means, so it adds nothing.
fn plain_move(script: &str) -> Option<Crossing> {
    const ENDINGS: [&str; 5] = ["'", "';", "'; waitrt?", "';waitrt", "'; waitrt"];
    let found = ENDINGS
        .iter()
        .find_map(|ending| quoted(script, &[";e move '", ending]))?;
    let command = found
        .first()
        .copied()
        .filter(|hole| is_plain_argument(hole))?;
    Some(Crossing::Steps(vec![always(Action::Move(
        command.to_owned(),
    ))]))
}

/// `fput 'open gate'; move 'go gate'`.
fn put_then_move(script: &str) -> Option<Crossing> {
    const FORMS: [[&str; 3]; 4] = [
        [";e fput '", "'; move '", "'"],
        [";e fput '", "';move '", "'"],
        [";e fput '", "'\nmove '", "'"],
        [";e fput '", "';move('", "')"],
    ];
    let found = FORMS.iter().find_map(|form| quoted(script, form))?;
    let [first, then] = found[..] else {
        return None;
    };
    if !is_plain_argument(first) || !is_plain_argument(then) {
        return None;
    }
    Some(Crossing::Steps(vec![
        always(Action::Put(first.to_owned())),
        always(Action::Move(then.to_owned())),
    ]))
}

/// `2.times{fput "event transport duskruin"};UserVars.mapdb_duskruin_origin = 7;`
///
/// The game asks once and goes on the second asking, so the last send is the
/// move. What is remembered is always the room being left: upstream writes
/// its id, or `Map.current.id`, and an arm that saw anything else refuses.
fn event_transport(script: &str, from: u32) -> Option<Crossing> {
    let found = holes(
        script,
        &[";e ", ".times{fput \"", "\"};UserVars.mapdb_", " = ", ";"],
    )?;
    let [times, command, name, value] = found[..] else {
        return None;
    };
    let times: usize = times.parse().ok().filter(|times| (1..=3).contains(times))?;
    if !is_plain_argument(command) || !is_word(name) {
        return None;
    }
    if value != "Map.current.id" && value.parse() != Ok(from) {
        return None;
    }
    let mut steps = vec![always(Action::Put(command.to_owned())); times - 1];
    steps.push(always(Action::Move(command.to_owned())));
    steps.push(always(Action::Remember(name.to_owned(), from.to_string())));
    Some(Crossing::Steps(steps))
}

/// The urchin guides (`plan/21` §4.1). Upstream compares an expiry against
/// `Time.now`; here "paid for and not expired" is one flag the planner works
/// out, because nothing in the map's vocabulary reads a clock. `mounted` is
/// added: upstream switches the whole setting off when it learns the walker
/// is mounted, which comes to the same thing.
fn urchins(script: &str) -> Option<Cost> {
    let [seconds] = holes(
        script,
        &[
            ";e UserVars.mapdb_use_urchins == true and !UserVars.mapdb_urchins_expire.nil? and \
             Time.now.to_i < UserVars.mapdb_urchins_expire and !hidden? and !invisible? ? ",
            " : nil;",
        ],
    )?[..] else {
        return None;
    };
    let flag = |name: &str| Cond::Flag(name.to_owned());
    let not = |cond| Cond::Not(Box::new(cond));
    gated(
        Cond::All(vec![
            Cond::Setting("use_urchins".to_owned(), "true".to_owned()),
            flag("urchin_access"),
            not(flag("hidden")),
            not(flag("invisible")),
            not(flag("mounted")),
        ]),
        seconds,
    )
}

/// `!(Script.list.map(&:name) & %w{go2 route2}).empty? ? 0.1 : nil` -- "only
/// while go2 is running", which keeps a person stepping through the map by
/// hand out of an urchin hub's exits. A cost is only ever priced *for* a
/// planned walk here, so the test is always true and the cost is constant.
fn only_when_travelling(script: &str) -> Option<Cost> {
    let [seconds] = holes(
        script,
        &[
            ";e !(Script.list.map(&:name) & %w{go2 route2}).empty? ? ",
            " : nil;",
        ],
    )?[..] else {
        return None;
    };
    let seconds: f64 = seconds.parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then_some(Cost::Fixed(seconds))
}

/// The Mist Harbor trinket: usable once the profile names one.
fn trinket_named(script: &str) -> Option<Cost> {
    let [seconds] = holes(
        script,
        &[
            ";e (!UserVars.mapdb_fwi_trinket.nil? and !UserVars.mapdb_fwi_trinket.empty?) ? ",
            " : nil;",
        ],
    )?[..] else {
        return None;
    };
    gated(Cond::SettingIsSet("fwi_trinket".to_owned()), seconds)
}

/// `Stats.prof == 'Bard' ? 0.2 : nil`, four ways. Two of them add
/// `!defined?(Stats.prof) or`, which lets a walker of unknown profession
/// through; here unknown is impassable like every other unknown.
fn profession(script: &str) -> Option<Cost> {
    const FORMS: [[&str; 3]; 4] = [
        [";e Stats.prof == '", "' ? ", " : nil"],
        [
            ";e ((!defined?(Stats.prof) or Stats.prof == '",
            "') ? ",
            " : nil);",
        ],
        [
            ";e (!defined?(Stats.prof) or Stats.prof == '",
            "') ? ",
            " : nil",
        ],
        [";e if Stats.prof == '", "'; ", "; else; nil; end"],
    ];
    let found = FORMS.iter().find_map(|form| quoted(script, form))?;
    let [name, seconds] = found[..] else {
        return None;
    };
    is_word(name).then_some(())?;
    gated(Cond::Profession(name.to_owned()), seconds)
}

/// The way back from an event ground or Mist Harbor: open only to where the
/// walker came in from (`plan/21` §4.4).
fn remembered(script: &str) -> Option<Cost> {
    const GUARDED: [&str; 5] = [
        ";e (!UserVars.mapdb_",
        ".nil? and UserVars.mapdb_",
        " == ",
        ") ? ",
        " : nil;",
    ];
    const BARE: [&str; 4] = [";e (UserVars.mapdb_", " == ", " ? ", " : nil);"];
    let (name, value, seconds) = if let Some(found) = holes(script, &GUARDED) {
        let [name, again, value, seconds] = found[..] else {
            return None;
        };
        (name == again).then_some((name, value, seconds))?
    } else {
        let [name, value, seconds] = holes(script, &BARE)?[..] else {
            return None;
        };
        (name, value, seconds)
    };
    if !is_word(name) || value.parse::<u32>().is_err() {
        return None;
    }
    gated(Cond::Remembered(name.to_owned(), value.to_owned()), seconds)
}

fn setting_or_month(script: &str) -> Option<Cost> {
    if let Some(found) = holes(script, &[";e UserVars.mapdb_", " == true ? ", " : nil"]) {
        let [name, seconds] = found[..] else {
            return None;
        };
        is_word(name).then_some(())?;
        return gated(Cond::Setting(name.to_owned(), "true".to_owned()), seconds);
    }
    let found = holes(script, &[";e Time.now.month == ", " ? ", " : nil"])?;
    let [month, seconds] = found[..] else {
        return None;
    };
    let month: u32 = month
        .parse()
        .ok()
        .filter(|month| (1..=12).contains(month))?;
    gated(Cond::Month(month), seconds)
}

/// Match `script` against literal parts with a hole between each pair, and
/// return what filled the holes. `["a", "b"]` has one hole.
fn holes<'s>(script: &'s str, parts: &[&str]) -> Option<Vec<&'s str>> {
    let (first, rest) = parts.split_first()?;
    let mut remaining = script.strip_prefix(first)?;
    let mut found = Vec::with_capacity(rest.len());
    for (index, part) in rest.iter().enumerate() {
        let at = if index + 1 == rest.len() {
            // The last part must end the script, so a hole cannot swallow a
            // second statement that happens to end the same way.
            remaining.len().checked_sub(part.len())?
        } else {
            remaining.find(part)?
        };
        let (hole, after) = remaining.split_at_checked(at)?;
        found.push(hole);
        remaining = after.strip_prefix(part)?;
    }
    remaining.is_empty().then_some(found)
}

/// A hole that is an identifier: a variable's name, a profession.
fn is_word(hole: &str) -> bool {
    !hole.is_empty() && hole.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// A hole that is one quoted word: no quote, no statement separator.
fn is_plain_argument(hole: &str) -> bool {
    !hole.is_empty() && !hole.contains(['\'', '"', ';', '\n', '#'])
}

/// The icy paths: 171 exits in two upstream shapes.
///
/// **What upstream does**, and where this departs from it. The common shape
/// (150) waits four seconds when the walker is heavy or unskilled and not
/// hasted. The other (21, the glacier) waits six on a simpler test, and
/// otherwise casts Sigil of Resolve if it can.
///
/// **One profile setting, `ice_mode`: `run`, `wait` or `auto`** -- go2's
/// values, kept by the author's choice. `run` does nothing special: no cast,
/// no wait, just the move. `wait` always waits. `auto` waits when the shape's
/// own test says the walker is likely to slip.
///
/// **The author's rule: all 171 cast Sigil of Resolve when it is known and
/// affordable -- unless `ice_mode` is `run`.** So both shapes become the same three steps -- cast,
/// wait, move -- each keeping its own wait and its own test for it. The cast
/// comes first and does not depend on the wait: Resolve is what makes the
/// crossing likelier to succeed either way.
///
/// Dropped: the `echo`, which talks to a Lich user; and the glacier shape's
/// reaction to `Rushing heedlessly` (cast Haste, stand, replan) -- a fall is
/// the `move` step's to recover from, as it is on any other exit.
///
/// **A guard that cannot be answered does not fire** (`cena_map::cond`), so a
/// walker whose skills are unknown neither casts nor waits. That errs toward a
/// fall, which the walk recovers from, rather than toward refusing a route.
fn icy_path(script: &str) -> Option<Crossing> {
    const TRAIL: &str = ";e if (UserVars.mapdb_ice_mode == 'wait') or \
        ((UserVars.mapdb_ice_mode != 'run') and ((XMLData.encumbrance_value > 50) or \
        ((Skills.survival < 50) and not Spell['Haste'].active?))); \
        sleep 0.2; echo 'trying not to slip...'; sleep 4; end; move '";
    const GLACIER: &str = ";e \n\t\tresolve=Spell['Sigil of Resolve']\n\t\thaste=Spell['Haste']\n\t\t\
        if UserVars.mapdb_ice_mode == 'wait' || Skills.survival < 50 || \
        XMLData.encumbrance_value >= 50\n\t\t\techo 'trying not to slip...'; sleep 6\n\t\t\
        elsif resolve.known? && resolve.affordable? && !resolve.active?\n\t\t\tresolve.cast\n\t\t\
        end\n\t\tresult = fput '";
    const GLACIER_AFTER: &str = "'\n\t\tif result =~ /^Rushing heedlessly/\n\t\t\t\
        haste.cast if haste.known? && haste.affordable? && !haste.active?\n\t\t\t\
        fput 'stand'\n\t\t\t$go2_restart = true\n\t\tend\n\t";

    // go2's own words, kept (author, 2026-09-20: "more informative" than
    // off/on). The third value, `auto`, is never tested for: it is what is left.
    const RUN: &str = "run";
    const WAIT: &str = "wait";
    let setting = |value: &str| Cond::Setting("ice_mode".to_owned(), value.to_owned());
    let unskilled = Cond::SkillUnder("survival".to_owned(), 50);
    let (direction, pause, slippery) = if let Some(found) = holes(script, &[TRAIL, "'"]) {
        let heavy_or_slow = Cond::Any(vec![
            Cond::EncumbranceOver(50),
            Cond::All(vec![
                unskilled,
                Cond::Not(Box::new(Cond::SpellActive("Haste".to_owned()))),
            ]),
        ]);
        let unless_running = Cond::All(vec![Cond::Not(Box::new(setting(RUN))), heavy_or_slow]);
        let slippery = Cond::Any(vec![setting(WAIT), unless_running]);
        (*found.first()?, 4200, slippery)
    } else {
        let found = holes(script, &[GLACIER, GLACIER_AFTER])?;
        // `>= 50` upstream, and whole percents: over 49.
        let slippery = Cond::Any(vec![setting(WAIT), unskilled, Cond::EncumbranceOver(49)]);
        (*found.first()?, 6000, slippery)
    };
    if !is_plain_argument(direction) {
        return None;
    }

    let resolve = || "Sigil of Resolve".to_owned();
    let can_cast = Cond::All(vec![
        Cond::Not(Box::new(setting(RUN))),
        Cond::SpellKnown(resolve()),
        Cond::SpellAffordable(resolve()),
        Cond::Not(Box::new(Cond::SpellActive(resolve()))),
    ]);
    let step = |action, when| Step { action, when };
    Some(Crossing::Steps(vec![
        step(Action::Cast(resolve()), Some(can_cast)),
        step(Action::Pause(pause), Some(slippery)),
        step(Action::Move(direction.to_owned()), None),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holes_are_exact_at_both_ends() {
        assert_eq!(holes("move 'west'", &["move '", "'"]), Some(vec!["west"]));
        // The last part anchors at the end, so a trailing statement lands IN
        // the hole -- where `is_plain_argument` refuses it.
        let smuggled = holes("move 'west'; fput 'x'", &["move '", "'"]).unwrap();
        assert_eq!(smuggled, ["west'; fput 'x"]);
        assert!(!is_plain_argument(smuggled[0]));
        assert_eq!(holes("xmove 'west'", &["move '", "'"]), None);
        assert_eq!(holes("a=1;b=2;", &["a=", ";b=", ";"]), Some(vec!["1", "2"]));
        assert_eq!(holes("anything", &[]), None);
    }

    #[test]
    fn an_argument_cannot_smuggle_a_second_statement() {
        assert!(is_plain_argument("go bridge"));
        assert!(!is_plain_argument("west'; fput 'quit"));
        assert!(!is_plain_argument(""));
    }

    fn steps(script: &str, from: u32) -> Vec<Action> {
        match crossing(script, from, 0) {
            Some(Crossing::Steps(steps)) => steps.into_iter().map(|step| step.action).collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn a_scripted_plain_move_is_a_move_however_it_is_quoted_or_ended() {
        let go = vec![Action::Move("go thinness".into())];
        assert_eq!(steps(";e move 'go thinness'", 1), go);
        assert_eq!(steps(";e move \"go thinness\"; waitrt?", 1), go);
        assert_eq!(steps(";e move 'go thinness';waitrt", 1), go);
        // Anything after it that is not a known ending is not this shape.
        assert_eq!(steps(";e move 'go thinness'; fput 'quit'", 1), vec![]);
    }

    #[test]
    fn a_put_then_a_move() {
        assert_eq!(
            steps(";e fput 'open gate'\nmove 'go gate'", 1),
            vec![
                Action::Put("open gate".into()),
                Action::Move("go gate".into())
            ]
        );
    }

    /// The transport asks once and goes on the second asking; what is
    /// remembered is the room left, whichever way upstream wrote it.
    #[test]
    fn an_event_transport_remembers_the_room_it_left() {
        let expected = vec![
            Action::Put("event transport duskruin".into()),
            Action::Move("event transport duskruin".into()),
            Action::Remember("duskruin_origin".into(), "7".into()),
        ];
        let literal = ";e 2.times{fput \"event transport duskruin\"};\
                       UserVars.mapdb_duskruin_origin = 7;";
        let current = ";e 2.times{fput \"event transport duskruin\"};\
                       UserVars.mapdb_duskruin_origin = Map.current.id;";
        assert_eq!(steps(literal, 7), expected);
        assert_eq!(steps(current, 7), expected);
        assert_eq!(steps(literal, 8), vec![], "it names another room");
    }

    fn gate(script: &str) -> Option<(Cond, f64)> {
        match cost(script) {
            Some(Cost::Gated {
                when,
                then,
                otherwise: None,
            }) => Some((when, then)),
            _ => None,
        }
    }

    #[test]
    fn a_profession_gate_is_one_gate_however_upstream_wrote_it() {
        for script in [
            ";e Stats.prof == 'Bard' ? 0.2 : nil",
            ";e ((!defined?(Stats.prof) or Stats.prof == 'Bard') ? 0.2 : nil);",
            ";e if Stats.prof == \"Bard\"; 0.2; else; nil; end",
        ] {
            assert_eq!(
                gate(script),
                Some((Cond::Profession("Bard".into()), 0.2)),
                "{script}"
            );
        }
        assert_eq!(gate(";e Stats.prof == 'Bard' ? -1 : nil"), None);
    }

    #[test]
    fn the_way_back_is_gated_on_the_memory_the_way_in_wrote() {
        assert_eq!(
            gate(
                ";e (!UserVars.mapdb_duskruin_origin.nil? and \
                 UserVars.mapdb_duskruin_origin == 7) ? 0.2 : nil;"
            ),
            Some((Cond::Remembered("duskruin_origin".into(), "7".into()), 0.2))
        );
        assert_eq!(
            gate(
                ";e (!UserVars.mapdb_duskruin_origin.nil? and \
                 UserVars.mapdb_talondown_origin == 7) ? 0.2 : nil;"
            ),
            None,
            "two different variables is not this shape"
        );
        assert_eq!(
            gate(";e (UserVars.mapdb_fwi_return_room == 3668 ? 5 : nil);"),
            Some((
                Cond::Remembered("fwi_return_room".into(), "3668".into()),
                5.0
            ))
        );
    }

    #[test]
    fn settings_and_months() {
        assert_eq!(
            gate(";e UserVars.mapdb_use_portmasters == true ? 1200 : nil"),
            Some((
                Cond::Setting("use_portmasters".into(), "true".into()),
                1200.0
            ))
        );
        assert_eq!(
            gate(";e Time.now.month == 10 ? 0.2 : nil"),
            Some((Cond::Month(10), 0.2))
        );
        assert_eq!(gate(";e Time.now.month == 13 ? 0.2 : nil"), None);
    }

    #[test]
    fn the_confluence_is_a_routine_whose_goal_is_the_exit() {
        let inside = ";e $mapdb_confluence_target = 23290; Room[23282].wayto['23282'].call";
        let out = ";e $mapdb_confluence_target = 'tranquility'; Room[23282].wayto['23282'].call";
        assert_eq!(
            crossing(inside, 23282, 23290),
            Some(Crossing::Routine(Routine::Confluence { leave: false }))
        );
        assert_eq!(
            crossing(out, 23282, 188),
            Some(Crossing::Routine(Routine::Confluence { leave: true }))
        );
        assert_eq!(
            crossing(inside, 23282, 23291),
            None,
            "a goal that is not this exit"
        );
    }

    #[test]
    fn the_minotaur_maze_is_a_routine_only_with_upstreams_exact_search() {
        let search = include_str!("upstream_scripts/minotaur_maze.rb");
        let script = format!(";e target_room_id = 6192; maze_rooms = [6191, 6254, 6192]; {search}");
        assert_eq!(
            crossing(&script, 6191, 6192),
            Some(Crossing::Routine(Routine::MinotaurMaze {
                rooms: vec![RoomId(6191), RoomId(6254), RoomId(6192)]
            }))
        );
        assert_eq!(
            crossing(&script, 6191, 6254),
            None,
            "the goal is not this exit"
        );
        let edited = script.replace("sleep 0.1", "sleep 0.2");
        assert_eq!(
            crossing(&edited, 6191, 6192),
            None,
            "upstream changed the search"
        );
    }
}
