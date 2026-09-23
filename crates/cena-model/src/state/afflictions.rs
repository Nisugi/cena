//! The six statuses that arrive only as **text**, never as an indicator.
//!
//! # Why this exists, and how it was found
//!
//! > **AUTHOR, 2026-09-21:** *"This is also why I expected you to sleuth
//! > through the lich parser, xmlparser, and all of it's related things to see
//! > what else is missing rather than just looking at what my character only
//! > sees"*.
//!
//! Censusing one live log finds what that character did. The author's own
//! character earns **ascension** experience, so its training points never
//! change and the game never resends them -- which I had misread as "the login
//! burst sends them once". A log is evidence of one character's session; the
//! parser is evidence of what the game can send at all.
//!
//! Censusing `infomon/parser.rb` and `infomon/xmlparser.rb` for every
//! `Infomon.set` key found **38 distinct keys**, all modelled here except
//! these six:
//!
//! ```text
//! status.bound  status.calmed  status.cutthroat
//! status.silenced  status.sleeping  status.thorned
//! ```
//!
//! `CLAUDE.md` already records the distinction that makes this a real gap:
//! *"the indicator-vs-text-derived split in `lib/gemstone/infomon/status.rb`"*.
//! [`StatusInfo`](crate::StatusInfo) holds the 11 `ICONMAP` indicators plus
//! `poisoned`/`diseased`; **no indicator exists for any of these six**, so a
//! client that reads only `<indicator>` cannot know a character is silenced,
//! bound, asleep or bleeding out from a thorn.
//!
//! # Every pattern is Lich's, and the anchors are load-bearing
//!
//! Ported from `infomon/parser.rb:87-102`. The patterns are anchored to the
//! start of a line there, and that is not cosmetic: a player can type *"A calm
//! washes over you."* into a channel. One exception is Lich's own and is
//! marked in its source -- `CutthroatActiveMid` is *"mid-line: cannot be part
//! of the anchored fast path"*, because the wire sends
//! `<creature> slices deep into your vocal cords!` with the attacker first.
//!
//! # Thorn poison has five ways to say yes
//!
//! `ThornPoisonStart`, five `Progression` lines and four `Deprogression` lines
//! all mean *still poisoned*; only `ThornPoisonEnd` clears it. Lich groups them
//! into one `true` arm with a `# TODO: refactor / streamline?` above it. Ported
//! as the same grouping, because the grouping is the fact: a deprogression line
//! is the poison **weakening, not gone**, and treating it as an end is the bug
//! that TODO is inviting someone to introduce.

/// A status the game reports only in prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affliction {
    /// Immobilised: Bind (spell 210) and the moonbeam traps.
    Bound,
    /// Calmed, and so unable to attack.
    Calmed,
    /// Cutthroat: vocal cords cut, so no spellcasting.
    Cutthroat,
    /// Silenced: a pall of silence.
    Silenced,
    /// Asleep.
    Sleeping,
    /// Thorn poison, from the vine traps.
    Thorned,
}

impl Affliction {
    /// Every one.
    pub const ALL: [Self; 6] = [
        Self::Bound,
        Self::Calmed,
        Self::Cutthroat,
        Self::Silenced,
        Self::Sleeping,
        Self::Thorned,
    ];

    /// The id this is stored under in [`StatusInfo`](crate::StatusInfo).
    ///
    /// Lich's own key without its `status.` prefix, so
    /// `status.silenced` is `silenced` -- the same shape `StatusInfo` already
    /// normalises `IconSTUNNED` to.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::Calmed => "calmed",
            Self::Cutthroat => "cutthroat",
            Self::Silenced => "silenced",
            Self::Sleeping => "sleeping",
            Self::Thorned => "thorned",
        }
    }
}

/// What one line says about an affliction, if anything.
///
/// `(affliction, active)`: `true` began or continues, `false` ended.
#[must_use]
pub fn classify(line: &str) -> Option<(Affliction, bool)> {
    // Mid-line FIRST, because Lich does not anchor these at the start
    // (`parser.rb:95`, and one alternative of `:101` below). Checked before
    // the anchored set so a line that is both cannot be taken for the other.
    // This said "only this one", which missed the thorn line.
    if line.ends_with("slices deep into your vocal cords!") {
        return Some((Affliction::Cutthroat, true));
    }
    // The other unanchored one: `ThornPoisonDeprogression`'s second
    // alternative has `$` and no `^` (`parser.rb:101`), unlike its three
    // siblings. A deprogression line is the poison weakening, so `true`.
    if line.trim_end().ends_with(THORN_EASING) {
        return Some((Affliction::Thorned, true));
    }
    let text = line.trim_start();
    // `ThornPoisonStart` (`parser.rb:99`) has a wildcard in the MIDDLE --
    // `^One of the vines surrounding .*? lashes out at you, driving a thorn`
    // -- so it is a prefix AND a suffix, not a prefix alone. The prefix alone
    // matched any line that began by describing someone else's vines.
    if let Some(rest) = text.strip_prefix(THORN_START.0)
        && rest.trim_end().ends_with(THORN_START.1)
    {
        return Some((Affliction::Thorned, true));
    }
    for (affliction, active, patterns) in TABLE {
        if patterns.iter().any(|p| text.starts_with(*p)) {
            return Some((*affliction, *active));
        }
    }
    None
}

/// `ThornPoisonStart` (`parser.rb:99`): the text either side of its `.*?`.
///
/// Two spaces after `skin!`, as the regex writes them.
const THORN_START: (&str, &str) = (
    "One of the vines surrounding ",
    " lashes out at you, driving a thorn into your skin!  You feel poison coursing through your veins.",
);

/// `ThornPoisonDeprogression`'s unanchored alternative (`parser.rb:101`).
const THORN_EASING: &str = "Although you can't seem to move as quickly as you usually can, you're feeling better than you were just moments ago.";

/// The prefixes, from `infomon/parser.rb:87-102`.
///
/// Prefixes rather than whole-line equality: Lich anchors the start and most of
/// its patterns end with `$`, but several do not (`^You are awoken`,
/// `^You awake`), and a prefix match satisfies both without a regex engine in
/// this crate.
type Table = &'static [(Affliction, bool, &'static [&'static str])];

const TABLE: Table = &[
    // --- sleep ------------------------------------------------------------
    (
        Affliction::Sleeping,
        true,
        &[
            "Your mind goes completely blank.",
            "You close your eyes and slowly drift off to sleep.",
            "You slump to the ground and immediately fall asleep.",
            "That is impossible to do while unconscious",
        ],
    ),
    (
        Affliction::Sleeping,
        false,
        &[
            "Your thoughts slowly come back to you as you find yourself lying on the ground.",
            "You wake up from your slumber.",
            "You are awoken",
            "You awake",
            "You slowly come back to alertness and realize you must have been sleeping.",
        ],
    ),
    // --- bind -------------------------------------------------------------
    (
        Affliction::Bound,
        true,
        &[
            "An unseen force envelops you, restricting",
            "An unseen force entangles you, restricting",
            "You are caught fast, the light of",
        ],
    ),
    (
        Affliction::Bound,
        false,
        &[
            "The restricting force that envelops you dissolves away.",
            "You shake off the immobilization that was restricting your movements!",
            "The restricting force enveloping you fades away.",
        ],
    ),
    // --- silence ----------------------------------------------------------
    (
        Affliction::Silenced,
        true,
        &[
            "A pall of silence settles over you.",
            "The pall of silence settles more heavily over you.",
        ],
    ),
    (
        Affliction::Silenced,
        false,
        &["The pall of silence leaves you."],
    ),
    // --- calm -------------------------------------------------------------
    (Affliction::Calmed, true, &["A calm washes over you."]),
    (
        Affliction::Calmed,
        false,
        &["You are enraged by ", "The feeling of calm leaves you."],
    ),
    // --- cutthroat --------------------------------------------------------
    (
        Affliction::Cutthroat,
        true,
        &["All you manage to do is cough up some blood."],
    ),
    (
        Affliction::Cutthroat,
        false,
        &[
            "The horrible pain in your vocal cords subsides as you spit out the last of the blood clogging your throat.",
            "That tingles, but there are no head injuries to repair.",
        ],
    ),
    // --- thorn poison -----------------------------------------------------
    //
    // Start, progression and DEPROGRESSION are all `true`: a deprogression
    // line is the poison weakening, not gone. Only `ThornPoisonEnd` clears it.
    (
        Affliction::Thorned,
        true,
        &[
            "You begin to feel a strange fatigue, spreading throughout your body.",
            "The strange lassitude is growing worse,",
            "You find yourself gradually slowing down, your muscles trembling with fatigue.",
            "It's getting increasingly difficult to move.",
            "No longer able to fight this odd paralysis, you collapse to the ground,",
            "With a shaky gasp and trembling muscles, you regain at least some small ability to move,",
            "Fine coordination is difficult, but at least you can move at something close to your normal speed again.",
            "While you're still a bit shaky, your muscles are responding better than they were.",
        ],
    ),
    (
        Affliction::Thorned,
        false,
        &[
            "Your body begins to respond normally again.",
            "Your skin takes on a more pinkish tint.",
        ],
    ),
];
