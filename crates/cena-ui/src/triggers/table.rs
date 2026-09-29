//! A trigger's table in the file, and the editor's [`Form`] of it, each made
//! from the other (`plan/54` steps 2 and 3). Only the keys the form edits
//! cross; the rest of a table (one character's copy, where it came from)
//! the writer keeps. Here, below the binary and the window both, so the
//! window can check a form and run it through the real matcher without
//! asking (`plan/54` step 3), and the two cannot turn a form into different
//! tables.

use cena_model::trigger::Rule;
use toml::{Table, Value};

use super::{Flag, Form, Look, Redirect};

impl Form {
    /// The rule this form makes, checked as the file's would be: what the
    /// matcher runs.
    ///
    /// # Errors
    ///
    /// Why the file would refuse it.
    pub fn rule(&self) -> Result<Rule, String> {
        Value::Table(to_table(self))
            .try_into::<Rule>()
            .map_err(|why| why.message().to_owned())
    }
}

/// The form of trigger `name`'s `table`.
#[must_use]
pub fn from_table(name: &str, table: &Table) -> Form {
    let text = |key: &str| table.get(key).and_then(Value::as_str).map(str::to_owned);
    let on = |key: &str| table.get(key).and_then(Value::as_bool);
    let number = |key: &str| {
        table
            .get(key)
            .and_then(Value::as_integer)
            .and_then(|n| u32::try_from(n).ok())
    };
    let words = |key: &str| match table.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect::<Vec<_>>()
            .join(", "),
        Some(Value::String(one)) => one.clone(),
        _ => String::new(),
    };
    let say = |key: &str| match table.get(key) {
        Some(Value::Boolean(true)) => Some(String::new()),
        Some(Value::String(words)) => Some(words.clone()),
        _ => None,
    };
    let (text_, regex) = match (text("regex"), text("text")) {
        (Some(regex), _) => (regex, true),
        (None, Some(text)) => (text, false),
        (None, None) => (String::new(), false),
    };
    let sub = |key: &str| table.get(key).and_then(Value::as_table);
    Form {
        name: name.to_owned(),
        category: text("category").unwrap_or_default(),
        enabled: on("enabled") != Some(false),
        text: text_,
        regex,
        case_sensitive: on("case_sensitive") == Some(true),
        whole_word: on("whole_word") != Some(false),
        stream: text("stream").unwrap_or_default(),
        event: text("event").unwrap_or_default(),
        condition: words("condition"),
        rearm: number("rearm"),
        only_if: words("only_if"),
        look: sub("look").map(|look| Look {
            color: look
                .get("color")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            background: look
                .get("background")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            bold: look.get("bold").and_then(Value::as_bool) == Some(true),
            span: match look.get("span") {
                Some(Value::String(span)) => span.clone(),
                Some(Value::Integer(group)) => group.to_string(),
                _ => "match".to_owned(),
            },
        }),
        squelch: on("squelch") == Some(true),
        substitute: text("substitute"),
        redirect: sub("redirect").map(|redirect| Redirect {
            stream: redirect
                .get("stream")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            copy: redirect.get("copy").and_then(Value::as_bool) == Some(true),
        }),
        flag: sub("flag").map(|flag| Flag {
            name: flag
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            seconds: flag
                .get("seconds")
                .and_then(Value::as_integer)
                .and_then(|n| u32::try_from(n).ok()),
            clear: flag.get("clear").and_then(Value::as_bool) == Some(true),
        }),
        sound: text("sound"),
        notify: say("notify"),
        alert: say("alert"),
        send: text("send"),
        cooldown: number("cooldown"),
        priority: table
            .get("priority")
            .and_then(Value::as_integer)
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0),
        characters: match table.get("characters") {
            Some(Value::Array(names)) => names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        },
    }
}

/// The table `form` writes: only the keys the form edits, each only when it says
/// something, so the file stays as short as a hand-written one.
#[must_use]
pub fn to_table(form: &Form) -> Table {
    let mut table = Table::new();
    if !form.category.trim().is_empty() {
        put(&mut table, "category", string(form.category.trim()));
    }
    if !form.enabled {
        put(&mut table, "enabled", Value::Boolean(false));
    }
    if !form.text.is_empty() {
        let key = if form.regex { "regex" } else { "text" };
        put(&mut table, key, string(&form.text));
    }
    if form.case_sensitive {
        put(&mut table, "case_sensitive", Value::Boolean(true));
    }
    if !form.whole_word && !form.regex {
        put(&mut table, "whole_word", Value::Boolean(false));
    }
    for (key, words) in [
        ("stream", &form.stream),
        ("event", &form.event),
        ("condition", &form.condition),
        ("only_if", &form.only_if),
    ] {
        if !words.trim().is_empty() {
            put(&mut table, key, string(words.trim()));
        }
    }
    for (key, seconds) in [("rearm", form.rearm), ("cooldown", form.cooldown)] {
        if let Some(seconds) = seconds {
            put(&mut table, key, Value::Integer(i64::from(seconds)));
        }
    }
    if form.priority != 0 {
        put(
            &mut table,
            "priority",
            Value::Integer(i64::from(form.priority)),
        );
    }
    if !form.characters.is_empty() {
        let names = form.characters.iter().map(|name| string(name)).collect();
        put(&mut table, "characters", Value::Array(names));
    }
    responses(form, &mut table);
    table
}

/// What `form` does, into `table`.
fn responses(form: &Form, table: &mut Table) {
    if let Some(look) = &form.look {
        let mut sub = Table::new();
        if !look.color.is_empty() {
            sub.insert("color".to_owned(), string(&look.color));
        }
        if !look.background.is_empty() {
            sub.insert("background".to_owned(), string(&look.background));
        }
        if look.bold {
            sub.insert("bold".to_owned(), Value::Boolean(true));
        }
        match look.span.trim() {
            "" | "match" => {}
            span => {
                let value = span
                    .parse::<i64>()
                    .map_or_else(|_| string(span), Value::Integer);
                sub.insert("span".to_owned(), value);
            }
        }
        put(table, "look", Value::Table(sub));
    }
    if form.squelch {
        put(table, "squelch", Value::Boolean(true));
    }
    if let Some(redirect) = &form.redirect {
        let mut sub = Table::new();
        sub.insert("stream".to_owned(), string(redirect.stream.trim()));
        if redirect.copy {
            sub.insert("copy".to_owned(), Value::Boolean(true));
        }
        put(table, "redirect", Value::Table(sub));
    }
    if let Some(flag) = &form.flag {
        let mut sub = Table::new();
        sub.insert("name".to_owned(), string(flag.name.trim()));
        if let Some(seconds) = flag.seconds {
            sub.insert("seconds".to_owned(), Value::Integer(i64::from(seconds)));
        }
        if flag.clear {
            sub.insert("clear".to_owned(), Value::Boolean(true));
        }
        put(table, "flag", Value::Table(sub));
    }
    // An empty notice or banner says the line itself: `true` in the file.
    let say = |words: &str| {
        if words.trim().is_empty() {
            Value::Boolean(true)
        } else {
            string(words)
        }
    };
    for (key, value) in [
        ("substitute", form.substitute.as_deref().map(string)),
        (
            "sound",
            form.sound.as_deref().map(|sound| string(sound.trim())),
        ),
        ("notify", form.notify.as_deref().map(say)),
        ("alert", form.alert.as_deref().map(say)),
        ("send", form.send.as_deref().map(string)),
    ] {
        if let Some(value) = value {
            put(table, key, value);
        }
    }
}

fn put(table: &mut Table, key: &str, value: Value) {
    table.insert(key.to_owned(), value);
}

fn string(text: &str) -> Value {
    Value::String(text.to_owned())
}

/// Who a trigger is off for, and whose copy changes more than that: from
/// its `for` table.
#[must_use]
pub fn copies(table: &Table) -> (Vec<String>, Vec<String>) {
    let (mut off, mut changed) = (Vec::new(), Vec::new());
    if let Some(Value::Table(copies)) = table.get("for") {
        for (character, copy) in copies {
            let Some(copy) = copy.as_table() else {
                continue;
            };
            if copy.get("enabled").and_then(Value::as_bool) == Some(false) {
                off.push(character.clone());
            }
            if copy.keys().any(|key| key != "enabled") {
                changed.push(character.clone());
            }
        }
    }
    (off, changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field the form edits goes to the file and comes back the same.
    #[test]
    fn a_form_round_trips_through_its_table() {
        let form = Form {
            name: "stunned".to_owned(),
            category: "Combat".to_owned(),
            enabled: false,
            text: r"^You are (\w+)ed".to_owned(),
            regex: true,
            case_sensitive: true,
            whole_word: true,
            stream: "combat".to_owned(),
            event: "attacked".to_owned(),
            condition: String::new(),
            rearm: None,
            only_if: "!hidden".to_owned(),
            look: Some(Look {
                color: "#ff4040".to_owned(),
                background: String::new(),
                bold: true,
                span: "1".to_owned(),
            }),
            squelch: false,
            substitute: Some("[$1]".to_owned()),
            redirect: Some(Redirect {
                stream: "thoughts".to_owned(),
                copy: true,
            }),
            flag: Some(Flag {
                name: "stunned".to_owned(),
                seconds: Some(10),
                clear: false,
            }),
            sound: Some("alarm.wav".to_owned()),
            notify: Some(String::new()),
            alert: Some("Stunned!".to_owned()),
            send: Some("stand".to_owned()),
            cooldown: Some(5),
            priority: 2,
            characters: vec!["Nisugi".to_owned()],
        };
        assert_eq!(from_table("stunned", &to_table(&form)), form);
        let plain = Form {
            name: "plain".to_owned(),
            text: "a rock".to_owned(),
            ..Form::default()
        };
        let table = to_table(&plain);
        assert_eq!(table.len(), 1, "only what it says: {table:?}");
        assert_eq!(from_table("plain", &table), plain);
    }

    #[test]
    fn copies_say_who_it_is_off_for_and_whose_copy_changes_more() {
        let table: Table =
            toml::from_str("[for.Dicate]\nenabled = false\n[for.Nisugi]\nlook = { bold = true }\n")
                .expect("toml");
        assert_eq!(
            copies(&table),
            (vec!["Dicate".to_owned()], vec!["Nisugi".to_owned()])
        );
    }

    #[test]
    fn a_form_makes_the_rule_the_file_would() {
        let form = Form {
            name: "rock".to_owned(),
            text: "a rock".to_owned(),
            squelch: true,
            ..Form::default()
        };
        assert!(form.rule().is_ok_and(|rule| rule.squelch));
        let nothing = Form {
            name: "idle".to_owned(),
            text: "a rock".to_owned(),
            ..Form::default()
        };
        assert!(
            nothing.rule().is_err(),
            "a trigger that does nothing is refused"
        );
    }
}
