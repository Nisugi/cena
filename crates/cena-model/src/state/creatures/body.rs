//! A creature's body, as the crit tables and the injury model name it.

use crate::crit::{Location, WoundLocation};

/// One of the sixteen body parts a wound can land on.
///
/// `CreatureInstance::BODY_PARTS` (`creature.rb:329`), whose spellings are the
/// same ids the character's own injury dialog uses (`leftArm`, `nsys` aside).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BodyPart {
    /// `abdomen`.
    Abdomen,
    /// `back`.
    Back,
    /// `chest`.
    Chest,
    /// `head`.
    Head,
    /// `leftArm`.
    LeftArm,
    /// `leftEye`. A "both eyes" wound lands here and on the right eye.
    LeftEye,
    /// `leftFoot`. No crit location or secondary wound maps here.
    LeftFoot,
    /// `leftHand`.
    LeftHand,
    /// `leftLeg`.
    LeftLeg,
    /// `neck`.
    Neck,
    /// `nerves`: the nervous system, which the injury dialog calls `nsys`.
    Nerves,
    /// `rightArm`.
    RightArm,
    /// `rightEye`. A "both eyes" wound lands here and on the left eye.
    RightEye,
    /// `rightFoot`. No crit location or secondary wound maps here.
    RightFoot,
    /// `rightHand`.
    RightHand,
    /// `rightLeg`.
    RightLeg,
}

impl BodyPart {
    /// Every part, in Lich's order.
    pub const ALL: [Self; 16] = [
        Self::Abdomen,
        Self::Back,
        Self::Chest,
        Self::Head,
        Self::LeftArm,
        Self::LeftEye,
        Self::LeftFoot,
        Self::LeftHand,
        Self::LeftLeg,
        Self::Neck,
        Self::Nerves,
        Self::RightArm,
        Self::RightEye,
        Self::RightFoot,
        Self::RightHand,
        Self::RightLeg,
    ];

    /// Lich's spelling, which is also the injury dialog's id.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Abdomen => "abdomen",
            Self::Back => "back",
            Self::Chest => "chest",
            Self::Head => "head",
            Self::LeftArm => "leftArm",
            Self::LeftEye => "leftEye",
            Self::LeftFoot => "leftFoot",
            Self::LeftHand => "leftHand",
            Self::LeftLeg => "leftLeg",
            Self::Neck => "neck",
            Self::Nerves => "nerves",
            Self::RightArm => "rightArm",
            Self::RightEye => "rightEye",
            Self::RightFoot => "rightFoot",
            Self::RightHand => "rightHand",
            Self::RightLeg => "rightLeg",
        }
    }

    /// Parse Lich's spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }

    /// The part a crit location wounds (`map_critranks_to_body_part`,
    /// `processor.rb:2410-2436`). `Unspecified` wounds nothing: the table
    /// row named no location, so a wound would be a guess.
    #[must_use]
    pub const fn from_location(location: Location) -> Option<Self> {
        Some(match location {
            Location::Head => Self::Head,
            Location::Neck => Self::Neck,
            Location::LeftEye => Self::LeftEye,
            Location::RightEye => Self::RightEye,
            Location::Chest => Self::Chest,
            Location::Abdomen => Self::Abdomen,
            Location::Back => Self::Back,
            Location::LeftArm => Self::LeftArm,
            Location::RightArm => Self::RightArm,
            Location::LeftHand => Self::LeftHand,
            Location::RightHand => Self::RightHand,
            Location::LeftLeg => Self::LeftLeg,
            Location::RightLeg => Self::RightLeg,
            Location::Nerves => Self::Nerves,
            Location::Unspecified => return None,
        })
    }

    /// The parts a secondary wound lands on (`apply_secondary_wound`,
    /// `processor.rb:2186-2215`): *"both eyes" is a real table location with
    /// no single body part -- it is two wounds, one per eye.*
    #[must_use]
    pub fn from_wound(location: WoundLocation) -> Vec<Self> {
        match location {
            WoundLocation::Head => vec![Self::Head],
            WoundLocation::Neck => vec![Self::Neck],
            WoundLocation::Chest => vec![Self::Chest],
            WoundLocation::Abdomen => vec![Self::Abdomen],
            WoundLocation::Back => vec![Self::Back],
            WoundLocation::Nerves => vec![Self::Nerves],
            WoundLocation::RightLeg => vec![Self::RightLeg],
            WoundLocation::BothEyes => vec![Self::LeftEye, Self::RightEye],
        }
    }
}
