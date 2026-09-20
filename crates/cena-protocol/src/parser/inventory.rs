//! `<inventoryManager>`: a whole-inventory snapshot and its `<i>` item rows.
//!
//! Split out of `dispatch.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. Adding these helpers put that file at 805 of
//! 800.
//!
//! # Why this was worth building
//!
//! The response carries the player's ENTIRE nested item tree -- every
//! container's contents, each item's weight and capacity, and its
//! closed/locked state -- none of which the passive stream provides. MEASURED
//! in the author's September logs: 36 snapshots, 5,364 rows, ~149 items each.
//!
//! Every one of those rows used to be lost. `<i>` sat in the parser's STYLING
//! arm, on the assumption that it was an italic tag, so each row degraded to
//! `Frame::Structural` with its attributes trapped in an unparsed string. See
//! `super::markup::is_markup` for the census that settled it.

use super::text;
use crate::frame::Frame;
use crate::parser::inner::inner_text;

/// Assemble an `<inventoryManager>` and the `<i>` rows in its body.
///
/// `<continuation>` rows are separated out: they are cursors saying the
/// snapshot is truncated, not items. None appear in the author's logs, but
/// treating one as an item would put a cursor in the inventory tree.
pub(super) fn inventory_manager(tag: &str) -> Frame {
    let mut items = Vec::new();
    let mut continuations = Vec::new();
    for part in inner_text(tag).split('<') {
        let child = format!("<{}>", part.trim_end_matches(['/', '>']).trim_end());
        if part.starts_with("i ") {
            if let Some(item) = inventory_item(&child) {
                items.push(item);
            }
        } else if part.starts_with("continuation") {
            continuations.push(crate::frame::Continuation {
                root: text::attribute(&child, "root").unwrap_or_default(),
                last: text::attribute(&child, "last").unwrap_or_default(),
            });
        }
    }
    Frame::InventoryManager(crate::frame::InventoryResponse {
        token: text::attribute(tag, "id").unwrap_or_default(),
        room: text::attribute(tag, "room").unwrap_or_default(),
        root: text::attribute(tag, "root"),
        after: text::attribute(tag, "after"),
        state: text::attribute(tag, "state"),
        items,
        continuations,
    })
}

/// One `<i>` item row.
///
/// `None` only when `id` or `loc` is missing: an item that cannot be anchored
/// in the tree. Ported from `VellumFE/src/core/state.rs:1212`, which is
/// likewise lenient everywhere else -- a malformed field degrades to a
/// default rather than losing the item.
fn inventory_item(tag: &str) -> Option<crate::frame::InventoryItem> {
    let id = text::attribute(tag, "id")?;
    let loc = text::attribute(tag, "loc")?;
    // The bare `room` has no parent to split from; everything else is
    // `relation,parent`.
    let (relation, parent) = if loc == "room" {
        ("room".to_owned(), "room".to_owned())
    } else {
        let (rel, parent) = loc.split_once(',')?;
        (rel.trim().to_owned(), parent.trim().to_owned())
    };
    let (article, adjective, noun) = split_name(&text::attribute(tag, "name").unwrap_or_default());
    let name = [article.as_str(), adjective.as_str(), noun.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    Some(crate::frame::InventoryItem {
        id,
        relation,
        parent,
        name,
        article,
        adjective,
        noun,
        // `$_` brackets the emphasised span in a long description; the
        // markers are display syntax, not part of the text.
        long: text::attribute(tag, "long")
            .map(|l| l.replace("$_", "").trim().to_owned())
            .filter(|l| !l.is_empty()),
        weight: num(tag, "weight"),
        encum: num(tag, "encum"),
        in_max: num(tag, "in_max"),
        on_max: num(tag, "on_max"),
        in_encum: num(tag, "in_encum"),
        in_selector: text::attribute(tag, "in_selector").filter(|s| !s.is_empty()),
        locker: text::attribute(tag, "locker").as_deref() == Some("1"),
        familyvault: text::attribute(tag, "familyvault").as_deref() == Some("1"),
        flags: text::attribute(tag, "flags")
            .map(|f| {
                f.split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// One numeric attribute, absent or unparsable becoming `None`.
fn num<T: std::str::FromStr>(tag: &str, attr: &str) -> Option<T> {
    text::attribute(tag, attr).and_then(|v| v.trim().parse().ok())
}

/// Split `name="article,adjective,noun"`, any of which may be empty.
///
/// A value that does not split into three keeps the whole string as the noun.
/// Losing the item would be worse than an odd noun.
fn split_name(raw: &str) -> (String, String, String) {
    match raw.splitn(3, ',').collect::<Vec<_>>()[..] {
        [a, adj, n] => (
            a.trim().to_owned(),
            adj.trim().to_owned(),
            n.trim().to_owned(),
        ),
        _ => (String::new(), String::new(), raw.trim().to_owned()),
    }
}
