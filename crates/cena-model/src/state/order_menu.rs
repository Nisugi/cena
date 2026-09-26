//! A shop's `order` menu: what each item is ordered by (`plan/36` Stage 4).
//!
//! eherbs' `read_menu` (`eherbs.lic:3312-3325`) sends `order` and reads each
//! `<d cmd='order 12'>some acantha leaf</d>` as a name and its number. The
//! links are the parser's `Direct` kind, so this reads them off the chunk: a
//! chunk that lists any replaces the menu, since a menu is restated whole.

use std::collections::BTreeMap;

use super::chunks::Chunk;

/// The last `order` menu seen: item name, without its article, to number.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderMenu {
    items: BTreeMap<String, u32>,
}

impl OrderMenu {
    /// The number an item is ordered by, by its name with or without its
    /// article.
    #[must_use]
    pub fn number(&self, name: &str) -> Option<u32> {
        self.items.get(strip_article(name)).copied()
    }

    /// Every item the menu lists, by name, with its number.
    pub fn items(&self) -> impl Iterator<Item = (&str, u32)> {
        self.items.iter().map(|(name, n)| (name.as_str(), *n))
    }

    /// Whether a menu has been seen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Read one chunk.
    pub(crate) fn read_chunk(&mut self, chunk: &Chunk) {
        let mut seen = BTreeMap::new();
        for line in chunk.lines() {
            for link in line.links() {
                if let cena_protocol::frame::LinkKind::Direct { cmd } = &link.kind
                    && let Some(number) = cmd.strip_prefix("order ")
                    && let Ok(number) = number.trim().parse::<u32>()
                {
                    seen.insert(strip_article(link.text.trim()).to_owned(), number);
                }
            }
        }
        if !seen.is_empty() {
            self.items = seen;
        }
    }
}

/// eherbs strips `a ` or `an ` (`name.sub(/^an? /, '')`); `some` stays,
/// since the herb table's names keep it.
fn strip_article(name: &str) -> &str {
    name.strip_prefix("a ")
        .or_else(|| name.strip_prefix("an "))
        .unwrap_or(name)
}
