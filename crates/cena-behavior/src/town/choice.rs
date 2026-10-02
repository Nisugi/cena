//! What of the round the player chose to sell (`plan/61` step 6): eloot's
//! three `--` forms (`eloot.lic:8033-8048`) in Hydra's words. Each narrows
//! the round the profile would run, as eloot's do: what the profile does not
//! sell is not sold for being named, and what it never sells stays unsold.
//!
//! - `loot sell type <kinds>` (`--type`, `custom_type`, `:6744-6813`): only
//!   things of these kinds, as the object table names them; with `box` among
//!   them the boxes go to the pool first, as eloot's `process_boxes` does.
//! - `loot sell shop <shops>` (`--sellable`, `custom_sellable`,
//!   `:6700-6742`): only these shops of the round, and no pool.
//! - `loot sell item <names>` (`--sell`, `custom_list`, `:6653-6698`): only
//!   things whose names hold one of these, in any case, and no pool.
//!
//! A bag is sold whole only when everything in it the shop would buy is
//! chosen: a sack of gems sold whole for `item emerald` would sell its rubies
//! too. eloot keeps its gem sack whole for `--type` only when `gem` is named
//! (`gemshop`, `:6970`); the rule here is that one, said of any choice.

use cena_session::RoomItem;
use cena_session::gameobj::{Classification, ObjectTypes, categories};

use super::goods::Shop;

/// What of the round to sell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Choice {
    /// All of it: `loot sell`, and a hunt's round.
    #[default]
    All,
    /// Only things of these kinds, as the object table names them.
    Kinds(Vec<String>),
    /// Only at these shops.
    Shops(Vec<Shop>),
    /// Only things whose names hold one of these, lowercase.
    Names(Vec<String>),
}

/// The shops a choice may name, by the words for them: eloot's sellable
/// categories and its two others (`custom_sellable`, `:6709`), the
/// collectibles counter by both its spellings.
const SHOP_WORDS: &[(&str, Shop)] = &[
    ("gemshop", Shop::Gemshop),
    ("pawnshop", Shop::Pawnshop),
    ("furrier", Shop::Furrier),
    ("collectibles", Shop::Collectibles),
    ("collectible", Shop::Collectibles),
    ("chronomage", Shop::Chronomage),
];

impl Choice {
    /// Whether this thing is chosen.
    #[must_use]
    pub fn takes(&self, item: &RoomItem, types: &ObjectTypes) -> bool {
        match self {
            Self::All | Self::Shops(_) => true,
            Self::Kinds(kinds) => types.types.iter().any(|kind| kinds.contains(kind)),
            Self::Names(names) => {
                let name = item.text.to_ascii_lowercase();
                names.iter().any(|wanted| name.contains(wanted.as_str()))
            }
        }
    }

    /// Whether the round visits `shop`.
    #[must_use]
    pub fn visits(&self, shop: Shop) -> bool {
        match self {
            Self::Shops(shops) => shops.contains(&shop),
            Self::All | Self::Kinds(_) | Self::Names(_) => true,
        }
    }

    /// Whether the round takes its boxes to the pool.
    #[must_use]
    pub fn pools(&self) -> bool {
        match self {
            Self::All => true,
            Self::Kinds(kinds) => kinds.iter().any(|kind| kind == "box"),
            Self::Shops(_) | Self::Names(_) => false,
        }
    }

    /// `loot sell type|shop|item <what>`: the choice, or what is wrong with
    /// it. Kinds and shops are one word or more between commas (`|` too, as
    /// eloot splits them, `:6750`), or one a space apart; names are between
    /// commas, a space being part of a name.
    ///
    /// # Errors
    ///
    /// Nothing named, or a kind or a shop that is not one, said.
    pub fn parse(how: &str, what: &str) -> Result<Self, String> {
        let what = what.trim();
        let listed = |text: &str| -> Vec<String> {
            text.split([',', '|'])
                .map(|part| part.trim().to_ascii_lowercase())
                .filter(|part| !part.is_empty())
                .collect()
        };
        let words = |text: &str| -> Vec<String> {
            if text.contains([',', '|']) {
                listed(text)
            } else {
                text.split_whitespace()
                    .map(str::to_ascii_lowercase)
                    .collect()
            }
        };
        let choice = match how {
            "type" | "types" => {
                let kinds = categories(Classification::Type);
                let whole = what.to_ascii_lowercase();
                let named = if kinds.contains(whole.as_str()) {
                    vec![whole]
                } else {
                    words(what)
                };
                if let Some(unknown) = named.iter().find(|kind| !kinds.contains(kind.as_str())) {
                    let known: Vec<&str> = kinds.into_iter().collect();
                    return Err(format!(
                        "`{unknown}` is not a kind the object table knows. The kinds: {}.",
                        known.join(", ")
                    ));
                }
                Self::Kinds(named)
            }
            "shop" | "shops" => {
                let mut shops = Vec::new();
                for word in words(what) {
                    if word == "consignment" {
                        return Err(
                            "Hydra does not sell at consignment yet (`plan/61` step 9).".to_owned()
                        );
                    }
                    let Some((_, shop)) = SHOP_WORDS.iter().find(|(name, _)| *name == word) else {
                        return Err(format!(
                            "`{word}` is not a shop the round sells at: gemshop, pawnshop, furrier, collectibles, chronomage."
                        ));
                    };
                    if !shops.contains(shop) {
                        shops.push(*shop);
                    }
                }
                Self::Shops(shops)
            }
            _ => Self::Names(listed(what)),
        };
        let empty = match &choice {
            Self::Kinds(kinds) => kinds.is_empty(),
            Self::Shops(shops) => shops.is_empty(),
            Self::Names(names) => names.is_empty(),
            Self::All => false,
        };
        if empty {
            return Err(format!(
                "loot sell {how} <what>: name at least one. `loot help` says more."
            ));
        }
        Ok(choice)
    }
}

#[cfg(test)]
mod tests {
    use super::{Choice, Shop};

    #[test]
    fn kinds_by_commas_or_spaces_and_a_kind_of_two_words_whole() {
        assert_eq!(
            Choice::parse("type", "gem, skin"),
            Ok(Choice::Kinds(vec!["gem".to_owned(), "skin".to_owned()]))
        );
        assert_eq!(
            Choice::parse("type", "Gem skin"),
            Ok(Choice::Kinds(vec!["gem".to_owned(), "skin".to_owned()]))
        );
        assert_eq!(
            Choice::parse("type", "alchemy equipment"),
            Ok(Choice::Kinds(vec!["alchemy equipment".to_owned()]))
        );
        assert_eq!(
            Choice::parse("type", "gem|alchemy equipment"),
            Ok(Choice::Kinds(vec![
                "gem".to_owned(),
                "alchemy equipment".to_owned()
            ]))
        );
        let wrong = Choice::parse("type", "gem, wnad");
        assert!(
            wrong
                .as_ref()
                .is_err_and(|why| why.contains("`wnad`") && why.contains("wand")),
            "{wrong:?}"
        );
        assert!(Choice::parse("type", "  ").is_err());
    }

    #[test]
    fn shops_by_their_words_and_consignment_not_yet() {
        assert_eq!(
            Choice::parse("shop", "gemshop furrier collectible"),
            Ok(Choice::Shops(vec![
                Shop::Gemshop,
                Shop::Furrier,
                Shop::Collectibles
            ]))
        );
        assert!(Choice::parse("shop", "consignment").is_err_and(|why| why.contains("not sell at")));
        assert!(Choice::parse("shop", "bank").is_err());
    }

    #[test]
    fn names_between_commas_a_space_inside_one() {
        assert_eq!(
            Choice::parse("item", "Blue Crystal, silver wand"),
            Ok(Choice::Names(vec![
                "blue crystal".to_owned(),
                "silver wand".to_owned()
            ]))
        );
        assert!(Choice::parse("item", ",").is_err());
    }
}
