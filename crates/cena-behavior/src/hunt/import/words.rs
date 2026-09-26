//! bigshot's guard words as Hydra's (`plan/33`), one token at a time.
//!
//! Each row is bigshot's own branch read for when the step **runs**, since
//! bigshot's answer the other question, "skip?" (`command_check`,
//! `bigshot.lic:4217-4294`). A word takes its `!` across unless the row
//! says otherwise.
//!
//! | bigshot | Hydra | from |
//! |---|---|---|
//! | `hN`, `mN`, `sN`, `vN`, `eN` | `health_at_least N`, `mana_at_least N`, `stamina_at_least N`, `spirit_at_least N`, `encumbrance_at_least N` | the amount checks, `:3204-3225`, which answer skip |
//! | `kN` | `self_kneeling` (the number is ignored, as bigshot ignores it) | `:3212` |
//! | `mobN`, `validN` | `targets_at_least N`; the `!` form is `targets_at_most N` | `:3216`, `:3224`: `<` against `>` |
//! | `tier1`-`tier3` | `position N`; `!tierN` is `!position_at_least N` | both the amount check and the `tierN` branch apply |
//! | `tierN` otherwise | `position_at_least N`; `!tierN` is `!position_at_least N+1` | the amount check alone |
//! | `ucsdecent`, `ucsgood`, `ucsexcellent` | `position 1`, `2`, `3` | `:4487-4492` |
//! | `thpN`, `wounded` | `thp N`, `thp 25` | `:4336`; `wounded` is `thp25` exactly (`creature.rb:607`) |
//! | `empoweredN` | `empowered_below N` | `:4319` skips when an Empowered of +N or more is up |
//! | `buffN` on a verb | `expiring "<its buff>" N` | `:4240` and the verb's buff, `:3228-3241` (see `guard.rs` for why `expiring`) |
//! | `repeatdelayN`, and `!repeatdelayN` | `every N` | `:4270`; the regex is not anchored, so the `!` changes nothing |
//! | `EB"…"`, `ES"…"`, `EC"…"`, `ED"…"` | `buff`, `spell`, `cooldown`, `debuff` `"…"` | `:4298`; a name, not a pattern (below) |
//! | the 21 effect words | `buff "<name>"` or `spell "<name>"` | `:4346-4391`, below |
//! | `frozen`, `prone`, `rooted`, `voidweaver` | `!immobilized`, `!down`, `!rooted`, `!buff "Voidweaver"` | bigshot reads these four the other way round (`plan/33` §1) |
//! | `pcs`, `room` | `alone`, `once_here` | renamed (`plan/33` §2b, §2g) |
//! | `censer` | no guard: the profile's `censer_between_actions` | `plan/33` §2h, the author's reading |
//! | `essence`, `justice` | held | the model does not capture them yet (`plan/33`, **later**) |
//! | every other word bigshot knows | the same word | `plan/33` §2 |
//!
//! **An effect's text is a name, not a pattern.** bigshot compiles it into
//! a regex (`:4302`); Hydra matches the start of the displayed name. A `\`
//! escape is read as the character it escapes and a bare `.` as itself,
//! which is what a person writing `Enh. Strength` meant. Any other pattern
//! character holds the step rather than guess.
//!
//! `!506` and `!celerity` mean "not in Celerity's last three seconds"
//! (`:4347`): down, or up with more than 3 s left. That is an either-or no
//! single guard says, so they hold.

/// Every guard word bigshot accepts (`bigshot.lic:3197`), bare: without
/// `!`, without the number an amount word takes, without the quoted text
/// an `E?"…"` word takes. MEASURED: 87 (`plan/33`).
const BIGSHOT_WORDS: &[&str] = &[
    "506",
    "ancient",
    "animate",
    "ascended",
    "ascension_boss",
    "barrage",
    "bearhug",
    "buff",
    "burst",
    "calm",
    "celerity",
    "censer",
    "challenging",
    "coupdegrace",
    "disease",
    "disengaged",
    "disoriented",
    "EB",
    "EC",
    "ED",
    "ES",
    "empowered",
    "e",
    "essence",
    "fatalcrit",
    "frozen",
    "flurry",
    "flying",
    "fury",
    "garrote",
    "h",
    "hidden",
    "holler",
    "hovering",
    "immobilized",
    "inferior",
    "justice",
    "k",
    "kneeling",
    "m",
    "mini_boss",
    "mob",
    "momentum",
    "mount",
    "noncorporeal",
    "once",
    "outside",
    "pcs",
    "poison",
    "prone",
    "pummel",
    "rapid",
    "rebuke",
    "reflex",
    "repeatdelay",
    "rider",
    "room",
    "rooted",
    "s",
    "scourge",
    "shout",
    "sitting",
    "sleeping",
    "smote",
    "splashy",
    "stunned",
    "surge",
    "sympathetic",
    "tailwind",
    "tier",
    "tier1",
    "tier2",
    "tier3",
    "thp",
    "thrash",
    "ucsdecent",
    "ucsexcellent",
    "ucsgood",
    "ucstierup",
    "undead",
    "v",
    "valid",
    "vigor",
    "voidweaver",
    "webbed",
    "wounded",
    "yowlp",
];

/// The buff each verb grants, for `buffN` (`bigshot.lic:3228-3241`).
/// `coupdegrace`'s is a pattern, `Empowered (+N)`, and is not here: a
/// pattern is not a name.
const BUFF_OF: &[(&str, &str)] = &[
    ("barrage", "Enh. Dexterity (+10)"),
    ("bearhug", "Enh. Strength (+10)"),
    ("flurry", "Slashing Strikes"),
    ("fury", "Enh. Constitution (+10)"),
    ("garrote", "Enh. Agility (+10)"),
    ("kweed", "Tangleweed Vigor"),
    ("pummel", "Concussive Blows"),
    ("shout", "Empowered (+20)"),
    ("thrash", "Forceful Blows"),
    ("weed", "Tangleweed Vigor"),
    ("yowlp", "Yertie's Yowlp"),
];

/// The words bigshot fixed an effect name into (`:4346-4391`), as
/// `plan/33` §2c merges them: the word, and the guard it runs on.
/// `bearhug`, `burst` and `surge` read any Enh. Strength or Dexterity, so
/// their name is the stem.
const EFFECT_WORDS: &[(&str, &str)] = &[
    ("506", "spell \"Celerity\""),
    ("celerity", "spell \"Celerity\""),
    ("animate", "spell \"Animate Dead\""),
    ("barrage", "buff \"Enh. Dexterity (+10)\""),
    ("bearhug", "buff \"Enh. Strength\""),
    ("burst", "buff \"Enh. Dexterity\""),
    ("coupdegrace", "buff \"Empowered\""),
    ("flurry", "buff \"Slashing Strikes\""),
    ("fury", "buff \"Enh. Constitution (+10)\""),
    ("garrote", "buff \"Enh. Agility (+10)\""),
    ("holler", "buff \"Enh. Health (+20)\""),
    ("momentum", "buff \"Glorious Momentum\""),
    ("pummel", "buff \"Concussive Blows\""),
    ("rapid", "buff \"Rapid Fire\""),
    ("rebuke", "buff \"Righteous Rebuke\""),
    ("scourge", "buff \"Ardor of the Scourge\""),
    ("shout", "buff \"Empowered (+20)\""),
    ("surge", "buff \"Enh. Strength\""),
    ("tailwind", "buff \"Breeze Archery Tailwind\""),
    ("thrash", "buff \"Forceful Blows\""),
    ("vigor", "buff \"Tangleweed Vigor\""),
    ("yowlp", "buff \"Yertie's Yowlp\""),
    // The Buffs dialog cuts the name off (`plan/33` §2c, MEASURED).
    ("reflex", "buff \"Nature's Touch Arcane Ref\""),
];

/// `!burst` and `!surge` read a cooldown, not the buff (`:4355`, `:4381`).
const COOLDOWN_OF: &[(&str, &str)] = &[
    ("burst", "Burst of Swiftness"),
    ("surge", "Surge of Strength"),
];

/// Words carried under a new name, `!` across.
const RENAMED: &[(&str, &str)] = &[("pcs", "alone"), ("room", "once_here")];

/// Words bigshot reads the other way round (`plan/33` §1): the Hydra word
/// the bare form is the negation of.
const FLIPPED: &[(&str, &str)] = &[
    ("frozen", "immobilized"),
    ("prone", "down"),
    ("rooted", "rooted"),
    ("voidweaver", "buff \"Voidweaver\""),
];

/// Words carried as they are, `!` across.
const SAME: &[&str] = &[
    "hidden",
    "disease",
    "poison",
    "outside",
    "splashy",
    "stunned",
    "webbed",
    "sleeping",
    "calm",
    "disoriented",
    "kneeling",
    "sitting",
    "flying",
    "hovering",
    "immobilized",
    "undead",
    "noncorporeal",
    "ancient",
    "ascended",
    "ascension_boss",
    "challenging",
    "disengaged",
    "inferior",
    "mini_boss",
    "mount",
    "rider",
    "sympathetic",
    "fatalcrit",
    "smote",
    "ucstierup",
    "once",
];

/// What a bigshot guard word becomes.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Word {
    /// A Hydra guard, as written.
    Guard(String),
    /// `censer`: no guard, but the profile casts the censer between steps.
    Censer,
}

/// One bigshot guard token as Hydra's, or why it cannot be.
pub(super) fn translate(verb: &str, token: &str) -> Result<Word, String> {
    let (bang, word) = token
        .strip_prefix('!')
        .map_or(("", token), |word| ("!", word));
    let flip = if bang.is_empty() { "!" } else { "" };
    let lower = word.to_ascii_lowercase();
    let guard = |text: String| Ok(Word::Guard(text));
    let never = || {
        Err(format!(
            "`{token}` matches no branch in bigshot, so it never held this step back"
        ))
    };
    if let Some(named) = effect_text(word) {
        let (dialog, text) = named;
        return guard(format!("{bang}{dialog} \"{}\"", literal(token, text)?));
    }
    if lower == "censer" {
        return if bang.is_empty() {
            Ok(Word::Censer)
        } else {
            never()
        };
    }
    if matches!(lower.as_str(), "506" | "celerity") && !bang.is_empty() {
        return Err(format!(
            "`{token}` means \"not in Celerity's last three seconds\" (`bigshot.lic:4347`), which no single guard says"
        ));
    }
    if !bang.is_empty()
        && let Some((_, name)) = COOLDOWN_OF.iter().find(|(w, _)| *w == lower)
    {
        return guard(format!("!cooldown \"{name}\""));
    }
    if let Some((_, text)) = EFFECT_WORDS.iter().find(|(w, _)| *w == lower) {
        return guard(format!("{bang}{text}"));
    }
    if let Some((_, text)) = FLIPPED.iter().find(|(w, _)| *w == lower) {
        return guard(format!("{flip}{text}"));
    }
    if let Some((_, text)) = RENAMED.iter().find(|(w, _)| *w == lower) {
        return if lower == "room" && !bang.is_empty() {
            never()
        } else {
            guard(format!("{bang}{text}"))
        };
    }
    if lower == "wounded" {
        return guard(format!("{bang}thp 25"));
    }
    if let Some(tier) = lower.strip_prefix("ucs") {
        let n = match tier {
            "decent" => 1,
            "good" => 2,
            "excellent" => 3,
            _ => 0,
        };
        if n > 0 {
            return guard(format!("{bang}position {n}"));
        }
    }
    if lower == "once" && !bang.is_empty() {
        return never();
    }
    if SAME.contains(&lower.as_str()) {
        return guard(format!("{bang}{lower}"));
    }
    if let Some((stem, n)) = numbered(&lower) {
        return amount(verb, token, bang, stem, n).unwrap_or_else(never);
    }
    if matches!(lower.as_str(), "essence" | "justice") {
        return later(token);
    }
    if bigshot_knows(word) {
        return never();
    }
    Err(format!(
        "`{token}` is not a guard bigshot knows either; it never held this step back"
    ))
}

/// A word and its number: the amount checks, `thp`, `empowered`, `buff`
/// and `repeatdelay`. `None` for a stem none of them is.
fn amount(verb: &str, token: &str, bang: &str, stem: &str, n: u32) -> Option<Result<Word, String>> {
    let guard = |text: String| Some(Ok(Word::Guard(text)));
    match stem {
        "h" => guard(format!("{bang}health_at_least {n}")),
        "m" => guard(format!("{bang}mana_at_least {n}")),
        "s" => guard(format!("{bang}stamina_at_least {n}")),
        "v" => guard(format!("{bang}spirit_at_least {n}")),
        "e" => guard(format!("{bang}encumbrance_at_least {n}")),
        "k" => guard(format!("{bang}self_kneeling")),
        "mob" | "valid" if bang.is_empty() => guard(format!("targets_at_least {n}")),
        "mob" | "valid" => guard(format!("targets_at_most {n}")),
        "tier" => guard(match (bang.is_empty(), n) {
            (true, 1..=3) => format!("position {n}"),
            (true, _) => format!("position_at_least {n}"),
            (false, 1..=3) => format!("!position_at_least {n}"),
            (false, _) => format!("!position_at_least {}", n.saturating_add(1)),
        }),
        "thp" => guard(format!("{bang}thp {n}")),
        "empowered" => guard(format!("{bang}empowered_below {n}")),
        "repeatdelay" => guard(format!("every {n}")),
        "essence" => Some(later(token)),
        "buff" => Some(buff(verb, token, bang, n)),
        _ => None,
    }
}

/// `buffN` on a verb: `expiring` on the buff the verb grants.
fn buff(verb: &str, token: &str, bang: &str, n: u32) -> Result<Word, String> {
    if !bang.is_empty() {
        return Err(format!("`{token}`: bigshot has no negated buff guard"));
    }
    let first = verb
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match BUFF_OF.iter().find(|(v, _)| *v == first) {
        Some((_, buff)) => Ok(Word::Guard(format!("expiring \"{buff}\" {n}"))),
        None if first == "coupdegrace" => Err(format!(
            "`{token}` on `{verb}`: its buff is a pattern, `Empowered (+N)`, and `expiring` takes a name"
        )),
        None => Err(format!(
            "`{token}` on `{verb}`: bigshot knows no buff for that verb, so the guard never held in bigshot either"
        )),
    }
}

/// A word `plan/33` left for when the model can answer it.
fn later(token: &str) -> Result<Word, String> {
    Err(format!(
        "guard `{token}` needs a fact the model does not capture yet (`plan/33`, later)"
    ))
}

/// `thp20` is `thp` and 20: a stem and its trailing digits.
fn numbered(lower: &str) -> Option<(&str, u32)> {
    let stem = lower.trim_end_matches(|c: char| c.is_ascii_digit());
    if stem.is_empty() || stem.len() == lower.len() {
        return None;
    }
    Some((stem, lower[stem.len()..].parse().ok()?))
}

/// `EB"Rapid Fire"` is the Buffs dialog's word and `Rapid Fire`.
fn effect_text(word: &str) -> Option<(&'static str, &str)> {
    let (kind, rest) = word.split_at_checked(2)?;
    let dialog = match kind.to_ascii_uppercase().as_str() {
        "EB" => "buff",
        "ES" => "spell",
        "EC" => "cooldown",
        "ED" => "debuff",
        _ => return None,
    };
    let text = rest.strip_prefix('"')?.strip_suffix('"')?;
    (!text.is_empty()).then_some((dialog, text))
}

/// A bigshot effect pattern as the name it spells: `\x` is `x`, a bare `.`
/// is itself, and any other pattern character holds the step.
fn literal(token: &str, text: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' => {
                return Err(format!(
                    "`{token}` is a pattern, and Hydra matches an effect by name: write the name as the dialog shows it"
                ));
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

/// Whether bigshot's `:3197` alternation accepts the word: as written, or
/// with its number or quoted text removed. Case does not matter (`/i`).
fn bigshot_knows(word: &str) -> bool {
    let bare = word.split('"').next().unwrap_or(word);
    let known = |w: &str| BIGSHOT_WORDS.iter().any(|k| k.eq_ignore_ascii_case(w));
    if known(bare) {
        return true;
    }
    let stem = bare.trim_end_matches(|c: char| c.is_ascii_digit());
    !stem.is_empty() && known(stem)
}
