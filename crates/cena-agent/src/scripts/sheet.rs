//! The character's sheet, in a runner's local copy (`plan/46` §11 step 7):
//! what the game has said of the character in `info`, `skills`,
//! `experience`, `society`, `resource`, `wealth` and the PSM lists, as
//! Hydra's model read it (`cena_session::GameState::character`). Lich's
//! scripts read the same through `Infomon`, the store Lich fills from that
//! text; the Ruby bridge answers it from this.
//!
//! **Unknown stays unknown**: a table never read is absent, a value never
//! stated is `null`, never zero. Lich cannot tell those apart; a bridge can.

use std::collections::BTreeMap;

use cena_session::{GameState, PsmCategory, SkillKind, Warcry};
use serde::Serialize;

/// What the game has said of the character.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Sheet {
    /// Race, profession, gender and age as `info` says them, and the
    /// account's tier as `profile` says it.
    pub identity: Identity,
    /// The stats `info` has shown, by name (`strength`).
    pub stats: BTreeMap<&'static str, Stat>,
    /// Ranks and bonus by skill, keyed as Lich keys them
    /// (`two_weapon_combat`); empty until `skills` has been read.
    pub skills: BTreeMap<String, Skill>,
    /// Ranks by spell circle, as the `skills` table names it
    /// (`Minor Elemental`).
    pub circles: BTreeMap<String, u16>,
    /// Ranks by PSM, by category (`armor`, `cman`, `feat`, `shield`,
    /// `weapon`) and mnemonic; a category whose list was never read is
    /// absent, and a mnemonic not in a list read is not known.
    pub psms: BTreeMap<&'static str, BTreeMap<String, u16>>,
    /// The warcries known, by their short names (`bellow`).
    pub warcries: Vec<&'static str>,
    /// The society; `null` until stated.
    pub society: Option<Society>,
    /// The citizenship; `null` until stated.
    pub citizenship: Option<Citizenship>,
    /// The profession's resource and the other earned amounts.
    pub resources: Resources,
    /// Every balance stated, by name (`silver`, `bloodscrip`...).
    pub currency: BTreeMap<&'static str, i64>,
    /// Level and experience.
    pub experience: Experience,
}

/// Who the character is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Identity {
    /// `Half-Elf`.
    pub race: Option<String>,
    /// `Ranger`.
    pub profession: Option<String>,
    /// `Male`.
    pub gender: Option<String>,
    /// In years.
    pub age: Option<u32>,
    /// `Free`, `Normal`, `Premium` or `Platinum`.
    pub account: Option<&'static str>,
}

/// One stat, by each of `info`'s columns; each `null` until shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Stat {
    /// Before ascension and enhancives: `info full` only.
    pub base: Option<Value>,
    /// After ascension: `info`'s first column.
    pub ascended: Option<Value>,
    /// After enhancives: `info`'s last column.
    pub enhanced: Option<Value>,
}

/// A stat's value and its bonus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Value {
    /// The value.
    pub value: i16,
    /// Its bonus.
    pub bonus: i16,
}

/// One skill.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Skill {
    /// Ranks.
    pub ranks: Option<u16>,
    /// The bonus.
    pub bonus: Option<u16>,
}

/// A society and the rank in it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Society {
    /// `Order of Voln`; `null` when stated none.
    pub name: Option<&'static str>,
    /// The rank, or step; a master's is the society's highest.
    pub rank: Option<u8>,
}

/// A citizenship.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Citizenship {
    /// The town; `null` when stated none.
    pub town: Option<String>,
}

/// What the character has earned beyond experience.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Resources {
    /// The profession's resource (`Essence`, `Grit`...), from `resource`.
    pub kind: Option<&'static str>,
    /// Earned this week.
    pub weekly: Option<u32>,
    /// Earned in all.
    pub total: Option<u32>,
    /// Suffused now.
    pub suffused: Option<u32>,
    /// Voln's favor.
    pub voln_favor: Option<i64>,
    /// Covert Arts charges, out of 200.
    pub covert_arts_charges: Option<i32>,
    /// Shadow essence, 0 to 5.
    pub shadow_essence: Option<u8>,
}

/// Level and experience, from the experience window and the `experience`
/// command.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Experience {
    /// The level, from the window's `Level 100`.
    pub level: Option<u32>,
    /// The experience total.
    pub experience: Option<u64>,
    /// The next-level bar's text, as the game words it.
    pub next_level: Option<String>,
    /// The next-level bar, in percent.
    pub next_level_percent: Option<u32>,
    /// Experience to the next level, or training point.
    pub until_next: Option<u32>,
    /// Field experience held.
    pub field_experience: Option<u32>,
    /// Field experience that can be held.
    pub field_experience_max: Option<u32>,
    /// Ascension experience.
    pub ascension_experience: Option<u64>,
    /// Total experience.
    pub total_experience: Option<u64>,
    /// Long-term experience.
    pub long_term_experience: Option<u32>,
    /// Deeds.
    pub deeds: Option<u32>,
    /// Fame.
    pub fame: Option<i64>,
    /// Death's sting, as the game words it (`None`, `Light`...).
    pub deaths_sting: Option<&'static str>,
    /// Recent deaths.
    pub recent_deaths: Option<u32>,
    /// The gift of Lumnis's number, while it is active.
    pub lumnis: Option<u32>,
    /// `rpa`, while active, as the game sends it.
    pub rpa: Option<String>,
    /// An orb of Fash'lo'nae: 1 redeemed, 2 active.
    pub fashlonae: Option<u8>,
}

/// The sheet of `state`.
#[must_use]
#[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "the model's AccountType and ResourceType are not re-exported, so their paths cannot be named here"
)]
pub fn sheet(state: &GameState) -> Sheet {
    let character = &state.character;
    let identity = &character.identity;
    let standing = &character.standing;
    Sheet {
        identity: Identity {
            race: identity.race.clone(),
            profession: identity.profession.clone(),
            gender: identity.gender.clone(),
            age: identity.age,
            account: identity.account.map(|account| account.as_str()),
        },
        stats: character
            .stats
            .iter()
            .map(|(kind, stat)| {
                let stat = Stat {
                    base: stat.normal.map(|v| Value {
                        value: v.value,
                        bonus: v.bonus,
                    }),
                    ascended: stat.ascended.map(|v| Value {
                        value: v.value,
                        bonus: v.bonus,
                    }),
                    enhanced: stat.enhanced.map(|v| Value {
                        value: v.value,
                        bonus: v.bonus,
                    }),
                };
                (kind.as_str(), stat)
            })
            .collect(),
        skills: SkillKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let skill = character.skills.get(kind)?;
                Some((
                    kind.key(),
                    Skill {
                        ranks: skill.ranks,
                        bonus: skill.bonus,
                    },
                ))
            })
            .collect(),
        circles: character
            .skills
            .circles()
            .map(|(name, ranks)| (name.to_owned(), ranks))
            .collect(),
        psms: PsmCategory::ALL
            .into_iter()
            .filter(|category| character.psms.has_table(*category))
            .map(|category| {
                let ranks = character
                    .psms
                    .mnemonics(category)
                    .map(|(mnemonic, ranks)| (mnemonic.to_owned(), ranks.ranks))
                    .collect();
                (category.as_str(), ranks)
            })
            .collect(),
        warcries: standing
            .warcries
            .iter()
            .map(|warcry: &Warcry| warcry.short_name())
            .collect(),
        society: standing.society.map(|society| Society {
            name: society.map(cena_session::Society::as_str),
            rank: standing.society_rank,
        }),
        citizenship: standing
            .citizenship
            .clone()
            .map(|town| Citizenship { town }),
        resources: Resources {
            kind: standing.resource_type.map(|kind| kind.as_str()),
            weekly: standing.resources.map(|amounts| amounts.weekly),
            total: standing.resources.map(|amounts| amounts.total),
            suffused: standing.suffused,
            voln_favor: character.currency.voln_favor,
            covert_arts_charges: standing.covert_arts_charges,
            shadow_essence: standing.shadow_essence,
        },
        currency: currency(state),
        experience: experience(state),
    }
}

/// Every balance stated, by Lich's name for it.
fn currency(state: &GameState) -> BTreeMap<&'static str, i64> {
    let currency = &state.character.currency;
    [
        ("silver", currency.silver),
        ("silver_container", currency.silver_container),
        ("silver_total", currency.silver_total),
        ("notes", currency.notes),
        ("tickets", currency.tickets),
        ("gold", currency.gold),
        ("blackscrip", currency.blackscrip),
        ("bloodscrip", currency.bloodscrip),
        ("ethereal_scrip", currency.ethereal_scrip),
        ("soul_shards", currency.soul_shards),
        ("raikhen", currency.raikhen),
        ("aevit", currency.aevit),
        (
            "gigas_artifact_fragments",
            currency.gigas_artifact_fragments,
        ),
        ("redsteel_marks", currency.redsteel_marks),
        ("dust", currency.dust),
    ]
    .into_iter()
    .filter_map(|(name, amount)| Some((name, i64::try_from(amount?).unwrap_or(i64::MAX))))
    .collect()
}

#[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "the model's DeathsSting is not re-exported, so its path cannot be named here"
)]
fn experience(state: &GameState) -> Experience {
    let exp = &state.character.experience;
    Experience {
        level: exp.level.as_deref().and_then(|level| {
            level
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .parse()
                .ok()
        }),
        experience: exp.experience,
        next_level: exp.next_level.clone(),
        next_level_percent: exp.next_level_percent,
        until_next: exp.mind_bar.until_next,
        field_experience: exp.field_experience,
        field_experience_max: exp.field_experience_max,
        ascension_experience: exp.ascension_experience,
        total_experience: exp.total_experience,
        long_term_experience: exp.long_term_experience,
        deeds: exp.deeds,
        fame: exp.fame,
        deaths_sting: exp.deaths_sting.map(|sting| sting.as_str()),
        recent_deaths: exp.recent_deaths,
        lumnis: exp.mind_bar.lumnis,
        rpa: exp.mind_bar.rpa.clone(),
        fashlonae: exp.mind_bar.fashlonae,
    }
}
