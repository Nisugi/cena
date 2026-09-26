//! Experience, injuries, stance and encumbrance: **M2 step 3**, `plan/18` §2b.
//!
//! Split out of `state.rs` under Rule 4.1 (`plan/05:352-353`).
//!
//! # All four are wire-driven, and that moved the milestone boundary
//!
//! `plan/18` §1 records the finding: Lich scrapes experience and injuries out of
//! prose with regexes *and* reads the dialog (`common/xmlparser.rb:725-735`), so
//! reading the directory names would have put both in the deferred,
//! text-scraped half. Reading the wire puts them here, with no regex at all.
//!
//! MEASURED over 24 files across 6 characters, `<dialogData id=>` occurrences:
//! `combat` 5,869, `minivitals` 5,235, `expr` 2,439, `Buffs` 2,426,
//! `mapViewMain` 2,229, `injuries` 1,892, `Active Spells` 1,660, `encum` 941,
//! `Debuffs` 840, `Cooldowns` 840, `stance` 377, `quick` 367. The effect dialogs
//! are already handled by `plan/17`; `combat` and `quick*` are UI affordances,
//! not character state (`plan/18` §2b defers them).
//!
//! # The shapes, verbatim from `GSIV-Zoleta`
//!
//! ```text
//! expr:   <label id='yourLvl' value='Level 100'/>
//!         <progressBar id='mindState' value='0' text='clear as a bell'/>
//!         <progressBar id='nextLvlPB' value='100' text='11999265 experience'/>
//! stance: <progressBar id='pbarStance' value='100' text='defensive (100%)'/>
//! encum:  <progressBar id='encumlevel' value='0' text='None'/>
//!         <label id='encumblurb' value='You are not encumbered enough to notice.'/>
//! ```
//!
//! # Injuries are `<image>`, not `<progressBar>` -- and `plan/18` guessed wrong
//!
//! §2b said *"per-body-part injury and scar, 16 parts, named on the wire"* and
//! assumed the shape that carries everything else. It does not. MEASURED: the
//! `injuries` dialog carries only a `health2` bar, and the body parts arrive as
//!
//! ```text
//! <image id="leftArm" name="Injury1" height="0" width="0"/>
//! <image id="neck" name="neck" height="0" width="0"/>
//! ```
//!
//! **`name` equal to `id` means unhurt**; `Injury1`..`Injury3` and
//! `Scar1`..`Scar3` are the severities. VERIFIED against Lich, which reads it
//! identically (`lib/common/xmlparser.rb:809-822`) and supplies two facts the
//! wire alone does not give:
//!
//! * **a scar CLEARS the wound.** They are separate tracks, and a scar is what a
//!   healed wound leaves behind, so reporting both would double-count.
//! * **`nsys`** -- the nervous system -- is one of the parts, not a separate
//!   mechanism.

pub mod blocks;
pub mod body;
pub mod currency;
pub mod enhancive;
pub mod experience_report;
pub mod injured;
pub mod profile;
pub mod psm;
pub mod skills;
pub mod snapshot;
pub mod spellsong;
pub mod stance;
pub mod standing;
pub mod stats;
pub mod training;
pub mod vocabulary;

use std::collections::BTreeMap;

use stance::Stance;

/// What the `expr` dialog says about advancement.
///
/// Every field is `Option` because the dialog is **partial**: a burst may carry
/// the level and not the mind state. `None` is "not yet told", which `plan/12`
/// §5.2 makes a first-class value rather than a default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Experience {
    /// `<label id='yourLvl' value='Level 100'>`, **verbatim**.
    ///
    /// The whole label, not a parsed number. The wire says `Level 100`; turning
    /// that into `100` is a guess about a format the game may change, and every
    /// consumer that wants to display it wants the string back.
    pub level: Option<String>,
    /// Physical training points, from `<label id='PTPs'>`. See
    /// [`training`] for how these were found and why Lich has them
    /// nowhere: the wire is the only authority.
    pub physical_training: Option<u32>,
    /// Mental training points, from `<label id='MTPs'>`.
    pub mental_training: Option<u32>,
    /// Physical points already converted to mental, from `<label id='p2m'>`.
    /// Conversion is capped, so this is a separate question from the total.
    pub physical_converted: Option<u32>,
    /// Mental points converted to physical, from `<label id='m2p'>`.
    pub mental_converted: Option<u32>,
    /// `<progressBar id='mindState' text='clear as a bell'>`.
    pub mind_state: Option<String>,
    /// `mindState`'s `value=`, 0-100.
    pub mind_percent: Option<u32>,
    /// `<progressBar id='nextLvlPB' text='11999265 experience'>`.
    pub next_level: Option<String>,
    /// `nextLvlPB`'s `value=`, 0-100.
    pub next_level_percent: Option<u32>,
    /// `Fame: 1,453,539,090`, from the `experience` command.
    ///
    /// Signed: Lich captures `-?[\d,]+` (`parser.rb:16`), so the wire can send
    /// a negative. `i64` because the author's own character reads 1,453,539,090
    /// -- past `i32` -- and fame only grows.
    pub fame: Option<i64>,
    /// `Field Exp: 1,234/1,403` -- earned and the cap.
    ///
    /// Two fields from one line, and BOTH are read. Lich reads both
    /// (`parser.rb:278`) but `;infomon show` displays only the max, which is
    /// how a reader of that output would conclude the current value is not
    /// tracked.
    pub field_experience: Option<u32>,
    /// `Field Exp: 1,234/1,403` -- the cap.
    pub field_experience_max: Option<u32>,
    /// `Ascension Exp: 24,865,590`.
    pub ascension_experience: Option<u64>,
    /// `Total Exp: 68,770,511`.
    pub total_experience: Option<u64>,
    /// `Long-Term Exp: 493`.
    pub long_term_experience: Option<u32>,
    /// `Deeds: 11`.
    pub deeds: Option<u32>,
    /// `Death's Sting: None`.
    pub deaths_sting: Option<vocabulary::DeathsSting>,
    /// `Experience: 43,904,921` -- the experience total the line leads with.
    ///
    /// **Lich reads this line and throws this number away** (`parser.rb:17`:
    /// `Experience: [\d,]+` is matched and not captured, while the field-exp
    /// pair beside it is). Rule 2.2 says nothing the wire states is dropped
    /// silently, and `;infomon show` confirms the loss: it has
    /// `experience.total_experience` and no plain experience key.
    pub experience: Option<u64>,
    /// `Recent Deaths: 0`.
    ///
    /// Also matched-and-discarded by Lich (`parser.rb:18`).
    pub recent_deaths: Option<u32>,
    /// The gift of Lumnis, as far as it can be known. See [`Gift`].
    pub gift: Gift,
}

/// The gift of Lumnis: 360 minutes of doubled experience, once a week.
///
/// # What Lich does, and why this does not copy it
///
/// Lich counts **pulses** -- one per `nextLvlPB` text change -- and reports
/// `(360 - count)` minutes remaining (`gemstone/gift.rb:25`). MEASURED, that
/// counter has exactly one live caller:
///
/// ```text
/// $ grep -rn "Gift\." reference/lich-5/ --include=*.rb \
///     | grep -v spec/ | grep -v lib/gemstone/gift.rb
/// lib/common/xmlparser.rb:751:  Gift.pulse unless @next_level_text == attributes['text']
/// ```
///
/// `started`, `ended`, `restarts_on` and the serialization are called **only
/// from Lich's own specs**. Nothing in production resets the counter or saves
/// it, so it begins at zero on every launch and `remaining` is only right for
/// someone who started Lich at the instant their gift began.
///
/// # So this counts pulses and does not claim to know the rest
///
/// [`Self::pulses`] is the fact: the experience bar changed this many times
/// since the session began. Turning that into "minutes remaining" needs a
/// start time nothing on the wire has yet been shown to send, so there is no
/// `remaining()` here -- an `Option` that is always `None` would be worse than
/// the absence, and a number derived from an unreset counter would be worse
/// still.
///
/// The upgrade trigger is explicit: **if the wire is found to state when a gift
/// starts or ends**, this gains a start time and the arithmetic
/// (`360` minutes, restarting `594_000` seconds later) is already written down
/// above to port.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gift {
    /// How many times the experience bar's text has changed this session.
    ///
    /// Lich's pulse count, under a name that says what it measures rather than
    /// what it is used for.
    pub pulses: u32,
}

/// How badly one body part is hurt.
///
/// Wound and scar are **separate tracks**, per Lich: a scar is what a healed
/// wound leaves, so a part can be scarred and unwounded at once. Rank 0 is
/// healthy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Injury {
    /// `Injury1`..`Injury3` -> 1..3. Zero when unhurt.
    pub wound: u8,
    /// `Scar1`..`Scar3` -> 1..3. Zero when unscarred.
    pub scar: u8,
}

impl Injury {
    /// Whether this part is hurt or scarred at all.
    #[must_use]
    pub const fn is_hurt(self) -> bool {
        self.wound > 0 || self.scar > 0
    }
}

/// What the game says about the character, beyond vitals and the room.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Character {
    /// The `expr` dialog.
    pub experience: Experience,
    /// Injuries by body part id (`head`, `leftArm`, `nsys`, ...).
    ///
    /// **Only hurt parts are present.** A healthy part is removed rather than
    /// stored as zero, so `is_empty()` answers "is this character unhurt"
    /// without a scan, and iterating gives exactly what a display should show.
    ///
    /// `BTreeMap` for criterion 7: a replay iterating a `HashMap` would be
    /// non-deterministic.
    pub injuries: BTreeMap<String, Injury>,
    /// Parts explicitly observed in this connection, for conservative recovery.
    /// Not persisted: an empty injury map alone cannot prove a healthy body.
    pub observed_body_parts: u16,
    /// `<progressBar id='pbarStance' text='defensive (100%)'>`.
    pub stance: Option<String>,
    /// `pbarStance`'s `value=`: percent of stance contributing to defense.
    pub stance_percent: Option<u32>,
    /// `<progressBar id='encumlevel' text='None'>`.
    pub encumbrance: Option<String>,
    /// `encumlevel`'s `value=`, 0-100.
    pub encumbrance_percent: Option<u32>,
    /// `<label id='encumblurb' value='You are not encumbered enough to notice.'>`.
    pub encumbrance_detail: Option<String>,
    /// The ten statistics, as far as `info` has taught them.
    ///
    /// `BTreeMap` rather than ten named fields: criterion 7 needs a
    /// deterministic iteration order, and a stat the wire has never mentioned
    /// is absent rather than zero -- the same "unknown is not a default" rule
    /// `roundtime_ends` follows (`plan/12` §5.2).
    pub stats: BTreeMap<stats::StatKind, stats::Stat>,
    /// What `profile` shows that nothing else does: title, description,
    /// achievements, history (`profile.rs`).
    pub profile: profile::Profile,
    /// Race, profession, gender and age, from `info`.
    pub identity: stats::Identity,
    /// Groups taught since a caller last asked, for one that persists them.
    ///
    /// **A mailbox, not state.** [`Self::take_taught`] empties it.
    ///
    /// This crate does no I/O -- an architecture test enforces it -- so the
    /// model reports what changed and the session decides what that costs.
    taught: std::collections::BTreeSet<snapshot::Group>,
    /// The character's name, from `<app char=>` via [`Frame::AppInfo`].
    ///
    /// **Who this file is about.** `identity.race` and its siblings come from
    /// `info`, which a character may never have run; this comes from the login
    /// burst, so it is known from the first moments of every session.
    ///
    /// [`Frame::AppInfo`]: cena_protocol::Frame::AppInfo
    pub name: Option<String>,
    /// `<playerID id=>`, verbatim: the game's number for this character, from
    /// the login burst. [`Self::exist_id`] is what the game's links call it.
    pub player_id: Option<String>,
    /// The instance -- `Prime`, `Platinum`, `Shattered`, `Test`.
    ///
    /// **Part of the identity, not decoration.** Lich keys its table
    /// `game_name` (`infomon.rb:86`) because two characters of the same name
    /// on different instances are different characters, and `frame.rs` records
    /// the same fact about `<settingsInfo>`.
    pub instance: Option<String>,
    /// The 46 skills and the spell circles, from `skills`.
    pub skills: skills::SkillSet,
    /// The five PSM tables, each from its own `<category> list all all`.
    pub psms: psm::PsmSet,
    /// Enhancive totals, from `inventory enhancive totals`.
    pub enhancives: enhancive::EnhanciveTotals,
    /// Silver, notes and the event currencies.
    pub currency: currency::Currency,
    /// Society, citizenship, warcries and resources.
    ///
    /// Filled from single lines rather than from a block: most of these
    /// arrive during ordinary play, not during a sync. See
    /// [`standing`] for why that distinction drives the store's design.
    pub standing: standing::Standing,
    /// Whether Shroud of Deception (spell 1212) is believed active.
    ///
    /// **Set by the effects layer, read here.** While it is true, an `info`
    /// report's identity fields are refused: the spell falsifies them, and a
    /// stored lie is permanent because nothing later retracts it. The stat
    /// numbers are stored regardless -- the shroud does not touch those.
    pub shrouded: bool,
}

impl Character {
    /// The stance as a typed value, when one has been reported.
    ///
    /// [`Self::stance`] stays the string the wire sent, and this reads it —
    /// the two are not redundant. The raw field is what Rule 2.2 preserves
    /// (a stance name the game adds later must survive to a display even
    /// though [`Stance`] cannot name it), and this is what a behavior
    /// compares against.
    ///
    /// Prefers `text=`, which carries the name; falls back to the bar's
    /// `value=`, which carries only a percent. MEASURED: the two agreed in
    /// **all 19,526** `pbarStance` readings across the 208 live logs, so the
    /// fallback is a safety net rather than a second source of truth.
    #[must_use]
    pub fn stance_typed(&self) -> Option<Stance> {
        if let Some(text) = self.stance.as_deref()
            && let Some((stance, _)) = Stance::parse_bar_text(text)
        {
            return Some(stance);
        }
        self.stance_percent.and_then(Stance::from_percent)
    }

    /// Forget what belonged to the connection; keep what is still true.
    ///
    /// M3 step 9, **rewritten 2026-09-20** after the author corrected the rule
    /// twice. `GameState::invalidate_for_reconnect` used to do
    /// `*character = Character::default()`; step 9 split that per group, on
    /// the reasoning that the four dialogs are absent from the login burst and
    /// are therefore `plan/12` §5.2's Invalidated set.
    ///
    /// **Both halves of that reasoning were wrong.**
    ///
    /// # The facts were wrong
    ///
    /// MEASURED 2026-09-20 across two captures, from `<app>` to the first
    /// client command: `dialogData id='expr'`, `id='injuries'`, `encumlevel`
    /// and `encumblurb` are all **in** the burst, with real values. Only
    /// `pbarStance` is genuinely absent -- zero occurrences in either burst.
    ///
    /// # And the rule was wrong
    ///
    /// > **AUTHOR, 2026-09-20:** *"time stops for 99.9% of things when you're
    /// > offline ... you can't really change rooms when you're logged off."*
    ///
    /// A logged-off character is out of the world. Nothing re-stances them,
    /// nothing wounds them, nothing changes what they are carrying. So
    /// absence from the burst is not a reason to forget: it means the server
    /// had no need to restate a fact that never stopped being true.
    ///
    /// # What that leaves
    ///
    /// **Everything here is kept**, and this method now clears exactly one
    /// field. It is retained rather than deleted because the destructure is
    /// load-bearing -- see below -- and because "the character model survives
    /// a reconnect intact" is a claim worth having a single place to state.
    ///
    /// | Field | Kept | Why |
    /// |---|---|---|
    /// | `experience` | yes | in the burst, AND the one thing that genuinely moves offline (*"you can absorb experience extremely slowly if you enable that option"*) -- so the burst's value is the authority and arrives unprompted |
    /// | `injuries` | yes | wounds do not heal or appear while out of the world; `id='injuries'` is in the burst |
    /// | `stance`, `stance_percent` | yes | **not** in the burst, and that is fine: nobody shifts a logged-off character's stance |
    /// | `encumbrance` and friends | yes | in the burst; nothing is picked up or dropped while away |
    /// | `stats`, `identity` | yes | taught by a command, which was step 9's original point and is the one part that survived |
    ///
    /// # `shrouded` is the exception, and it is not about elapsed time
    ///
    /// It is a **belief derived from another subsystem** -- the effects layer
    /// sets it -- and it is a *suppression* flag: while true, an `info`
    /// report's identity fields are refused, because Shroud of Deception
    /// falsifies them.
    ///
    /// A suppression flag that outlives the evidence for it is the one shape
    /// where keeping is worse than forgetting. If the spell dropped while we
    /// were away, a surviving `shrouded = true` makes the new session refuse
    /// every `info` identity on the strength of an effect nobody has
    /// re-observed -- **silently discarding good data**. Cleared, the worst
    /// case is that one `info` is believed before the effect list catches up,
    /// and the effect list then restores the guard.
    ///
    /// So the asymmetry is deliberate: a stale fact is corrected by the next
    /// observation, but a stale *refusal to observe* is not.
    ///
    /// # The destructuring is the point
    ///
    /// Every field named, mirroring `GameState::invalidate_for_reconnect` and
    /// for the identical reason: adding a field is a compile error here, and
    /// whoever adds it has to decide which side it belongs on. That matters
    /// more now that all but one field is on the "keep" side -- a new field
    /// silently defaulting to "keep" is the easy mistake, and this makes it a
    /// decision instead.
    pub(super) fn invalidate_for_reconnect(&mut self) {
        let Self {
            experience,
            injuries,
            observed_body_parts,
            stance,
            stance_percent,
            encumbrance,
            encumbrance_percent,
            encumbrance_detail,
            stats,
            identity,
            name,
            player_id,
            instance,
            taught,
            currency,
            skills,
            psms,
            enhancives,
            standing,
            shrouded,
            profile,
        } = self;

        // --- Cleared: a suppression flag whose evidence is gone ------------
        *shrouded = false;
        *observed_body_parts = 0;

        // `profile` must be run again to be true again: a title or an
        // achievement can change while a character is logged out, and nothing
        // re-sends this unasked.
        profile.clear();

        // --- Kept: a logged-off character is out of the world --------------
        let _ = (
            experience,
            injuries,
            stance,
            stance_percent,
            encumbrance,
            encumbrance_percent,
            encumbrance_detail,
            stats,
            identity,
            // KEPT. A balance is a fact about the character, and nothing in
            // the login burst re-states it -- clearing would leave a display
            // blank until the player happened to run `wealth`.
            currency,
            // KEPT. A fact taught just before the transport dropped is
            // still a fact, and dropping the mark would lose the only record
            // that it needs writing.
            taught,
            // KEPT. Who the character IS does not change across a
            // reconnect -- and the login burst re-sends `<playerID>` anyway,
            // so clearing would be undone within a few lines while leaving a
            // window where the store has no filename to write under.
            name,
            player_id,
            instance,
            // KEPT, all three, for the reason `stats` is kept: a command
            // taught them and no reconnect re-sends them. Enhancives are the
            // one worth a second thought -- they depend on what is WORN, and
            // a character could in principle change gear while disconnected.
            // They are kept anyway: the alternative is discarding good data on
            // the chance it went stale, and `inventory enhancive totals`
            // re-teaches the whole set whenever it is run.
            skills,
            psms,
            enhancives,
            // KEPT. Society, citizenship and warcries are facts about the
            // character, not about the connection -- and unlike `stats` they
            // are taught by ORDINARY PLAY as well as by a sync, so clearing
            // them would wait for a resync that nothing schedules. A resigned
            // society is announced when it happens; a reconnect announces
            // nothing.
            standing,
        );
    }

    /// Fold a `<progressBar>` that belongs to one of step 3's dialogs.
    ///
    /// Returns whether it was consumed, so the caller can fall through to the
    /// vitals path for everything else. The enclosing dialog is what separates
    /// these from vitals: the same `<progressBar>` shape carries all of them.
    pub(super) fn apply_bar(&mut self, dialog: &str, id: &str, text: &str, percent: u32) -> bool {
        let text = (!text.is_empty()).then(|| text.to_owned());
        match (dialog, id) {
            ("expr", "mindState") => {
                self.experience.mind_state = text;
                self.experience.mind_percent = Some(percent);
            }
            ("expr", "nextLvlPB") => {
                // Lich's gift pulse: counted only when the TEXT CHANGES, not
                // on every refresh of the bar (`common/xmlparser.rb:751`'s
                // `unless @next_level_text == attributes['text']`). The dialog
                // is re-sent constantly, so counting every arrival would tick
                // several times a second.
                if self.experience.next_level != text {
                    self.pulse_experience_bar();
                }
                self.experience.next_level = text;
                self.experience.next_level_percent = Some(percent);
            }
            // **By id, not by dialog**, and that is measured rather than lax.
            // `pbarStance` arrives in BOTH `stance` and `combat`, 130 times each
            // over 15 files, with identical values every time -- they are one
            // fact shown in two windows. Requiring `dialog == "stance"` rejected
            // the `combat` copy, which then fell through to `vitals` and showed
            // up as a gauge called `pbarStance` sitting beside health.
            //
            // The id is specific enough to be safe: unlike `health` or
            // `mindState`, nothing else on the wire is named `pbarStance`.
            (_, "pbarStance") => {
                self.stance = text;
                self.stance_percent = Some(percent);
            }
            ("encum", "encumlevel") => {
                self.encumbrance = text;
                self.encumbrance_percent = Some(percent);
            }
            _ => return false,
        }
        true
    }

    /// Fold a `<label>` that belongs to one of step 3's dialogs.
    pub(super) fn apply_label(&mut self, dialog: &str, id: &str, value: &str) {
        match (dialog, id) {
            ("expr", "yourLvl") => self.experience.level = Some(value.to_owned()),
            // Training points, moved down under Rule 4.1 when this file passed
            // its cap: `training::read_label` owns the four `expr` labels.
            ("expr", id) if training::read_label(&mut self.experience, id, value) => {}
            ("encum", "encumblurb") => self.encumbrance_detail = Some(value.to_owned()),
            _ => {}
        }
    }

    /// Fold an `<image>` from the `injuries` dialog.
    ///
    /// `name` equal to `id` means the part is whole; `Injury<n>` and `Scar<n>`
    /// are the severities.
    ///
    /// # A part can carry a wound AND a scar, and this used to lose one
    ///
    /// > **AUTHOR, 2026-09-20:** *"I'm healthy, I get injured and I get a
    /// > wound, rank 1, 2, or 3. If an empath heals me then the wound is
    /// > healed. If I heal from an herb though, the wound heals 1 rank, and I
    /// > gain a scar of the rank that was healed. So R2W -> R1W & R2S ->
    /// > R0W & R2S -> R1S -> Healthy."*
    ///
    /// So `R1W & R2S` is a real state: herb healing steps the wound down and
    /// leaves a scar recording the worst it reached. This method previously
    /// wrote `Injury { wound, scar: 0 }` on every `Injury<n>`, which **erased
    /// a known scar the moment a new wound arrived** -- and the doc justified
    /// it by saying a scar clears the wound, which has the relationship
    /// backwards.
    ///
    /// Lich gets this right and is the citation: `xmlparser.rb:811-815` sets
    /// the wound alone on `Injury<n>`, and sets `wound = 0` plus the scar on
    /// `Scar<n>`. The asymmetry is the rule -- a scar image means the wound is
    /// gone, a wound image says nothing about the scar.
    ///
    /// **The wire still shows only one per part**, because a wound covers a
    /// scar in the default injury mode. Keeping the last-known scar is
    /// therefore the best available answer rather than a complete one; see
    /// [`injured`] for what that costs.
    pub(super) fn apply_injury_image(&mut self, part: &str, name: &str) {
        body::apply_image(self, part, name);
    }
}

impl Character {
    /// Record who this character is, from the login burst.
    ///
    /// Empty strings are refused rather than stored: they would name a file
    /// after nobody, and `character_store::store_path` would reject them
    /// anyway -- better to hold `None` and say "not yet known".
    pub(crate) fn identify(&mut self, frame: &cena_protocol::Frame) {
        match frame {
            cena_protocol::Frame::AppInfo {
                character, game, ..
            } => {
                if !character.is_empty() {
                    self.name = Some(character.clone());
                }
                if !game.is_empty() {
                    self.instance = Some(game.clone());
                }
            }
            cena_protocol::Frame::PlayerId { id } if !id.is_empty() => {
                self.player_id = Some(id.clone());
            }
            _ => {}
        }
    }

    /// The character's own `exist` id, as the game's links name it: what
    /// tells a consumer "that link is me". `None` until `<playerID>` arrives.
    ///
    /// `-(10,000,000 + playerID)`. VERIFIED for one character, in two
    /// fixtures cut from one log (`cena-protocol/tests/FIXTURES.md`):
    /// `<playerID id='966483'/>` (`login_burst.xml:3`), and the same session's
    /// `info` naming the character `<a exist="-10966483">`
    /// (`character_info.xml:3`) -- the id Lich's own example gives him too
    /// (`gemstone/group.rb:482`). INFERRED for everyone else: every player
    /// link in Lich's examples is `-10` and six digits (`group.rb:417-509`).
    /// Lich stores the number (`common/xmlparser.rb:910-911`) and never
    /// compares it with a link, so no source states the rule.
    #[must_use]
    pub fn exist_id(&self) -> Option<String> {
        let number: u64 = self.player_id.as_deref()?.parse().ok()?;
        Some(format!("-{}", number.checked_add(10_000_000)?))
    }

    /// Read whatever command reports a completed chunk carries.
    ///
    /// Called once per prompt from `GameState::close_chunk`. Each report type
    /// is a pure function of the chunk (`blocks::InfoReport::read`,
    /// `skills::read_table`, ...), so this is dispatch and nothing else.
    pub(crate) fn consume_chunk(&mut self, chunk: &crate::state::chunks::Chunk) {
        if let Some(report) = blocks::InfoReport::read(chunk) {
            self.apply_info(&report);
            self.taught.insert(snapshot::Group::Stats);
            // Identity is REFUSED while shrouded, so nothing was taught --
            // marking it would stamp the group fresh on a report whose
            // identity fields were thrown away.
            if !self.shrouded {
                self.taught.insert(snapshot::Group::Identity);
            }
        }
        // `profile`'s own output, which arrives in the main window like every
        // other report (`profile.rs` records the capture that corrected this).
        let lines: Vec<String> = chunk.lines().iter().map(|l| l.runs.plain()).collect();
        if self.profile.read_lines(&lines) {
            self.absorb_profile();
        }
        if self.consume_standing(chunk) {
            self.taught.insert(snapshot::Group::Standing);
        }
        if self.currency.absorb_chunk(chunk) {
            self.taught.insert(snapshot::Group::Currency);
        }
        self.consume_psms(chunk);
        self.consume_skills(chunk);
        self.consume_enhancives(&lines);
        // NOT MARKED TAUGHT, and there is no `Group::Experience`.
        // `reconnect_invalidation.rs` records why: experience changes
        // continuously and `<dialogData id='expr'>` is the live authority, so
        // the snapshot has no experience field and a stored copy would be
        // stale the moment it was written. The report still fills the model --
        // it carries fame, deeds and field exp that the dialog does not.
        if let Some(report) = experience_report::ExperienceReport::read(chunk) {
            self.experience.absorb(&report);
        }
    }

    /// Count one change of the experience bar's text.
    ///
    /// Lich's `Gift.pulse` (`common/xmlparser.rb:751`), under a name that says
    /// what it counts. See [`Gift`] for why this is the only part of Lich's
    /// gift tracker that is ported.
    pub(crate) fn pulse_experience_bar(&mut self) {
        self.experience.gift.pulses = self.experience.gift.pulses.saturating_add(1);
    }

    /// Take the groups taught since this was last called.
    ///
    /// Empties the mailbox, so whoever takes them owns writing them -- a
    /// second caller would get nothing and silently skip a save.
    #[must_use]
    pub fn take_taught(&mut self) -> Vec<snapshot::Group> {
        std::mem::take(&mut self.taught).into_iter().collect()
    }

    /// Read the single-line facts a chunk carries.
    ///
    /// Separate from the report readers above because these are **not a
    /// report**: each line stands alone, and most of them arrive during
    /// ordinary play rather than inside a command's output. A chunk closed by
    /// a prompt after joining a society carries exactly one of them and no
    /// report at all.
    ///
    /// Every line is offered to every classifier. That is cheap -- they are
    /// prefix tests -- and it is the only shape that works when the same chunk
    /// can hold a `society` report, a `resource` report, and a PSM the
    /// character trained while the command was in flight.
    fn consume_standing(&mut self, chunk: &crate::state::chunks::Chunk) -> bool {
        let mut warcries = std::collections::BTreeSet::new();
        let mut saw_warcry_report = false;
        // Whether a value actually MOVED. A re-sync that finds everything
        // identical reports nothing, so it does not delay a pending write.
        let mut changed = false;

        for line in chunk.lines() {
            let text = line.text();

            if let Some(event) = standing::society_line(&text) {
                changed |= self.standing.apply_society(event);
            }
            if let Some(town) = standing::citizenship_line(&text) {
                let town = Some(town);
                changed |= self.standing.citizenship != town;
                self.standing.citizenship = town;
            }
            // A warcry report states the COMPLETE set, so the lines are
            // gathered and applied once below. Applying them one at a time
            // could not express "you have none".
            if let Some(warcry) = standing::warcry_line(&text) {
                saw_warcry_report = true;
                if let Some(warcry) = warcry {
                    warcries.insert(warcry);
                }
            }
            if let Some(amounts) = standing::resource_line(&text) {
                changed |= self.standing.resources != Some(amounts);
                self.standing.resources = Some(amounts);
            }
            if let Some((kind, amount)) = standing::suffused_line(&text) {
                changed |= self.standing.resource_type != Some(kind)
                    || self.standing.suffused != Some(amount);
                self.standing.resource_type = Some(kind);
                self.standing.suffused = Some(amount);
            }
            if let Some(charges) = standing::covert_arts_line(&text) {
                changed |= self.standing.covert_arts_charges != Some(charges);
                self.standing.covert_arts_charges = Some(charges);
            }
        }

        if saw_warcry_report {
            changed |= self.standing.set_warcries(warcries);
        }
        changed
    }

    /// Fold an `info` report into the typed stats.
    ///
    /// # Identity is not taken while Shroud of Deception is up
    ///
    /// Spell **1212** falsifies race, profession, gender and age in `info`
    /// output. Lich refuses to store those four while it is active
    /// (`infomon/parser.rb:243`, `:249`) and `Infomon.sync` force-STOPs the
    /// spell before scraping, warning `TEND TO YOUR SHROUD!` afterwards
    /// (`infomon/cli.rb:11-18`, `:41`).
    ///
    /// **The numbers are stored regardless**, because the shroud does not touch
    /// them -- which is why this is a partial refusal rather than a dropped
    /// report.
    ///
    /// Persisting a shrouded identity is permanent damage: nothing later says
    /// "that was a lie", so the wrong race stays until someone runs `info`
    /// again unshrouded.
    pub fn apply_info(&mut self, report: &blocks::InfoReport) {
        for (kind, line, bolded) in &report.stats {
            let slot = self.stats.entry(*kind).or_default();
            *slot = blocks::InfoReport::merge_into(line, *bolded, *slot);
        }
        if self.shrouded {
            return;
        }
        let identity = &report.identity;
        if identity.race.is_some() {
            self.identity.race.clone_from(&identity.race);
        }
        if identity.profession.is_some() {
            self.identity.profession.clone_from(&identity.profession);
        }
        if identity.gender.is_some() {
            self.identity.gender.clone_from(&identity.gender);
        }
        if identity.age.is_some() {
            self.identity.age = identity.age;
        }
    }
}
