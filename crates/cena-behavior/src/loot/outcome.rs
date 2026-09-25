//! What the game said back to a search, a `loot` or a drag, as a closed set.
//!
//! eloot reads its replies with three regex unions (`eloot.lic:541-580`,
//! `put_regex`, `look_regex`, and the search patterns at `:5647-5700`), and
//! acts on a dozen of the phrases. Those phrases are this enum. A line that
//! matches none is not an outcome, and the driver keeps reading until the
//! prompt.
//!
//! `Stored` is provisional: eloot confirms a stow by finding the item in
//! the bag's contents, not by the text (`:4040-4046`, `:4102-4108`), and
//! the driver will too; the line only says the game accepted the verb.

/// One thing the game said about a loot command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// `You search the …`: the corpse was searched (also `plunge`, `break`,
    /// `tentatively`, the in-hand critters' verbs).
    Searched,
    /// `not in any condition`: the character cannot search right now.
    NotInCondition,
    /// `There is no loot`: nothing on the floor for `loot room`.
    NothingHere,
    /// `loot room` took what the hands could carry and left the rest: drag
    /// the hands' contents away and loot again.
    TooMuch,
    /// `You need a free hand to do that`.
    NeedFreeHand,
    /// The game accepted a put or a drag.
    Stored,
    /// `won't fit`: the bag is full.
    WontFit,
    /// `It's closed!` or `That is closed`: the bag closes itself.
    Closed,
    /// The item crumbled when handled: never take one of these again.
    Crumbled,
    /// `That is not yours` / `Hey, that belongs to`: somebody else's.
    NotYours,
    /// `put something that you can't hold`: this character cannot hold it.
    Unlootable,
    /// `I could not find what you were referring to` / `Get what`: gone.
    NotFound,
    /// `There doesn't seem to be any way to do that`: the thing opened is
    /// not a container (`bag_loot`, `eloot.lic:4990`).
    NotAContainer,
    /// `You skinned`: a skin came off (`skin_obj_types`, `eloot.lic:5824`).
    Skinned,
    /// `You botched` / `You are unable to break through`: the skin is ruined.
    Botched,
    /// `already been`: the corpse was skinned before.
    AlreadySkinned,
    /// `You cannot skin`: this creature never yields a skin; learned.
    CannotSkin,
    /// `must be a member` / `can only skin` / `Because your account is
    /// free` / `not possible to get a worthwhile`: the game refuses this
    /// character's skinning; skinning ends for the visit.
    SkinNotAllowed,
    /// `You break through the crust` / `You crack open a portion`: a gem
    /// came out of the corpse into the left hand.
    BrokeThrough,
    /// `You kneel down` / `already kneeling`.
    Kneeled,
    /// `describe chimera`: *A huge scorpion tail rises high from the rear*,
    /// the one form of a rotting chimera that skins (`eloot.lic:5619`).
    ScorpionTail,
    /// `open` on a box that is still locked (`box_loot`, `eloot.lic:5074`).
    Locked,
    /// A box's coins are out: `You gather the remaining`, or a charm's
    /// swarm `reclaiming them`.
    Gathered,
    /// `You can only collect` / `You cannot hold any more silvers`: no more
    /// coins fit on the character.
    CoinsFull,
}

/// Read one reply line. `None` when it says nothing about a loot command.
#[must_use]
pub fn classify(line: &str) -> Option<Outcome> {
    let text = line.trim();
    let has = |needle: &str| text.contains(needle);
    if has("not in any condition") {
        return Some(Outcome::NotInCondition);
    }
    if has("There is no loot") {
        return Some(Outcome::NothingHere);
    }
    if has("You need a free hand to do that") {
        return Some(Outcome::NeedFreeHand);
    }
    if has("won't fit") {
        return Some(Outcome::WontFit);
    }
    if has("It's closed!") || has("That is closed") {
        return Some(Outcome::Closed);
    }
    if (has("crumble") && has("decay")) || has("crumbles into a pile of dust") {
        return Some(Outcome::Crumbled);
    }
    if has("That is not yours") || has("Hey, that belongs to") {
        return Some(Outcome::NotYours);
    }
    if has("put something that you can't hold") {
        return Some(Outcome::Unlootable);
    }
    if has("I could not find what you were referring to") || text.starts_with("Get what") {
        return Some(Outcome::NotFound);
    }
    if has("There doesn't seem to be any way to do that") {
        return Some(Outcome::NotAContainer);
    }
    // Skinning (`skin_obj_types`'s `skin_match`, `eloot.lic:5823-5840`).
    if text.starts_with("You skinned") {
        return Some(Outcome::Skinned);
    }
    if text.starts_with("You botched") || has("You are unable to break through") {
        return Some(Outcome::Botched);
    }
    if has("already been") && (has("skin") || has("harvest")) {
        return Some(Outcome::AlreadySkinned);
    }
    if has("You cannot skin") {
        return Some(Outcome::CannotSkin);
    }
    if has("must be a member")
        || has("can only skin")
        || has("Because your account is free")
        || has("not possible to get a worthwhile")
    {
        return Some(Outcome::SkinNotAllowed);
    }
    if has("You break through the crust") || has("You crack open a portion") {
        return Some(Outcome::BrokeThrough);
    }
    if has("You kneel down") || has("already kneeling") {
        return Some(Outcome::Kneeled);
    }
    if has("A huge scorpion tail rises high from the rear") {
        return Some(Outcome::ScorpionTail);
    }
    if has("is locked") || has("seems to be locked") {
        return Some(Outcome::Locked);
    }
    if has("You gather the remaining") || has("reclaiming them") {
        return Some(Outcome::Gathered);
    }
    if has("You can only collect") || has("You cannot hold any more silvers") {
        return Some(Outcome::CoinsFull);
    }
    // `loot_all`'s "too much" alternation (`eloot.lic:5185`).
    if (has("up and stow") && has("treasure")) || has("but quickly realize") {
        return Some(Outcome::TooMuch);
    }
    if ["You search ", "You plunge", "You break", "You tentatively"]
        .iter()
        .any(|verb| text.starts_with(verb))
    {
        return Some(Outcome::Searched);
    }
    if text.starts_with("You put ")
        || text.starts_with("You stow ")
        || text.starts_with("You place ")
    {
        return Some(Outcome::Stored);
    }
    None
}
