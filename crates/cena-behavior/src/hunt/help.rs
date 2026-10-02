//! What `hunt help`, `heal help`, `waggle help`, `sc help` and `loot help`
//! say, a line each: the families' commands, and how a setting is changed.
//! Moved down from `command.rs` at its cap.

use super::command::Topic;

/// What a family's `help` says, a line each.
#[must_use]
pub const fn help(topic: Topic) -> &'static [&'static str] {
    match topic {
        Topic::Hunt => HUNT_HELP,
        Topic::Heal => HEAL_HELP,
        Topic::Waggle => WAGGLE_HELP,
        Topic::Sc => SC_HELP,
        Topic::Loot => LOOT_HELP,
    }
}

const LOOT_HELP: &[&str] = &[
    "loot                                   skin and search the dead here, take what the floor holds",
    "loot skin                              only skin the dead here",
    "loot box                               empty the open box in hand, then keep it or throw it out (in the pool's bin when none is here)",
    "loot ground                            the same for each box on the ground; a locked one stays there",
    "loot sell                              the selling round: the pool, the shops, the bank, and back",
    "loot sell type <kinds>                 only things of these kinds: loot sell type gem, skin",
    "loot sell shop <shops>                 only these: gemshop, pawnshop, furrier, collectibles, chronomage",
    "loot sell item <names>                 only things so named: loot sell item blue crystal, silver wand",
    "                                       each sells only what the profile sells; boxes go to the pool only for `type box`",
    "loot pool | pool deposit | pool return the locksmith pool alone: both, only give boxes, only collect them; the bank after a drop-off",
    "loot deposit                           the bank alone, keeping the silver the profile says",
    "loot last                              what the last selling round came to, shop by shop",
    "loot reset unskinnable [creature]      forget the creatures learned unskinnable, or one of them",
    "loot show                              the loot settings, skinning and selling with them",
    "loot set <setting> <value>             change one: loot set town.sell_keep_silver 5000",
    "loot unset <setting>                   back to its default",
    "loot summary | recent | boxes | creatures | cap   reports on what was recorded",
    "A hunt loots and sells by the same settings. `stop` or `hunt stop` stops one under way.",
];

const HUNT_HELP: &[&str] = &[
    "hunt <profile>                         hunt on a profile",
    "hunt <profile> quick | bounty          clear this room | hunt until the bounty is done",
    "hunt <profile> with <name> <name>...   lead these characters, each hunting its own <profile>",
    "hunt stop                              stop",
    "hunt list                              the profiles there are",
    "hunt check <profile>                   read it as this character will run it: what is wrong, what is held",
    "hunt show <profile> [setting]          every setting, or those under one: hunt show ojandhaart rest",
    "hunt set <profile> <setting> <value>   change one: hunt set ojandhaart rooms.resting 29877",
    "hunt unset <profile> <setting>         take one out, so the default decides it",
    "hunt import <bigshot yaml> [as <name>] bring in a bigshot profile",
    "hunt import-loot <eloot yaml>          bring in eloot's settings as this character's loot profile",
    "hunt setup                             where the map's setup page is",
    "A value is on or off, a number, a list [\"a\", \"b\"], a table { name = \"warg\", routine = \"a\" }, or words.",
    "A setting in a list is picked by number from 1: hunt set ojandhaart targets.2.routine c",
    "heal help, waggle help, keep list, go2 help: the other behaviors. `help` lists everything.",
];

const HEAL_HELP: &[&str] = &[
    "heal [spellcast] [ranged] [blood]      heal with herbs: everything, or only what stops a cast, a shot, or the blood",
    "heal show                              the heal settings, the defaults included",
    "heal set <setting> <value>             change one: heal set container herb pouch",
    "heal unset <setting>                   back to its default",
    "heal stock | fill                      stock the herb container at the herbalist | buy one of each herb it lacks",
    "A hunt heals at every rest once a container is set. `hunt stop` stops a heal under way.",
];

const SC_HELP: &[&str] = &[
    "sc <spell|alias> [target] [count]      cast it: sc 401, sc 903 kobold, sc 111 3",
    "sc alias <spell> <name>                call a spell by a name of yours: sc alias 211 bravery",
    "sc verb <spell> <verb>                 cast it with this verb (channel, evoke, incant...)",
    "sc stance <spell> <stance>             take this stance to cast it",
    "sc set typed on|off                    cast a bare 401 or alias typed with no sc (on by default)",
    "sc set conserve|safety|channel|stance on|off   keep mana, need a target, channel attacks, stance",
    "A cast goes beside a running hunt, never in its place. The Spellcaster page in Settings has every setting.",
];

const WAGGLE_HELP: &[&str] = &[
    "waggle [name] [name]...                cast the waggle spells on these people, or yourself",
    "waggle show                            the waggle settings, the defaults included",
    "waggle set <setting> <value>           change one: waggle set cast_list [101, 107, 401]",
    "waggle unset <setting>                 back to its default",
    "`hunt stop` stops a waggle under way.",
];
