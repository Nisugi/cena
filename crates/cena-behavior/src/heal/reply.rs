//! What the game says to eating or drinking a herb, as a closed set: the
//! lines eherbs' `use_herbs` waits on (`eherbs.lic:1968`).

/// One answer to a herb step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// It went down: *You take a bite*, *You manage to take a drink*, *You
    /// eat your*, *Using your*.
    Used,
    /// *Why don't you leave some for others?*: a public herb eaten from
    /// enough; eherbs stops.
    LeaveSome,
    /// *You must pick the*: a herb that has to be picked up first.
    MustPick,
    /// *Get what?* or *I could not find*: the herb is not there.
    Gone,
    /// *You need a free hand*.
    NeedHand,
}

/// Read one line. `None` when it says nothing a herb step acts on.
#[must_use]
pub fn classify(line: &str) -> Option<Reply> {
    let text = line.trim();
    let has = |needle: &str| text.contains(needle);
    if text.starts_with("You take a bite")
        || text.starts_with("You take a drink")
        || text.starts_with("You manage to take a")
        || text.starts_with("You eat your")
        || text.starts_with("Using your")
    {
        return Some(Reply::Used);
    }
    if has("Why don't you leave some for others") {
        return Some(Reply::LeaveSome);
    }
    if has("You must pick the") {
        return Some(Reply::MustPick);
    }
    if text.starts_with("Get what") || has("I could not find what you were referring to") {
        return Some(Reply::Gone);
    }
    if has("You need a free hand") {
        return Some(Reply::NeedHand);
    }
    None
}
