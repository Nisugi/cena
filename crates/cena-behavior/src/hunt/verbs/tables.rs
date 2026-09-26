//! The tables bigshot's handlers dispatch on, each with the line it came
//! from.

/// Combat maneuvers, `cman <step> #id`, with the name the Cooldowns dialog
/// lists: bigshot's tables for `cmd_cmans` (`bigshot.lic:5038-5061`),
/// `cmd_bearhug`, and the rogue's `cmd_rogue_cmans` (`:5318-5333`).
pub(super) const CMANS: &[(&str, &str)] = &[
    ("bullrush", "Bull Rush"),
    ("coupdegrace", "Coup de Grace"),
    ("cpress", "Crowd Press"),
    ("dirtkick", "Dirtkick"),
    ("disarm", "Disarm Weapon"),
    ("exsanguinate", "Exsanguinate"),
    ("feint", "Feint"),
    ("gkick", "Groin Kick"),
    ("hamstring", "Hamstring"),
    ("haymaker", "Haymaker"),
    ("headbutt", "Headbutt"),
    ("kifocus", "Ki Focus"),
    ("leapattack", "Leap Attack"),
    ("mblow", "Mighty Blow"),
    ("sattack", "Spin Attack"),
    ("sbash", "Shield Bash"),
    ("sblow", "Staggering Blow"),
    ("scleave", "Spell Cleave"),
    ("sthieve", "Spell Thieve"),
    ("sunder", "Sunder Shield"),
    ("tackle", "Tackle"),
    ("trip", "Trip"),
    ("truestrike", "True Strike"),
    ("vaultkick", "Vault Kick"),
    ("bearhug", "Bearhug"),
    ("cutthroat", "Cutthroat"),
    ("divert", "Divert"),
    ("shroud", "Dust Shroud"),
    ("eviscerate", "Eviscerate"),
    ("eyepoke", "Eyepoke"),
    ("footstomp", "Footstomp"),
    ("garrote", "Garrote"),
    ("kneebash", "Kneebash"),
    ("mug", "Mug"),
    ("nosetweak", "Nosetweak"),
    ("spunch", "Sucker Punch"),
    ("subdue", "Subdue"),
    ("sweep", "Sweep"),
    ("swiftkick", "Swiftkick"),
    ("templeshot", "Templeshot"),
    ("throatchop", "Throatchop"),
];

/// Weapon techniques, `weapon <step> #id` (`:4623-4628`, `:4728-4738`).
pub(super) const WEAPONS: &[(&str, &str)] = &[
    ("barrage", "Barrage"),
    ("flurry", "Flurry"),
    ("fury", "Fury"),
    ("gthrusts", "Guardant Thrusts"),
    ("pummel", "Pummel"),
    ("thrash", "Thrash"),
    ("charge", "Charge"),
    ("clash", "Clash"),
    ("cripple", "Cripple"),
    ("cyclone", "Cyclone"),
    ("dizzyingswing", "Dizzying Swing"),
    ("pindown", "Pin Down"),
    ("pulverize", "Pulverize"),
    ("twinhammer", "Twin Hammerfists"),
    ("volley", "Volley"),
    ("wblade", "Whirling Blade"),
    ("whirlwind", "Whirlwind"),
];

/// The shield moves after `shield` (`:4076`).
pub(super) const SHIELD_MOVES: &[&str] = &[
    "throw", "bash", "charge", "strike", "pin", "trample", "push",
];

/// Warcries and the stamina each needs (`cmd_warrior_shouts`, `:4913-4921`).
pub(super) const WARCRIES: &[(&str, i32)] = &[
    ("shout", 25),
    ("yowlp", 11),
    ("holler", 31),
    ("bellow all", 21),
    ("bellow", 11),
    ("growl all", 15),
    ("growl", 8),
    ("cry all", 31),
    ("cry", 16),
];

/// What a plant spell leaves in the room: Tangleweed is not cast again
/// while one is here (`cmd_weed`, `:5754`).
pub(super) const PLANTS: &[&str] = &[
    "vine",
    "bramble",
    "widgeonweed",
    "vathor club",
    "swallowwort",
    "smilax",
    "creeper",
    "ivy",
    "tumbleweed",
];

/// Spells cast on oneself, whatever the target (`spell_is_selfcast?`).
pub(super) const SELF_CAST: &[u16] = &[
    106, 109, 115, 117, 120, 130, 140, 205, 206, 211, 213, 215, 218, 219, 220, 240, 303, 307, 310,
    313, 314, 319, 350, 401, 402, 403, 404, 405, 406, 414, 418, 419, 425, 430, 503, 506, 507, 508,
    509, 511, 513, 515, 517, 520, 535, 540, 601, 602, 604, 605, 606, 608, 612, 613, 617, 618, 620,
    625, 630, 640, 650, 707, 712, 905, 911, 913, 916, 919, 1003, 1006, 1007, 1009, 1010, 1011,
    1012, 1014, 1017, 1018, 1019, 1020, 1025, 1035, 1040, 1109, 1119, 1125, 1130, 1150, 1202, 1204,
    1208, 1213, 1214, 1215, 1216, 1220, 1235, 1601, 1605, 1606, 1607, 1608, 1609, 1610, 1611, 1612,
    1613, 1616, 1617, 1618, 1619, 1635,
];

/// Short buffs `cmd_spell` does not cast while their name is cooling
/// (`bigshot.lic:5864`).
pub(super) const SHORT_BUFFS: &[u16] = &[140, 211, 215, 219, 919, 1619, 1650];

/// Cast with no target whatever the step says: Celerity and 902.
pub(super) const UNAIMED: &[u16] = &[506, 902];

/// bigshot verbs not sent yet, by their first word.
pub(super) const UNPORTED: &[&str] = &["nudgeweapon", "nudgeweapons", "unarmed", "mstrike"];
