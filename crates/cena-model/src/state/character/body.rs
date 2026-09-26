//! Body parts, and the wound/scar accessors over them.
//!
//! Ports `lib/gemstone/wounds.rb` and `lib/gemstone/scars.rb`, which are the
//! same file twice: a `diff` with the field names normalised shows they differ
//! in exactly one method (`Scars.all_scars` swaps the game's injury mode, and
//! `Wounds.all_wounds` does not). So this is one type read two ways rather
//! than two types.
//!
//! # Sixteen parts, not the wiki's fourteen
//!
//! `reference/wiki_clean/Wound.txt` lists fourteen body locations. The wire
//! carries **sixteen**: it adds `leftFoot` and `rightFoot`, which
//! `wounds.rb:24-25` names and `injured.rb`'s sneaking rule reads. Another
//! case of the wiki being authoritative where it speaks and incomplete
//! elsewhere (`inventory/10` §9b).
//!
//! # The composites are not the obvious groupings
//!
//! Three of them, from `wounds.rb:57-87`, and two are surprising enough that
//! the tests assert their membership rather than their results:
//!
//! | Composite | Parts | Note |
//! |---|---|---|
//! | `arms` | arms **and hands** | four parts, not two |
//! | `limbs` | arms, hands, legs | **no feet**, though the wire has them |
//! | `torso` | chest, abdomen, back **and both eyes** | eyes are not torso anatomically |
//!
//! `eherbs.lic` relies on the shapes: `Scars.limbs == 3` is its severed-limb
//! check and `Scars.reye == 3` its missing-eye check (`:3222-3223`), which is
//! why `limbs` excluding feet matters -- a severed foot is not a severed limb
//! for its purposes.
//!
//! Each takes the **maximum**, not a sum. A character with four rank-1 wounds
//! has `arms == 1`.

use std::collections::BTreeMap;

use super::Injury;

/// Keep injury folding and observation coverage together. An unrecognized
/// image preserves legacy injury behavior but cannot prove this part healthy.
pub(super) fn apply_image(character: &mut super::Character, part: &str, name: &str) {
    if let Some(index) = ALL_PARTS.iter().position(|id| *id == part) {
        if name == part
            || matches!(
                name,
                "Injury1" | "Injury2" | "Injury3" | "Scar1" | "Scar2" | "Scar3"
            )
        {
            character.observed_body_parts |= 1 << index;
        } else {
            character.observed_body_parts &= !(1 << index);
        }
    }
    let rank = |prefix: &str| -> Option<u8> { name.strip_prefix(prefix)?.parse().ok() };
    let known = character.injuries.get(part).copied().unwrap_or_default();
    let injury = if let Some(wound) = rank("Injury") {
        // A wound covers a scar; it is not evidence the old scar healed.
        Injury {
            wound,
            scar: known.scar,
        }
    } else if let Some(scar) = rank("Scar") {
        // A scar image means no wound remains over it.
        Injury { wound: 0, scar }
    } else {
        Injury::default()
    };
    if injury.is_hurt() {
        character.injuries.insert(part.to_owned(), injury);
    } else {
        character.injuries.remove(part);
    }
}

/// The sixteen body parts the `injuries` dialog names.
///
/// Wire spelling, which is what the dialog's `id` attribute carries and what
/// [`Character::injuries`](super::Character::injuries) is keyed by.
pub const ALL_PARTS: [&str; 16] = [
    "leftEye",
    "rightEye",
    "head",
    "neck",
    "back",
    "chest",
    "abdomen",
    "leftArm",
    "rightArm",
    "rightHand",
    "leftHand",
    "leftLeg",
    "rightLeg",
    "leftFoot",
    "rightFoot",
    "nsys",
];

/// `arms`: both arms **and both hands** (`wounds.rb:57-65`).
pub const ARMS: [&str; 4] = ["leftArm", "rightArm", "leftHand", "rightHand"];

/// `limbs`: arms, hands and legs -- **not feet** (`wounds.rb:67-77`).
///
/// The omission is load-bearing rather than an oversight to correct:
/// `eherbs.lic:3222` reads `Scars.limbs == 3` as "severed limb", and a severed
/// foot is not one for that purpose.
pub const LIMBS: [&str; 6] = [
    "leftArm",
    "rightArm",
    "leftHand",
    "rightHand",
    "leftLeg",
    "rightLeg",
];

/// `torso`: chest, abdomen, back **and both eyes** (`wounds.rb:79-89`).
///
/// The eyes are not anatomically torso. Ported as written because
/// `eherbs.lic` groups its herbs the same way -- `'minor organ wound'` covers
/// the same set -- so the name is the game's rather than Lich's.
pub const TORSO: [&str; 5] = ["rightEye", "leftEye", "chest", "abdomen", "back"];

/// Which of an [`Injury`]'s two tracks to read.
///
/// The one thing `wounds.rb` and `scars.rb` differ in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Track {
    /// Fresh injuries. Heal to nothing, or to a scar.
    Wound,
    /// What a partially-healed wound left behind.
    ///
    /// **Only visible once no wound covers it** in the game's default injury
    /// mode, which is what `Scars.all_scars` swaps modes to defeat. The model
    /// retains the last scar it was shown (`character.rs`'s
    /// `apply_injury_image`), so this reports what is known rather than what
    /// is currently displayed.
    Scar,
}

/// Wound and scar ranks by body part.
///
/// A thin reader over [`Character::injuries`](super::Character::injuries),
/// so it holds a borrow rather than a copy.
#[derive(Debug, Clone, Copy)]
pub struct Body<'a> {
    parts: &'a BTreeMap<String, Injury>,
}

impl<'a> Body<'a> {
    /// Read over one character's injuries.
    #[must_use]
    pub const fn new(parts: &'a BTreeMap<String, Injury>) -> Self {
        Self { parts }
    }

    /// One part's rank on one track.
    ///
    /// **Zero for a part that is whole**, and zero for a part the wire has
    /// never mentioned. Those are the same thing here: the dialog sends every
    /// part it knows about, and `apply_injury_image` removes a part that heals
    /// rather than storing a zero, so absence IS "no injury".
    #[must_use]
    pub fn rank(&self, part: &str, track: Track) -> u8 {
        let injury = self.parts.get(part).copied().unwrap_or_default();
        match track {
            Track::Wound => injury.wound,
            Track::Scar => injury.scar,
        }
    }

    /// The worst rank across a group, as the composites do.
    ///
    /// **Maximum, not sum.** Four rank-1 wounds give 1.
    #[must_use]
    pub fn worst(&self, parts: &[&str], track: Track) -> u8 {
        parts.iter().map(|p| self.rank(p, track)).max().unwrap_or(0)
    }

    /// `arms`: the worst of both arms and both hands.
    #[must_use]
    pub fn arms(&self, track: Track) -> u8 {
        self.worst(&ARMS, track)
    }

    /// `limbs`: the worst of arms, hands and legs. **Feet excluded.**
    #[must_use]
    pub fn limbs(&self, track: Track) -> u8 {
        self.worst(&LIMBS, track)
    }

    /// `torso`: the worst of chest, abdomen, back and both eyes.
    #[must_use]
    pub fn torso(&self, track: Track) -> u8 {
        self.worst(&TORSO, track)
    }

    /// Every part's rank on one track, including the whole ones.
    ///
    /// `wounds.rb:99-101`'s `all_wounds`. Unlike
    /// [`Character::injuries`](super::Character::injuries) -- which holds only
    /// hurt parts, so `is_empty` answers "unhurt" without a scan -- this lists
    /// all sixteen, because a caller asking for "all wounds" wants a complete
    /// picture rather than a sparse one.
    #[must_use]
    pub fn all(&self, track: Track) -> BTreeMap<&'static str, u8> {
        ALL_PARTS
            .into_iter()
            .map(|part| (part, self.rank(part, track)))
            .collect()
    }

    /// Is anything hurt on this track?
    #[must_use]
    pub fn any(&self, track: Track) -> bool {
        ALL_PARTS.iter().any(|p| self.rank(p, track) > 0)
    }

    /// **A severed limb**: `eherbs.lic:3222`'s check.
    ///
    /// A rank-3 limb SCAR, which the wiki describes as *"a missing right
    /// arm"*. Named here because the herb that treats it is its own type
    /// (`'severed limb'`) and a caller should not have to know the magic
    /// number.
    #[must_use]
    pub fn has_severed_limb(&self) -> bool {
        self.limbs(Track::Scar) == 3
    }

    /// **A missing eye**: `eherbs.lic:3223`'s check.
    ///
    /// A rank-3 eye scar -- *"a missing right eye"*. Note this reads the eyes
    /// directly rather than through [`Self::torso`], which would also match a
    /// rank-3 chest scar.
    #[must_use]
    pub fn has_missing_eye(&self) -> bool {
        self.rank("leftEye", Track::Scar) == 3 || self.rank("rightEye", Track::Scar) == 3
    }
}
