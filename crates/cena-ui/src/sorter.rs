//! `;sorter`: a container look shown as one line per category.
//!
//! Ported from `VellumFE`'s `src/core/sorter.rs`, itself the native cousin of
//! Lich's `sorter.lic` (Tillmen, 0.9; `reference/lich_repo_mirror/lib/`).
//! With sorting on, a main-stream look such as
//!
//! ```text
//! In the mahogany box you see a bright gold ingot, some silver coins, a smooth amber wand, a piece of brown jade, a steel lockpick, a pinch of electrum dust and a tar black tourmaline.
//! ```
//!
//! finishes as a header and one line per category, the label bold:
//!
//! ```text
//! In the mahogany box:
//!   valuable (1): bright gold ingot
//!   other (1): some silver coins
//!   wand (1): smooth amber wand
//!   gem (3): pinch of electrum dust, piece of brown jade, tar black tourmaline
//!   lockpick (1): steel lockpick
//! ```
//!
//! (That look and four more, verbatim, are `tests/fixtures/container_looks.xml`.)
//!
//! It sends nothing and reads no state. It is pure over the line's own
//! pieces, because the look already carries every item as an
//! `<a exist= noun=>` link (`VellumFE`'s reasoning, `sorter.rs:12-15`) -- so
//! it lives in the projection, beside the line assembly it rewrites
//! (`plan/30` §4), and the session's story is unchanged underneath it.
//!
//! # What is `VellumFE`'s
//!
//! - **The gate:** a line starting `In the ` or `On the ` with ` you see `,
//!   or `Peering into ` with `you see `, and never one that names both an
//!   `In the` and an `On the` ([`is_container_look`], `sorter.rs:32-41`).
//! - **The container is the first object link; the items are the rest**
//!   (`sorter.rs:102-108`).
//! - **The category is every type the item has, joined by commas, or
//!   `other`** (`sorter.rs:120-126`, which is Lich's `item.type || 'other'`,
//!   `sorter.lic:130`). The types are `cena_model`'s `gameobj` table, a set,
//!   so they join in name order. Lich joins in the data file's order, which
//!   is the same for the base types (the file lists them alphabetically) and
//!   differs only among the event and customization tags after them.
//! - Categories in first-seen order; duplicate names collapse to a count;
//!   items ordered by the last word of their name, stably (`sorter.rs:110-146`,
//!   `:172-181`).
//! - The header is the line up to and including the container, then `:`;
//!   each category line is `  gem (3): ` in bold, then the items, a count
//!   after any duplicate (`sorter.rs:148-157`, `:182-212`).
//!
//! `VellumFE`'s knobs -- user rules, category order, renames, counts or bold
//! off, alpha or look order -- belong to its `.sorter edit` window and its
//! settings file. Hydra has neither, so only the defaults are ported
//! (`config/settings.rs:858-868`): a knob with one value is not a knob
//! (`plan/05` §-1).
//!
//! # What is stricter: nothing is dropped
//!
//! `VellumFE` takes every link after the first as an item and throws the rest
//! of the line away. MEASURED over one month of the author's logs
//! (`E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi\2026\09\xml`, 208 files), that
//! loses text on **182 of 475** container looks -- every look in the herb
//! kit, which goes on past its list:
//!
//! ```text
//! ... and a pure potion.  The kit contains DOSEs of the following solid herbs: wolifrew (143), basal (120), ...
//! ```
//!
//! Sorted `VellumFE`'s way the kit itself became an item and every dose count
//! was gone. Counted with
//! `grep -hoE '(In|On) the <a exist="[0-9]+" noun="[^"]*">[^<]*</a> you see ' *.xml | wc -l`
//! (475) and `grep -hoE '</a>\.  The <a exist="[0-9]+" noun="kit">kit</a> contains DOSEs' *.xml | wc -l`
//! (182).
//!
//! So a line is sorted only when it **is** a list, end to end: after the
//! container ` you see ` and an article; between items `, ` or ` and ` and an
//! article; after the last item a `.` that ends the line. Any other shape is
//! shown as it came. A tag walk over the same 475 lines (a scratchpad script,
//! not committed) found that shape in **293**, and the 182 kit looks are
//! exactly the rest.
//!
//! Text after an item's link stays with the item, where `VellumFE` dropped it:
//! `smooth turquoise leather journal embossed with a peacock feather` (2 of
//! the 293, both a shop's shelf). A trailing text holding a `.` is a sentence
//! boundary, not an item's own, and the line is left alone.

use cena_model::state::gameobj::classify;

use crate::StyledRun;

/// The category of an item the data does not type (`sorter.lic:130`).
const OTHER: &str = "other";

/// One piece of a line as it was pushed, before same-style pieces merge:
/// the only place that knows where an object's link began and ended.
#[derive(Clone, Debug)]
pub(crate) struct Piece {
    /// The text and its style.
    pub(crate) run: StyledRun,
    /// The `noun=` of the object the text names, when it names one.
    pub(crate) noun: Option<String>,
}

/// True when this main-stream text is a container look worth sorting.
///
/// `VellumFE`'s gate, verbatim (`sorter.rs:32-41`), which mirrors sorter.lic's,
/// including refusing a line that describes an `In` and an `On` surface at
/// once. A cheap first test: [`sort`] also requires the list's shape.
#[must_use]
pub(crate) fn is_container_look(text: &str) -> bool {
    let sees = text.contains(" you see ");
    let in_form = text.starts_with("In the ") && sees;
    let on_form = text.starts_with("On the ") && sees;
    let peering = text.starts_with("Peering into ") && text.contains("you see ");
    if !(in_form || on_form || peering) {
        return false;
    }
    !(text.contains("In the ") && text.contains("On the "))
}

/// One distinct item in a category.
struct Entry {
    /// Its name and trailing text, lowercased: what makes two items the same.
    key: String,
    /// The last word of its name, lowercased: sorter.lic's order.
    last_word: String,
    count: usize,
    /// The first occurrence's runs, style intact.
    runs: Vec<StyledRun>,
}

/// One category and its items, in first-seen order.
struct Bucket {
    category: String,
    entries: Vec<Entry>,
}

/// The replacement lines for a container look, as runs; `None` when the
/// line is not one, or is not a list from end to end (module docs).
pub(crate) fn sort(pieces: &[Piece]) -> Option<Vec<Vec<StyledRun>>> {
    let full: String = pieces.iter().map(|piece| piece.run.text.as_str()).collect();
    if !is_container_look(&full) {
        return None;
    }
    let (gaps, objects) = cut(pieces);
    let (container, items) = objects.split_first()?;
    if items.is_empty() {
        return None;
    }
    let opening = text(&gaps[1]);
    if !matches!(strip_article(&opening), " you see " | ", you see ") {
        return None;
    }
    let mut buckets: Vec<Bucket> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        // The gap after this item: another item follows it, or the line ends.
        let after = &gaps[index + 2];
        let after_text = text(after);
        let tail_len = if index + 1 < items.len() {
            tail_before_separator(&after_text)?
        } else {
            after_text.trim_end().strip_suffix('.')?.len()
        };
        if after_text[..tail_len].contains('.') {
            return None;
        }
        file(
            &mut buckets,
            *item,
            &after_text[..tail_len],
            leading(after, tail_len),
        );
    }
    Some(render(&gaps[0], container.1, buckets))
}

/// Cut a line at its objects: `gaps[k]` is the text before `objects[k]`,
/// and the last gap is the text after the last object, so there is always
/// one more gap than objects.
///
/// One piece is one object. A link whose text changed style partway would
/// arrive as two, with an empty gap between that no list has, so that line
/// is shown as it came. None did: no look among the 475 measured carries a
/// tag inside its visible text.
fn cut(pieces: &[Piece]) -> (Vec<Vec<StyledRun>>, Vec<(&str, &StyledRun)>) {
    let mut gaps: Vec<Vec<StyledRun>> = vec![Vec::new()];
    let mut objects = Vec::new();
    for piece in pieces {
        if let Some(noun) = &piece.noun {
            objects.push((noun.as_str(), &piece.run));
            gaps.push(Vec::new());
        } else if let Some(gap) = gaps.last_mut() {
            gap.push(piece.run.clone());
        }
    }
    (gaps, objects)
}

/// File one item, with its own trailing text, under its category.
fn file(
    buckets: &mut Vec<Bucket>,
    item: (&str, &StyledRun),
    tail: &str,
    tail_runs: Vec<StyledRun>,
) {
    let (noun, run) = item;
    let name = run.text.trim();
    let types = classify(noun, name).types;
    let category = if types.is_empty() {
        OTHER.to_owned()
    } else {
        types.into_iter().collect::<Vec<_>>().join(",")
    };
    let key = format!("{name}{tail}").to_lowercase();
    let index = if let Some(found) = buckets.iter().position(|b| b.category == category) {
        found
    } else {
        buckets.push(Bucket {
            category,
            entries: Vec::new(),
        });
        buckets.len() - 1
    };
    let entries = &mut buckets[index].entries;
    if let Some(entry) = entries.iter_mut().find(|entry| entry.key == key) {
        entry.count += 1;
        return;
    }
    let mut runs = vec![run.clone()];
    runs.extend(tail_runs);
    entries.push(Entry {
        key,
        last_word: name.rsplit(' ').next().unwrap_or_default().to_lowercase(),
        count: 1,
        runs,
    });
}

/// The header -- the line up to and including the container, then `:` --
/// and a bold-labelled line per category.
fn render(
    opening: &[StyledRun],
    container: &StyledRun,
    buckets: Vec<Bucket>,
) -> Vec<Vec<StyledRun>> {
    let mut header = opening.to_vec();
    header.push(container.clone());
    header.push(plain(":"));
    let mut lines = vec![header];
    for mut bucket in buckets {
        bucket.entries.sort_by(|a, b| a.last_word.cmp(&b.last_word));
        let total: usize = bucket.entries.iter().map(|entry| entry.count).sum();
        let mut line = vec![StyledRun {
            text: format!("  {} ({total}): ", bucket.category),
            bold: true,
            ..StyledRun::default()
        }];
        for (index, entry) in bucket.entries.into_iter().enumerate() {
            if index > 0 {
                line.push(plain(", "));
            }
            line.extend(entry.runs);
            if entry.count > 1 {
                line.push(plain(&format!(" ({})", entry.count)));
            }
        }
        lines.push(line);
    }
    lines
}

fn plain(text: &str) -> StyledRun {
    StyledRun {
        text: text.to_owned(),
        ..StyledRun::default()
    }
}

fn text(runs: &[StyledRun]) -> String {
    runs.iter().map(|run| run.text.as_str()).collect()
}

/// `gap` less a closing article: the list names each item after `a `, `an `
/// or `some `, or with no article when the link's text carries its own
/// (`<a ...>some silver coins</a>`).
fn strip_article(gap: &str) -> &str {
    for article in ["a ", "an ", "some "] {
        if let Some(rest) = gap.strip_suffix(article)
            && (rest.is_empty() || rest.ends_with(' '))
        {
            return rest;
        }
    }
    gap
}

/// How much of the gap between two items is the first one's own text: the
/// gap must end the way a list does, `, ` or ` and ` then an article.
fn tail_before_separator(gap: &str) -> Option<usize> {
    let rest = strip_article(gap);
    rest.strip_suffix(", ")
        .or_else(|| rest.strip_suffix(" and "))
        .map(str::len)
}

/// The first `len` bytes of `runs`, styles kept. `len` is a character
/// boundary of their concatenation, so it is one of whichever run it falls in.
fn leading(runs: &[StyledRun], len: usize) -> Vec<StyledRun> {
    let mut left = len;
    let mut out = Vec::new();
    for run in runs {
        if left == 0 {
            break;
        }
        let take = left.min(run.text.len());
        out.push(StyledRun {
            text: run.text[..take].to_owned(),
            ..run.clone()
        });
        left -= take;
    }
    out
}

#[cfg(test)]
mod tests {
    //! The sorter over real container looks, through the line assembler.
    //!
    //! `tests/fixtures/container_looks.xml` is five looks from the author's logs
    //! (`E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi\2026\09\xml`), each cut whole
    //! with the prompt after it and Lich's timestamp prefix removed. No player
    //! is named in any of them.
    //!
    //! | Fixture line | Look | Source |
    //! |---|---|---|
    //! | 1 | a mahogany box | `2026-09-04_01-30-58.xml:1550` |
    //! | 3 | a plumille cloak | `2026-09-01_20-24-11.xml:9859` |
    //! | 5 | a dwarf skin backpack | `2026-09-01_20-24-11.xml:9869` |
    //! | 7 | a shop's wooden shelf | `2026-09-02_22-34-52.xml:23595` |
    //! | 9 | the herb kit | `2026-09-04_00-48-57.xml:358` |
    //!
    //! Each test pushes a line's main-stream text through
    //! [`LineAssembler::push_naming`] as the pump does, a run per text and per
    //! link with the link's noun, so every assertion passes through the
    //! assembler's recording and finishing and not only [`sort`]. The same
    //! bytes go through the real parser and the pump in
    //! `crates/cena/tests/web_sorter.rs`.
    //!
    //! The categories asserted are `cena_model`'s `gameobj` table's answers for
    //! these items, not chosen ones: between them the looks file items under a
    //! single type, under two joined (`armor,uncommon`), and under `other`.

    use super::*;
    use crate::{LineAssembler, StoryLine};

    const WIRE: &str = include_str!("../tests/fixtures/container_looks.xml");

    /// The fixture's line `index`, counting from 0.
    fn wire_line(index: usize) -> Option<&'static str> {
        WIRE.lines().nth(index)
    }

    /// What the main stream shows of a wire line: all after its last `</inv>`.
    /// The `<inv>` bodies before it are the container window's, a different frame.
    fn visible(line: &str) -> &str {
        line.rsplit_once("</inv>").map_or(line, |(_, main)| main)
    }

    /// Cut `text` as the parser does: a piece per text between links and per
    /// `<a exist= noun=>` link, with the link's noun.
    fn pieces(text: &str) -> Option<Vec<(String, Option<String>)>> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find("<a exist=\"") {
            if start > 0 {
                out.push((rest[..start].to_owned(), None));
            }
            let open_end = start + rest[start..].find('>')?;
            let noun = rest[start..open_end]
                .split("noun=\"")
                .nth(1)?
                .split('"')
                .next()?;
            let close = open_end + rest[open_end..].find("</a>")?;
            out.push((rest[open_end + 1..close].to_owned(), Some(noun.to_owned())));
            rest = &rest[close + "</a>".len()..];
        }
        if !rest.is_empty() {
            out.push((rest.to_owned(), None));
        }
        Some(out)
    }

    fn plain(text: &str) -> StyledRun {
        StyledRun {
            text: text.to_owned(),
            ..StyledRun::default()
        }
    }

    /// Push `text` on `stream`, the last run ending the line.
    fn assemble(assembler: &mut LineAssembler, stream: &str, text: &str) -> Option<Vec<StoryLine>> {
        let pieces = pieces(text)?;
        let last = pieces.len().checked_sub(1)?;
        let mut lines = Vec::new();
        for (index, (text, noun)) in pieces.iter().enumerate() {
            lines.extend(assembler.push_naming(
                stream,
                &plain(text),
                noun.as_deref(),
                index == last,
            ));
        }
        Some(lines)
    }

    fn line_text(line: &StoryLine) -> String {
        line.runs.iter().map(|run| run.text.as_str()).collect()
    }

    /// Fixture line `index`'s look, finished with sorting on.
    fn sorted(index: usize) -> Option<Vec<StoryLine>> {
        let mut assembler = LineAssembler::default();
        assembler.sort_containers(true);
        assemble(&mut assembler, "", visible(wire_line(index)?))
    }

    fn texts(lines: &[StoryLine]) -> Vec<String> {
        lines.iter().map(line_text).collect()
    }

    #[test]
    fn a_box_files_each_item_under_its_type() {
        let lines = sorted(0).unwrap();
        assert_eq!(
            texts(&lines),
            [
                "In the mahogany box:",
                "  valuable (1): bright gold ingot",
                "  other (1): some silver coins",
                "  wand (1): smooth amber wand",
                "  gem (3): pinch of electrum dust, piece of brown jade, tar black tourmaline",
                "  lockpick (1): steel lockpick",
            ]
        );
        // The label is bold, as sorter.lic's monsterbold; the items are not.
        assert_eq!(lines[4].runs[0].text, "  gem (3): ");
        assert!(lines[4].runs[0].bold);
        assert!(lines[4].runs[1..].iter().all(|run| !run.bold));
        assert!(
            lines
                .iter()
                .all(|line| !line.truncated && line.stream.is_empty())
        );
    }

    #[test]
    fn duplicates_collapse_to_a_count_in_last_word_order() {
        assert_eq!(
            texts(&sorted(2).unwrap()),
            [
                "In the plumille cloak:",
                "  other (8): slender wooden rod (2), material swatch (5), strand of veniom thread",
                "  magic (1): small statue",
            ]
        );
    }

    #[test]
    fn an_item_of_two_types_is_filed_under_both_joined() {
        assert_eq!(
            texts(&sorted(4).unwrap()),
            [
                "In the dwarf skin backpack:",
                "  other (1): tumbler of black cherry whiskey",
                "  uncommon (3): nacreous disir feather (2), stygian valravn quill",
                "  gem (1): carved basalt teardrop",
                "  magic (1): shimmering green orb",
                "  armor,uncommon (1): scratched spiked vultite greathelm",
                "  jewelry (1): alexandrite inset mithril circlet",
            ]
        );
    }

    #[test]
    fn a_surface_sorts_and_an_items_own_text_stays_with_it() {
        let lines = texts(&sorted(6).unwrap());
        assert_eq!(lines[0], "On the wooden shelf:");
        assert!(
            lines.iter().any(|line| line
                .contains("smooth turquoise leather journal embossed with a peacock feather")),
            "the journal keeps what follows its link: {lines:?}"
        );
        // Nothing is lost: the labels count all sixteen things on the shelf.
        let counted: usize = lines[1..]
            .iter()
            .filter_map(|line| {
                line.split_once(" (")?
                    .1
                    .split_once(')')?
                    .0
                    .parse::<usize>()
                    .ok()
            })
            .sum();
        assert_eq!(counted, 16, "{lines:?}");
    }

    /// **The look `VellumFE` would have broken.** It passes `VellumFE`'s gate --
    /// asserted, so this test cannot pass by the line never reaching the list
    /// check -- and goes on past its list, so it is shown whole, dose counts and
    /// all.
    #[test]
    fn the_herb_kit_is_shown_whole() {
        let look = visible(wire_line(8).unwrap());
        let plain_text: String = pieces(look)
            .unwrap()
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        assert!(is_container_look(&plain_text));
        let lines = sorted(8).unwrap();
        assert_eq!(texts(&lines), [plain_text]);
        assert!(line_text(&lines[0]).contains("wolifrew (143)"));
    }

    #[test]
    fn nothing_is_sorted_until_asked_nor_off_the_main_stream() {
        let look = visible(wire_line(0).unwrap());
        let mut off = LineAssembler::default();
        assert_eq!(assemble(&mut off, "", look).unwrap().len(), 1);

        let mut on = LineAssembler::default();
        on.sort_containers(true);
        assert_eq!(assemble(&mut on, "thoughts", look).unwrap().len(), 1);
        assert_eq!(assemble(&mut on, "main", look).unwrap().len(), 6);

        on.sort_containers(false);
        assert_eq!(assemble(&mut on, "", look).unwrap().len(), 1);
    }

    /// A sentence after the list is not the last item's own text. SYNTHETIC: no
    /// look among the 475 measured has one, so this is the guard's only input;
    /// without the guard the rod would read `slender wooden rod.  It glows`.
    #[test]
    fn a_sentence_after_the_list_leaves_the_line_alone() {
        let mut assembler = LineAssembler::default();
        assembler.sort_containers(true);
        let look = "In the <a exist=\"1\" noun=\"box\">box</a> you see a \
                    <a exist=\"2\" noun=\"rod\">slender wooden rod</a>.  It glows.";
        let lines = assemble(&mut assembler, "", look).unwrap();
        assert_eq!(
            texts(&lines),
            ["In the box you see a slender wooden rod.  It glows."]
        );
    }

    /// `VellumFE`'s `categorizes_counts_and_keeps_links` (`sorter.rs:285-306`),
    /// its synthetic look unchanged. Its lockpick lands in `other` there because
    /// that test's data pack has two types; Hydra's table has `lockpick`.
    #[test]
    fn vellumfe_categorizes_and_counts() {
        let mut assembler = LineAssembler::default();
        assembler.sort_containers(true);
        let look = "In the <a exist=\"77\" noun=\"backpack\">backpack</a> you see a \
                    <a exist=\"1\" noun=\"sapphire\">blue sapphire</a>, a \
                    <a exist=\"2\" noun=\"crystal\">quartz crystal</a>, a \
                    <a exist=\"3\" noun=\"sapphire\">blue sapphire</a> and a \
                    <a exist=\"4\" noun=\"lockpick\">copper lockpick</a>.";
        assert_eq!(
            texts(&assemble(&mut assembler, "", look).unwrap()),
            [
                "In the backpack:",
                "  gem (3): quartz crystal, blue sapphire (2)",
                "  lockpick (1): copper lockpick",
            ]
        );
    }

    /// `VellumFE`'s `ignores_non_look_lines_and_dual_surface`
    /// (`sorter.rs:308-328`): a line that is no look, one naming both an `In`
    /// and an `On` surface, and a look with nothing in it.
    #[test]
    fn vellumfe_non_looks_pass_through() {
        let mut assembler = LineAssembler::default();
        assembler.sort_containers(true);
        for line in [
            "You pick up a rock.",
            "In the <a exist=\"5\" noun=\"counter\">counter</a> you see a \
             <a exist=\"6\" noun=\"rock\">rock</a>. On the \
             <a exist=\"5\" noun=\"counter\">counter</a> you see a \
             <a exist=\"7\" noun=\"rock\">rock</a>.",
            "In the <a exist=\"9\" noun=\"pouch\">pouch</a> you see nothing.",
        ] {
            let lines = assemble(&mut assembler, "", line).unwrap();
            assert_eq!(lines.len(), 1, "{line}");
        }
    }

    /// Past [`MAX_SORTED_PIECES`](crate::lines) the look is shown as it came; a
    /// shorter one of the same shape sorts, so the bound is what decided.
    #[test]
    fn a_look_too_long_to_keep_is_shown_as_it_came() {
        let look = |items: usize| {
            let mut line =
                String::from("In the <a exist=\"1\" noun=\"locker\">locker</a> you see ");
            for index in 0..items {
                let separator = match index {
                    0 => "a ",
                    last if last + 1 == items => " and a ",
                    _ => ", a ",
                };
                line.push_str(separator);
                // The sorter reads nouns, never ids, so one id serves.
                line.push_str("<a exist=\"2\" noun=\"rock\">rock</a>");
            }
            line.push('.');
            line
        };
        let mut assembler = LineAssembler::default();
        assembler.sort_containers(true);
        let short = assemble(&mut assembler, "", &look(100)).unwrap();
        assert_eq!(texts(&short)[1], "  other (100): rock (100)");
        let long = assemble(&mut assembler, "", &look(600)).unwrap();
        assert_eq!(long.len(), 1);
        assert!(!long[0].truncated);
    }
}
