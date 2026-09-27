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
//! It sends nothing and reads no state. It is pure over one finished line,
//! because the look already carries every item as an `<a exist= noun=>` link
//! (`VellumFE`'s reasoning, `sorter.rs:12-15`). It runs **in the session**,
//! on the line before it is published (`plan/45` §4a): every viewer gets the
//! sorted lines, M8's triggers match them one by one as `VellumFE`'s
//! highlights do (it sorts before it highlights), and the model's scrollback
//! and the player log keep the look as the game sent it. Each item keeps its
//! run whole, link included, so a sorted item is as clickable as it was.
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

use cena_protocol::frame::{LinkKind, Style};
use cena_protocol::runs::{Run, Runs};

use crate::state::gameobj::classify;

/// The category of an item the data does not type (`sorter.lic:130`).
const OTHER: &str = "other";

/// Runs a line may have and still be sorted; past it the line is shown as
/// it came. MEASURED: the longest container look in a month of the author's
/// logs names 108 items, which is 217 runs.
pub const MAX_SORTED_RUNS: usize = 1024;

/// True when this main-stream text is a container look worth sorting.
///
/// `VellumFE`'s gate, verbatim (`sorter.rs:32-41`), which mirrors sorter.lic's,
/// including refusing a line that describes an `In` and an `On` surface at
/// once. A cheap first test: [`sort`] also requires the list's shape.
#[must_use]
pub fn is_container_look(text: &str) -> bool {
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
    /// The first occurrence's runs, style and link intact.
    runs: Vec<Run>,
}

/// One category and its items, in first-seen order.
struct Bucket {
    category: String,
    entries: Vec<Entry>,
}

/// The lines a container look shows as with `;sorter` on -- a header, then
/// one bold-labelled line per category -- or `None` when `line` is not a
/// look, or is not a list from end to end (module docs).
#[must_use]
pub fn sort(line: &Runs) -> Option<Vec<Runs>> {
    if line.runs.len() > MAX_SORTED_RUNS || !is_container_look(&line.plain()) {
        return None;
    }
    let (gaps, objects) = cut(&line.runs);
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
    let lines = render(&gaps[0], container.1, buckets);
    Some(lines.into_iter().map(|runs| Runs { runs }).collect())
}

/// The `noun=` of the object a run names, if it names one.
fn noun(run: &Run) -> Option<&str> {
    run.object().and_then(|link| match &link.kind {
        LinkKind::Exist { noun, .. } => Some(noun.as_str()),
        _ => None,
    })
}

/// Cut a line at its objects: `gaps[k]` is the text before `objects[k]`,
/// and the last gap is the text after the last object, so there is always
/// one more gap than objects.
///
/// One run is one object. A link whose text changed style partway would
/// arrive as two, with an empty gap between that no list has, so that line
/// is shown as it came. None did: no look among the 475 measured carries a
/// tag inside its visible text.
fn cut(runs: &[Run]) -> (Vec<Vec<Run>>, Vec<(&str, &Run)>) {
    let mut gaps: Vec<Vec<Run>> = vec![Vec::new()];
    let mut objects = Vec::new();
    for run in runs {
        if let Some(noun) = noun(run) {
            objects.push((noun, run));
            gaps.push(Vec::new());
        } else if let Some(gap) = gaps.last_mut() {
            gap.push(run.clone());
        }
    }
    (gaps, objects)
}

/// File one item, with its own trailing text, under its category.
fn file(buckets: &mut Vec<Bucket>, item: (&str, &Run), tail: &str, tail_runs: Vec<Run>) {
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
fn render(opening: &[Run], container: &Run, buckets: Vec<Bucket>) -> Vec<Vec<Run>> {
    let mut header = opening.to_vec();
    header.push(container.clone());
    header.push(plain(":"));
    let mut lines = vec![header];
    for mut bucket in buckets {
        bucket.entries.sort_by(|a, b| a.last_word.cmp(&b.last_word));
        let total: usize = bucket.entries.iter().map(|entry| entry.count).sum();
        let mut line = vec![Run {
            text: format!("  {} ({total}): ", bucket.category),
            style: Style {
                bold_depth: 1,
                ..Style::default()
            },
            link: None,
            inner_link: None,
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

fn plain(text: &str) -> Run {
    Run {
        text: text.to_owned(),
        style: Style::default(),
        link: None,
        inner_link: None,
    }
}

fn text(runs: &[Run]) -> String {
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
fn leading(runs: &[Run], len: usize) -> Vec<Run> {
    let mut left = len;
    let mut out = Vec::new();
    for run in runs {
        if left == 0 {
            break;
        }
        let take = left.min(run.text.len());
        out.push(Run {
            text: run.text[..take].to_owned(),
            ..run.clone()
        });
        left -= take;
    }
    out
}
