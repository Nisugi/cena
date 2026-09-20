//! Attack initiations: *who is attacking whom, with what*.
//!
//! Ports `Parser.parse_attack` (`parser.rb:59-155`) over the `attack` family,
//! plus the four prefix/marker families that modify the attack line after
//! them: ambush and reaction prefixes, the guardian redirect, and the coup de
//! grace kill line.
//!
//! # Inbound attacks carry no target, by design
//!
//! A creature's swing at us names US as its target, so its only creature link
//! is the ATTACKER. Lich's first version fell through to a line scan, found
//! that link, installed the attacker as its own target and applied the damage
//! it dealt us to it -- *"real-feed replay across the log archive: 268
//! self-attributed attacks"* (`attack_defs_spec.rb:180`). So an inbound line
//! resolves to [`TargetKind::None`] and stops; the processor must never let a
//! later link fill that slot (`COMBAT_DEFS_ONBOARDING.md` rule 4).
//!
//! Three things make a line inbound (`parser.rb:75-89`):
//!
//! 1. its target capture says `you`/`your` ([`target::is_self`]);
//! 2. the def is **attackerless** -- the weather or our own gear did it -- and
//!    captured no one else; the attacker is then `environment` or `self`, so
//!    reports can tell them apart (*"frigid wind is environmental, it's not
//!    self inflicted"*, owner 2026-09-07);
//! 3. the def is **room-targeted** -- a creature's howl or trumpet aimed at
//!    everyone, whose SSR is our save.
//!
//! # Foreign attacks stay off our ledger
//!
//! A def that captured a target which is not a creature link -- a player
//! (*"striking Sugiin!"*) or a bare name -- is [`TargetKind::Foreign`]; the
//! event must never later adopt a creature (`parser.rb:107-116`). And an
//! attack whose captured attacker is a player link (negative id) or a bare
//! name is a **nearby player's** attack: `foreign_caster`, emitted for
//! observers but never landed on our rollup (`parser.rb:120-134`, real-feed
//! GSIV-Nisugi 2026-09-06).

use super::defs::{Role, defs};
use super::target::{self, Actor, Pick};
use crate::state::chunks::ChunkLine;

/// Who an attack was aimed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetKind {
    /// A creature, by link.
    Creature(Actor),
    /// Someone who is not a creature link: a player, or a name that did not
    /// resolve. The event is bound to them and must never adopt a creature.
    Foreign(String),
    /// Nobody, or us. An inbound attack, an untargeted `AoE` opener, or a def
    /// with no target at all.
    None,
}

/// One attack initiation line, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackLine {
    /// The def name: `attack`, `fire`, `tangleweed`, `divine_wrath`, ...
    pub name: String,
    /// The def group it came from (`priority`, `basic`, `spell`, ...), which
    /// is the assembly order made legible.
    pub group: String,
    /// Who was attacked.
    pub target: TargetKind,
    /// Aimed at us.
    pub inbound: bool,
    /// Who attacked, when the line says (third-person defs, or the synthetic
    /// `environment` / `self` of an attackerless tick).
    pub attacker: Option<Actor>,
    /// A nearby player's attack, not ours.
    pub foreign_caster: bool,
    /// The weapon named in prose, when the def captures one.
    pub weapon: Option<String>,
    /// An aimed shot (`take aim and`, `make a precise attempt`), which rolls
    /// differently.
    pub aimed: bool,
}

fn has_group(def: &super::defs::Def, name: &str) -> bool {
    def.regex
        .as_ref()
        .is_some_and(|re| re.capture_names().any(|n| n == Some(name)))
}

/// The attacker an `(?<attacker>)` capture names.
///
/// `parser.rb:196-214`: the **last** link in the capture, because a flavour
/// prefix can carry the attacker's own pronoun link first; a bare name when
/// there is no link at all.
fn attacker_from(line: &ChunkLine, caps: &regex::Captures<'_>) -> Option<Actor> {
    let m = caps.name("attacker")?;
    if m.as_str().trim().is_empty() {
        return None;
    }
    Some(
        target::link_in(line, m.range(), Pick::Last).unwrap_or_else(|| Actor::unlinked(m.as_str())),
    )
}

impl AttackLine {
    /// Classify one line as an attack initiation.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        let table = defs();
        let (def, caps) = table.first_match("attack", &text)?;
        let name = def.name.clone();
        let group = def.extra("group").unwrap_or("").to_owned();

        let target_cap = caps.name("target");
        let is_attackerless = table
            .attack_class(Role::Environmental)
            .chain(table.attack_class(Role::SelfInflicted))
            .any(|n| n == name);
        // Attackerless only when this particular pattern named nobody else --
        // a nearby player taking the same tick is a foreign target instead.
        let attackerless = is_attackerless && target_cap.is_none();

        let self_target = if has_group(def, "target") {
            target_cap.is_some_and(|m| target::is_self(m.as_str()))
        } else {
            has_group(def, "attacker") && target::pattern_addresses_self(&def.pattern)
        };
        let room_targeted = table.attack_class(Role::RoomTargeted).any(|n| n == name);

        if self_target || attackerless || room_targeted {
            let mut attacker = attacker_from(line, &caps);
            if attackerless && attacker.is_none() {
                let environmental = table.attack_class(Role::Environmental).any(|n| n == name);
                attacker = Some(Actor::unlinked(if environmental {
                    "environment"
                } else {
                    "self"
                }));
            }
            return Some(Self {
                name,
                group,
                target: TargetKind::None,
                inbound: true,
                attacker,
                foreign_caster: false,
                weapon: None,
                aimed: false,
            });
        }

        let attacker = attacker_from(line, &caps);
        // A def with a target capture resolves ONLY through it -- when the
        // capture is not a creature link, scanning the line would find some
        // other creature, typically the attacker (`parser.rb:92-99`). A def
        // with none takes the line's bolded link.
        let mut target = if has_group(def, "target") {
            target_cap
                .and_then(|m| target::link_in(line, m.range(), Pick::First))
                .filter(|a| a.id.is_some_and(|id| id >= 0))
        } else {
            target::bolded_link(line)
        };
        // Nothing can attack itself: an untargeted AoE's line scan returns the
        // only link present, which is the attacker.
        let struck_itself = matches!(
            (&target, &attacker),
            (Some(t), Some(a)) if t.id.is_some() && t.id == a.id
        );
        if struck_itself {
            target = None;
        }
        let foreign_target =
            target.is_none() && target_cap.is_some_and(|m| !m.as_str().trim().is_empty());
        let target = match target {
            Some(a) => TargetKind::Creature(a),
            None if foreign_target => TargetKind::Foreign(
                target_cap.map_or_else(String::new, |m| m.as_str().trim().to_owned()),
            ),
            None => TargetKind::None,
        };
        let foreign_caster = attacker
            .as_ref()
            .is_some_and(|a| a.is_player() || a.id.is_none());

        Some(Self {
            name,
            group,
            target,
            inbound: false,
            attacker,
            foreign_caster,
            weapon: caps.name("weapon").map(|m| m.as_str().trim().to_owned()),
            aimed: caps.name("aimed").is_some(),
        })
    }

    /// Is this a creature's attack on us?
    #[must_use]
    pub const fn is_inbound(&self) -> bool {
        self.inbound
    }
}

/// The weapon a 2p swing or fire line names, in plain text.
///
/// The generic swing defs capture the target and not the weapon; this is
/// Lich's `parse_swing_weapon` (`parser.rb:246-256`), which the processor
/// uses to claim a weapon's pre-flares when its swing arrives.
#[must_use]
pub fn swing_weapon(line: &ChunkLine) -> Option<String> {
    defs().swing_weapon(&line.text())
}

/// Is this line an environmental or self-inflicted tick?
///
/// `attacks.rb:615-619`: the tracker's chunk gate only forwards chunks with a
/// bolded creature link, and these name no creature -- *"real-feed
/// 2026-09-07: zero `frigid_wind` rows against 10 log ticks"*. Defined over the
/// `ENVIRONMENTAL_ATTACKS` def group, which the extractor records as
/// `group=environmental`.
#[must_use]
pub fn attackerless_line(line: &ChunkLine) -> bool {
    let text = line.text();
    defs()
        .family("attack")
        .iter()
        .filter(|d| d.extra("group") == Some("environmental"))
        .filter_map(|d| d.regex.as_ref())
        .any(|re| re.is_match(&text))
}

/// The maneuver an ambush or reaction prefix names.
///
/// `attacks.rb:666`: waylay weights damage where the bare hiding forms weight
/// crits, so the kind is recorded beside the flag; a reverse strike is a
/// parry reaction, engaged and visible, so it is never an ambush.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AmbushKind {
    /// From hiding, crit-weighted.
    Ambush,
    /// From hiding, damage-weighted (rogue CMAN).
    Waylay,
    /// After a parry (Two-Handed Weapons technique). Not from hiding.
    ReverseStrike,
}

impl AmbushKind {
    /// Every kind.
    pub const ALL: [Self; 3] = [Self::Ambush, Self::Waylay, Self::ReverseStrike];

    /// The TSV spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ambush => "ambush",
            Self::Waylay => "waylay",
            Self::ReverseStrike => "reverse_strike",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == text)
    }
}

/// A prefix line that modifies the attack after it: `You leap from hiding to
/// attack!`, `Spotting an opening ... you quickly reverse the direction`.
///
/// **Not an attack.** Treated as a def these opened *"a second, fact-less
/// event per ambush (35,549 occurrences)"* (`attacks.rb:643-664`); as a
/// prefix they arm a marker the next attack claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbushPrefix {
    /// The attacker, for the third-person form.
    pub attacker: Option<String>,
    /// Which maneuver.
    pub kind: AmbushKind,
    /// Made from hiding (sets `ambush` on the attack). False for a reaction.
    pub hidden: bool,
}

impl AmbushPrefix {
    /// Classify one line as an ambush or reaction prefix.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        let table = defs();
        if let Some((def, caps)) = table.first_match("ambush_prefix", &text) {
            return Some(Self {
                attacker: caps.name("attacker").map(|m| m.as_str().to_owned()),
                kind: AmbushKind::parse(&def.name)?,
                hidden: true,
            });
        }
        let (def, _) = table.first_match("reaction_prefix", &text)?;
        Some(Self {
            attacker: None,
            kind: AmbushKind::parse(&def.name)?,
            hidden: false,
        })
    }
}

/// A guardian stepping between us and our target, so the attack that follows
/// resolves in full against the guardian instead.
///
/// `attacks.rb:716-739`: *"Corpus (130 lines, 2026-09-07): the follower is
/// ALWAYS an attack line."* Not an outcome -- nothing was nullified -- but a
/// marker the next attack claims, so per-creature stats know whose DS was
/// rolled against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectPrefix {
    /// Who intercepted, as captured.
    pub interceptor: String,
    /// The intended victim's noun.
    pub intended: String,
}

impl RedirectPrefix {
    /// Classify one line as a guardian redirect.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        let (_, caps) = defs().first_match("redirect_prefix", &text)?;
        Some(Self {
            interceptor: caps.name("interceptor")?.as_str().to_owned(),
            intended: caps.name("intended")?.as_str().to_owned(),
        })
    }
}

/// The struck location when the line is a coup de grace kill.
///
/// `attacks.rb:580-592`: the coup prints no damage number; its success line
/// IS the killing blow, recorded as a zero-damage fatal hit (owner ruling
/// 2026-09-07).
#[must_use]
pub fn coup_kill_location(line: &ChunkLine) -> Option<String> {
    let text = line.text();
    defs()
        .first_match("coup_kill", &text)
        .map(|(def, _)| def.name.clone())
}
