//! What a shopkeeper or the game says to a selling step that is not a loot
//! fact.
//!
//! The prices, the appraisals, the refusals as too valuable or worthless,
//! the notes and the bank's answers are `plan/34`'s `LootFact`s, read from
//! the driver's own fold of the stream. These are the rest: the lines eloot's
//! `sell_item`, `appraise` and `pawnshop` wait on (`eloot.lic:7841`, `:5929`,
//! `:7498`) that say something about the step but nothing about loot.

/// A reply that is not a loot fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// The shop does not buy this kind of thing at all: *not quite my
    /// field*, *Can't say I'm interested*, *not a junkshop*, *don't buy
    /// trash*, *as if you were a lunatic*, *only deal in gems and jewelry*.
    WrongShop,
    /// The bulk sale was accepted: *inspects the contents carefully*.
    SackInspected,
    /// `analyze` says the item carries an appearance to transfer: a
    /// transmog, kept when the profile says so.
    Transmog,
    /// `analyze` says `ALTER 41`: kept (`eloot.lic:7517`).
    Alter41,
    /// A note read: *has a value of N silver*.
    NoteValue(u64),
    /// The item could not be picked up: *Get what*, *could not find*, *unable
    /// to handle the additional load*.
    CannotFetch,
    /// `You can't wear that`.
    CannotWear,
    /// `bundle remove` took the bundle apart: *Those were the last two*,
    /// one skin in each hand (`furrier`, `eloot.lic:6905`).
    LastTwo,
}

/// Read one reply line. `None` when it says nothing this planner acts on.
#[must_use]
pub fn classify(line: &str) -> Option<Reply> {
    let text = line.trim();
    let has = |needle: &str| text.contains(needle);
    if has("not quite my field")
        || has("Can't say I'm interested in that")
        || has("not a junkshop")
        || has("don't buy trash")
        || has("as if you were a lunatic")
        || has("only deal in gems and jewelry")
    {
        return Some(Reply::WrongShop);
    }
    if has("inspects the contents carefully") {
        return Some(Reply::SackInspected);
    }
    if has("allows you to transfer its appearance") {
        return Some(Reply::Transmog);
    }
    if has("ALTER 41") {
        return Some(Reply::Alter41);
    }
    if let Some(rest) = text.split("has a value of ").nth(1)
        && let Some(figure) = rest.split(" silver").next()
        && let Some(value) = grouped(figure)
    {
        return Some(Reply::NoteValue(value));
    }
    if text.starts_with("Get what")
        || has("I could not find what you were referring to")
        || has("unable to handle the additional load")
    {
        return Some(Reply::CannotFetch);
    }
    if has("You can't wear that") {
        return Some(Reply::CannotWear);
    }
    if has("Those were the last two") {
        return Some(Reply::LastTwo);
    }
    None
}

/// `1,234` as a number; `None` unless every character is a digit or a comma.
fn grouped(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit() || b == b',') {
        return None;
    }
    text.replace(',', "").parse().ok()
}
