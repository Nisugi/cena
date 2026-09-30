//! Wrayth's highlights, names and ignores, read into triggers (`plan/45`
//! Stage 4, §2d). The first importer, on the author's word (§1 row 4).
//!
//! | Wrayth | Hydra |
//! |---|---|
//! | `<strings>` `<h text color bgcolor>` | a trigger on the words, with a look; category "Wrayth strings" |
//! | `line="y"` | the look covers the whole line |
//! | `case="y"` | case-sensitive (the author's reading, §1 row 4, UNVERIFIED) |
//! | `sound` | the trigger's `sound`, the path as Wrayth wrote it: played from there when it is there, and otherwise by its file name from the sounds folder (Stage 3) |
//! | `<names>` | the same, category "Wrayth names" |
//! | `<ignores>` | a squelch on the words, category "Wrayth ignores"; `disable='y'` switches the category off |
//! | `@N` | the colour the file's `<palette>` gives entry N |
//! | `skin`, empty | no colour of its own |
//! | `<presets>`, `<palette>`, `<macros>` | not triggers: the GUI's theme and keybinds (`plan/28`) |
//!
//! Each trigger is named by its words, which is how a player finds it in
//! `;trigger list`; words that come twice get `(2)`. Each carries its
//! `origin`, `Wrayth: <file>`, so importing that file again replaces what
//! it brought rather than doubling it, and so a later stage can tell a rule
//! the player wrote from one that came from elsewhere (§1 row 1).
//!
//! **Whole words.** A Wrayth highlight imports with Hydra's default, whole
//! words with the boundary only where the words' own edge is a letter,
//! digit or `_` (`plan/45` §5d). Whether Wrayth matches inside words is
//! UNVERIFIED; a player recalled it had no such setting.
//!
//! # The file, read without an XML library
//!
//! A Wrayth settings file is one `<settings>` element. MEASURED over the
//! author's four exports (2026-09-27): no comments, CDATA, processing
//! instructions or doctype. Comments are taken out first all the same: a
//! player who comments an entry out by hand means it gone, and read as
//! markup it would come in. Every element in `<strings>`, `<names>`,
//! `<ignores>` and `<palette>` closes itself and nothing stands between
//! them; values are double-quoted, but for `<ignores disable='n'>` in three
//! of the four; the entities are `&apos;`, `&gt;` and `&quot;`. Reading that
//! is the few functions at the foot of this file, the precedent the hunt's
//! YAML set (`hunt/yaml.rs`): an XML crate would read it too, and every
//! other XML document, which no settings file is.

use std::collections::BTreeMap;

use toml::{Table, Value};

/// The category Wrayth's `<strings>` come in.
pub const STRINGS: &str = "Wrayth strings";
/// The category Wrayth's `<names>` come in.
pub const NAMES: &str = "Wrayth names";
/// The category Wrayth's `<ignores>` come in, which its `disable` switches.
pub const IGNORES: &str = "Wrayth ignores";

/// A Wrayth settings file, read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Import {
    /// Each trigger, by the name it is given, in the file's order.
    pub triggers: Vec<(String, Table)>,
    /// Whether the ignores are on, when the file has an `<ignores>`.
    pub ignores_on: Option<bool>,
    /// How many came from `<strings>`, `<names>` and `<ignores>`.
    pub counts: [usize; 3],
    /// How many triggers play a sound.
    pub sounds: usize,
    /// What could not be carried, one sentence each.
    pub notes: Vec<String>,
}

/// One element's attributes, entities decoded.
type Attributes = BTreeMap<String, String>;

/// Read Wrayth's settings file `xml`, marking each trigger with `origin`.
///
/// # Errors
///
/// It has no `<strings>`, `<names>` or `<ignores>`.
pub fn read(xml: &str, origin: &str) -> Result<Import, String> {
    let xml = &uncommented(xml);
    let palette: BTreeMap<String, String> = section(xml, "palette")
        .map(|(_, entries)| entries)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|mut entry| Some((entry.remove("id")?, entry.remove("color")?)))
        .collect();
    let mut import = Import::default();
    let mut found = false;
    for (index, (name, category)) in [("strings", STRINGS), ("names", NAMES), ("ignores", IGNORES)]
        .into_iter()
        .enumerate()
    {
        let Some((own, entries)) = section(xml, name) else {
            continue;
        };
        found = true;
        if name == "ignores" {
            import.ignores_on = Some(own.get("disable").map(String::as_str) != Some("y"));
        }
        for entry in entries {
            let Some(trigger) = import.trigger(&entry, category, &palette, origin) else {
                continue;
            };
            if let Some(count) = import.counts.get_mut(index) {
                *count += 1;
            }
            let words = entry.get("text").cloned().unwrap_or_default();
            let named = unique(&words, &import.triggers);
            import.triggers.push((named, trigger));
        }
    }
    if !found {
        return Err("it has no <strings>, <names> or <ignores>: not a Wrayth settings file".into());
    }
    Ok(import)
}

impl Import {
    /// The trigger one `<h>` becomes, or `None`, noted, when it cannot be
    /// one.
    fn trigger(
        &mut self,
        entry: &Attributes,
        category: &str,
        palette: &BTreeMap<String, String>,
        origin: &str,
    ) -> Option<Table> {
        let Some(words) = entry.get("text").filter(|words| !words.is_empty()) else {
            self.notes.push(format!(
                "an entry in {category} has no words, and is left out"
            ));
            return None;
        };
        let mut trigger = Table::new();
        let mut put = |key: &str, value: Value| trigger.insert(key.to_owned(), value);
        put("category", Value::String(category.to_owned()));
        put("text", Value::String(words.clone()));
        if entry.get("case").map(String::as_str) == Some("y") {
            put("case_sensitive", Value::Boolean(true));
        }
        put("origin", Value::String(origin.to_owned()));
        if category == IGNORES {
            trigger.insert("squelch".to_owned(), Value::Boolean(true));
            return Some(trigger);
        }
        let mut look = Table::new();
        for (from, to) in [("color", "color"), ("bgcolor", "background")] {
            if let Some(colour) = self.colour(words, entry.get(from), palette) {
                look.insert(to.to_owned(), Value::String(colour));
            }
        }
        let sound = entry.get("sound").filter(|sound| !sound.is_empty());
        if look.is_empty() && sound.is_none() {
            self.notes.push(format!(
                "`{words}` has no colour Hydra can show, and is left out"
            ));
            return None;
        }
        if !look.is_empty() {
            if entry.get("line").map(String::as_str) == Some("y") {
                look.insert("span".to_owned(), Value::String("line".to_owned()));
            }
            trigger.insert("look".to_owned(), Value::Table(look));
        }
        if let Some(sound) = sound {
            trigger.insert("sound".to_owned(), Value::String(sound.clone()));
            self.sounds += 1;
        }
        Some(trigger)
    }

    /// A Wrayth colour as `#rrggbb`: its own, or its palette entry's; `None`
    /// for `skin`, for none, and, noted, for one Hydra cannot read.
    fn colour(
        &mut self,
        words: &str,
        written: Option<&String>,
        palette: &BTreeMap<String, String>,
    ) -> Option<String> {
        let written = written?.trim();
        if written.is_empty() || written.eq_ignore_ascii_case("skin") {
            return None;
        }
        let colour = match written.strip_prefix('@') {
            Some(entry) => {
                let Some(colour) = palette.get(entry) else {
                    self.notes.push(format!(
                        "`{words}`: its colour {written} is not in the file's palette, and is left out"
                    ));
                    return None;
                };
                colour.as_str()
            }
            None => written,
        };
        let hex = colour.strip_prefix('#').unwrap_or_default();
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            self.notes.push(format!(
                "`{words}`: its colour `{colour}` is not #rrggbb, and is left out"
            ));
            return None;
        }
        Some(colour.to_ascii_lowercase())
    }
}

/// The `<presets>` of Wrayth's settings file `xml`, each `<p id color>`'s
/// colour as `#rrggbb`, its `@N` palette entry resolved; and a note for
/// each colour that cannot be read. `skin`, and a preset with no colour,
/// are left out (`plan/57` step 8: the GUI's theme reads them as pins).
#[must_use]
pub fn presets(xml: &str) -> (BTreeMap<String, String>, Vec<String>) {
    let xml = &uncommented(xml);
    let palette: BTreeMap<String, String> = section(xml, "palette")
        .map(|(_, entries)| entries)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|mut entry| Some((entry.remove("id")?, entry.remove("color")?)))
        .collect();
    let mut out = BTreeMap::new();
    let mut notes = Vec::new();
    for entry in section(xml, "presets")
        .map(|(_, entries)| entries)
        .unwrap_or_default()
    {
        let Some(id) = entry.get("id").filter(|id| !id.is_empty()) else {
            continue;
        };
        let Some(written) = entry.get("color").map(|c| c.trim()) else {
            continue;
        };
        if written.is_empty() || written.eq_ignore_ascii_case("skin") {
            continue;
        }
        let colour = if let Some(index) = written.strip_prefix('@') {
            let Some(colour) = palette.get(index) else {
                notes.push(format!(
                    "the preset `{id}`'s colour {written} is not in the file's palette, and is left out"
                ));
                continue;
            };
            colour.as_str()
        } else {
            written
        };
        let hex = colour.strip_prefix('#').unwrap_or_default();
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            notes.push(format!(
                "the preset `{id}`'s colour `{colour}` is not #rrggbb, and is left out"
            ));
            continue;
        }
        out.insert(id.clone(), colour.to_ascii_lowercase());
    }
    (out, notes)
}

/// `words` as a name no trigger in `taken` has: the words, then `(2)`, `(3)`.
fn unique(words: &str, taken: &[(String, Table)]) -> String {
    let used = |name: &str| taken.iter().any(|(other, _)| other == name);
    let mut name = words.to_owned();
    let mut count = 1;
    while used(&name) {
        count += 1;
        name = format!("{words} ({count})");
    }
    name
}

/// `xml` with its comments taken out; one never closed runs to the end.
fn uncommented(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(at) = rest.find("<!--") {
        out.push_str(rest.get(..at).unwrap_or_default());
        let after = rest.get(at + "<!--".len()..).unwrap_or_default();
        rest = after
            .find("-->")
            .and_then(|end| after.get(end + "-->".len()..))
            .unwrap_or_default();
    }
    out.push_str(rest);
    out
}

/// The element `<name>` anywhere in `xml`: its own attributes, and each
/// element inside it. `None` when there is none.
fn section(xml: &str, name: &str) -> Option<(Attributes, Vec<Attributes>)> {
    let opener = format!("<{name}");
    let at = xml.match_indices(&opener).map(|(at, _)| at).find(|&at| {
        xml.get(at + opener.len()..)
            .and_then(|after| after.chars().next())
            .is_some_and(|c| c == '>' || c == '/' || c.is_whitespace())
    })?;
    let (own, closed, body_at) = element(xml, at)?;
    if closed {
        return Some((own, Vec::new()));
    }
    let end = xml.get(body_at..)?.find(&format!("</{name}>"))? + body_at;
    let body = xml.get(body_at..end)?;
    let mut inside = Vec::new();
    let mut from = 0;
    while let Some(next) = body.get(from..).and_then(|rest| rest.find('<')) {
        let (attributes, _, after) = element(body, from + next)?;
        inside.push(attributes);
        from = after;
    }
    Some((own, inside))
}

/// The element that opens at `at`: its attributes, whether it closes
/// itself, and where it ends. Quotes are honoured, so a `>` inside a value
/// does not end it.
fn element(xml: &str, at: usize) -> Option<(Attributes, bool, usize)> {
    let tag = xml.get(at + 1..)?;
    let after_name = tag
        .char_indices()
        .skip_while(|(_, c)| !c.is_whitespace() && *c != '>' && *c != '/');
    let mut attributes = Attributes::new();
    let (mut key, mut value, mut quote) = (String::new(), String::new(), None);
    let mut closed = false;
    for (offset, c) in after_name {
        match (quote, c) {
            (Some(q), c) if c == q => {
                attributes.insert(
                    std::mem::take(&mut key),
                    decoded(&std::mem::take(&mut value)),
                );
                quote = None;
            }
            (Some(_), c) => value.push(c),
            (None, '"' | '\'') => quote = Some(c),
            (None, '/') => closed = true,
            (None, '>') => return Some((attributes, closed, at + 1 + offset + 1)),
            (None, '=') => {}
            (None, c) if c.is_whitespace() => closed = false,
            (None, c) => {
                closed = false;
                key.push(c);
            }
        }
    }
    None
}

/// `text` with XML's entities made characters again; one Hydra does not
/// know is kept as written.
fn decoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(rest.get(..at).unwrap_or_default());
        let after = rest.get(at..).unwrap_or_default();
        let Some(end) = after.find(';') else {
            out.push_str(after);
            return out;
        };
        let name = after.get(1..end).unwrap_or_default();
        let character = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name
                .strip_prefix("#x")
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| name.strip_prefix('#').map(str::parse))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match character {
            Some(character) => out.push(character),
            None => out.push_str(after.get(..=end).unwrap_or_default()),
        }
        rest = after.get(end + 1..).unwrap_or_default();
    }
    out.push_str(rest);
    out
}
