//! Arms for upstream cost scripts: who may use an exit, and at what price.

use cena_map::{Cond, Cost};

use super::{holes, is_word, quoted};

/// `$mapdb_instability_timeto[188]`: 477 costs, nine towns from each of the
/// Confluence's 53 rooms. go2 fills that global when the walker enters the
/// plane -- the walk from the instability it came in by to each town --
/// because leaving puts it back at that instability. Here the planner does
/// the same into `Walker::tables`, and the map says only which entry.
pub(super) fn instability(script: &str) -> Option<Cost> {
    let [town] = holes(script, &[";e $mapdb_instability_timeto[", "]"])?[..] else {
        return None;
    };
    Some(Cost::Table {
        table: "instability".to_owned(),
        key: cena_map::RoomId(town.parse().ok()?),
    })
}

pub(super) fn gated(when: Cond, then: &str) -> Option<Cost> {
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

/// The urchin guides (`plan/21` §4.1). Upstream compares an expiry against
/// `Time.now`; here "paid for and not expired" is one flag the planner works
/// out, because nothing in the map's vocabulary reads a clock. `mounted` is
/// added: upstream switches the whole setting off when it learns the walker
/// is mounted, which comes to the same thing.
pub(super) fn urchins(script: &str) -> Option<Cost> {
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
pub(super) fn only_when_travelling(script: &str) -> Option<Cost> {
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
pub(super) fn trinket_named(script: &str) -> Option<Cost> {
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
pub(super) fn profession(script: &str) -> Option<Cost> {
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
pub(super) fn remembered(script: &str) -> Option<Cost> {
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

pub(super) fn setting_or_month(script: &str) -> Option<Cost> {
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
