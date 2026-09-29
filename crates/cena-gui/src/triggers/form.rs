//! The trigger editor's form (`plan/54` step 2): when, do, and for whom.
//!
//! It edits a [`Form`] and nothing else; the window saves it as a change.
//! A condition watches the character, not a line, so the line's responses
//! (look, squelch, substitute, redirect) are greyed for one (`plan/45` §6b).

use cena_ui::triggers::{Book, Flag, Form, Look, Redirect};

/// What the trigger watches: a line (its words, an event, or both), or the
/// character (a condition).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Watch {
    /// A line: words, an event, or both.
    Line,
    /// Guard words becoming true.
    Condition,
}

impl Watch {
    /// What `form` watches, as the file says.
    pub(super) fn of(form: &Form) -> Self {
        if form.condition.trim().is_empty() {
            Self::Line
        } else {
            Self::Condition
        }
    }
}

/// Draw the form for `form`, offering what `book` knows, for `characters`.
pub(super) fn show(
    ui: &mut egui::Ui,
    form: &mut Form,
    watch: &mut Watch,
    book: &Book,
    characters: &[String],
) {
    egui::Grid::new("trigger-name")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut form.name);
            ui.end_row();
            ui.label("Category");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut form.category).desired_width(160.0));
                egui::ComboBox::from_id_salt("trigger-category")
                    .selected_text("…")
                    .show_ui(ui, |ui| {
                        for (category, _) in &book.categories {
                            ui.selectable_value(&mut form.category, category.clone(), category);
                        }
                    });
            });
            ui.end_row();
            ui.label("On");
            ui.checkbox(&mut form.enabled, "for everyone it is for");
            ui.end_row();
        });
    heading(ui, "When");
    when(ui, form, watch, book);
    heading(ui, "Do");
    does(ui, form, *watch, book);
    heading(ui, "For whom");
    whom(ui, form, characters);
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.strong(text);
    ui.separator();
}

/// What it watches.
fn when(ui: &mut egui::Ui, form: &mut Form, watch: &mut Watch, book: &Book) {
    ui.horizontal(|ui| {
        ui.radio_value(watch, Watch::Line, "A line")
            .on_hover_text("Words in a line, what the line is (an event), or both");
        ui.radio_value(watch, Watch::Condition, "A condition")
            .on_hover_text("Guard words about the character becoming true");
    });
    match watch {
        Watch::Line => {
            form.condition.clear();
            form.rearm = None;
            ui.horizontal(|ui| {
                ui.label("Words");
                ui.add(
                    egui::TextEdit::singleline(&mut form.text)
                        .hint_text("what to look for; empty for any line of the event")
                        .desired_width(320.0),
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut form.regex, "Regex");
                ui.checkbox(&mut form.case_sensitive, "Match case");
                ui.add_enabled(
                    !form.regex,
                    egui::Checkbox::new(&mut form.whole_word, "Whole words"),
                )
                .on_hover_text("A regex says its own edges with \\b");
            });
            if form.regex
                && let Err(why) = regex_ok(&form.text)
            {
                ui.colored_label(ui.visuals().error_fg_color, why);
            }
            ui.horizontal(|ui| {
                ui.label("Event");
                pick(ui, "trigger-event", &mut form.event, &book.events, "none");
                ui.label("Stream");
                ui.add(
                    egui::TextEdit::singleline(&mut form.stream)
                        .hint_text("every stream")
                        .desired_width(120.0),
                );
            });
        }
        Watch::Condition => {
            form.text.clear();
            form.event.clear();
            words(
                ui,
                "Becomes true",
                "trigger-condition",
                &mut form.condition,
                book,
            );
            ui.horizontal(|ui| {
                ui.label("Again only after");
                seconds(ui, &mut form.rearm, 3);
                ui.label("false");
            });
        }
    }
    words(ui, "Only if", "trigger-only-if", &mut form.only_if, book);
}

/// A line of guard words, with the vocabulary to add from.
fn words(ui: &mut egui::Ui, label: &str, id: &str, words: &mut String, book: &Book) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::TextEdit::singleline(words)
                .hint_text("guard words, all of them; ! for not")
                .desired_width(280.0),
        );
        egui::ComboBox::from_id_salt(id)
            .selected_text("add a word")
            .show_ui(ui, |ui| {
                for word in &book.guard_words {
                    if ui.selectable_label(false, word).clicked() {
                        if !words.trim().is_empty() {
                            words.push_str(", ");
                        }
                        words.push_str(word);
                    }
                }
            });
    });
}

/// What it does. The line's responses are greyed for a condition.
fn does(ui: &mut egui::Ui, form: &mut Form, watch: Watch, book: &Book) {
    let line = watch == Watch::Line;
    ui.add_enabled_ui(line, |ui| {
        if !line {
            ui.label(
                egui::RichText::new("A condition has no line to colour, hide, change or move.")
                    .weak()
                    .small(),
            );
        }
        look(ui, &mut form.look, form.regex);
        ui.checkbox(&mut form.squelch, "Squelch: hide the line");
        optional(ui, "Substitute", &mut form.substitute, |ui, words| {
            ui.add(
                egui::TextEdit::singleline(words).hint_text("in place of the match; $1 is a group"),
            );
        });
        optional(
            ui,
            "Redirect",
            &mut form.redirect,
            |ui, redirect: &mut Redirect| {
                ui.label("to");
                ui.add(egui::TextEdit::singleline(&mut redirect.stream).desired_width(120.0));
                ui.checkbox(&mut redirect.copy, "and keep it here");
            },
        );
    });
    optional(ui, "Flag", &mut form.flag, |ui, flag: &mut Flag| {
        ui.add(
            egui::TextEdit::singleline(&mut flag.name)
                .hint_text("its name")
                .desired_width(120.0),
        );
        ui.checkbox(&mut flag.clear, "clear it");
        if !flag.clear {
            ui.label("for");
            seconds(ui, &mut flag.seconds, 60);
        }
    });
    optional(ui, "Sound", &mut form.sound, |ui, sound| {
        ui.add(
            egui::TextEdit::singleline(sound)
                .hint_text("a file in the sounds folder, or a path")
                .desired_width(220.0),
        );
        pick(ui, "trigger-sound", sound, &book.sounds, "choose");
    });
    optional(ui, "Notify", &mut form.notify, |ui, words| {
        ui.add(egui::TextEdit::singleline(words).hint_text("empty says the line"));
    });
    optional(ui, "Alert", &mut form.alert, |ui, words| {
        ui.add(egui::TextEdit::singleline(words).hint_text("a banner; empty shows the line"));
    });
    optional(ui, "Send", &mut form.send, |ui, line| {
        ui.add(egui::TextEdit::singleline(line).hint_text("a command, as if typed; $1 is a group"));
    });
    ui.horizontal(|ui| {
        ui.label("Again only after");
        seconds(ui, &mut form.cooldown, 3);
        ui.label("Priority");
        ui.add(egui::DragValue::new(&mut form.priority).range(-100..=100))
            .on_hover_text("Higher goes first when two looks overlap");
    });
}

/// The look: a colour, a background, bold, and what it covers.
fn look(ui: &mut egui::Ui, look: &mut Option<Look>, regex: bool) {
    optional(ui, "Look", look, |ui, look: &mut Look| {
        colour(ui, "colour", &mut look.color);
        colour(ui, "background", &mut look.background);
        ui.checkbox(&mut look.bold, "bold");
        let mut spans = vec!["match".to_owned(), "line".to_owned()];
        if regex {
            spans.extend((1..=9).map(|group| group.to_string()));
        }
        ui.label("on the");
        pick(ui, "trigger-span", &mut look.span, &spans, "match");
    });
}

/// A colour, `#rrggbb`, or none: a box to have one, then its picker.
fn colour(ui: &mut egui::Ui, label: &str, hex: &mut String) {
    let mut on = !hex.is_empty();
    if ui.checkbox(&mut on, label).changed() {
        *hex = if on {
            "#ffffff".to_owned()
        } else {
            String::new()
        };
    }
    if on {
        let mut rgb = parse_hex(hex).unwrap_or([255, 255, 255]);
        if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
            *hex = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
        }
    }
}

/// `#rrggbb` as its three bytes.
pub(super) fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Who it is for: everyone, or the characters ticked.
fn whom(ui: &mut egui::Ui, form: &mut Form, characters: &[String]) {
    let mut only = !form.characters.is_empty();
    ui.horizontal(|ui| {
        if ui.radio_value(&mut only, false, "Everyone").clicked() {
            form.characters.clear();
        }
        ui.radio_value(&mut only, true, "Only these");
    });
    if !only {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        let mut names: Vec<String> = characters.to_vec();
        for named in &form.characters {
            if !names.iter().any(|name| name.eq_ignore_ascii_case(named)) {
                names.push(named.clone());
            }
        }
        for name in names {
            let mut on = form
                .characters
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&name));
            if ui.checkbox(&mut on, name.as_str()).changed() {
                if on {
                    form.characters.push(name);
                } else {
                    form.characters.retain(|c| !c.eq_ignore_ascii_case(&name));
                }
            }
        }
    });
    if form.characters.is_empty() {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "Tick at least one, or it is for everyone.",
        );
    }
}

/// A response the trigger may have: a box to have it, then its fields.
fn optional<T: Default>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<T>,
    fields: impl FnOnce(&mut egui::Ui, &mut T),
) {
    ui.horizontal(|ui| {
        let mut on = value.is_some();
        if ui.checkbox(&mut on, label).changed() {
            *value = on.then(T::default);
        }
        if let Some(value) = value {
            fields(ui, value);
        }
    });
}

/// A number of seconds, or the default when unticked.
fn seconds(ui: &mut egui::Ui, value: &mut Option<u32>, default: u32) {
    let mut set = value.is_some();
    if ui.checkbox(&mut set, "").changed() {
        *value = set.then_some(default);
    }
    match value {
        Some(seconds) => {
            ui.add(egui::DragValue::new(seconds).range(0..=86_400).suffix(" s"));
        }
        None => {
            ui.label(egui::RichText::new(format!("{default} s, the default")).weak());
        }
    }
}

/// A word from a list, or none.
fn pick(ui: &mut egui::Ui, id: &str, value: &mut String, choices: &[String], none: &str) {
    let shown = if value.is_empty() {
        none
    } else {
        value.as_str()
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), none);
            for choice in choices {
                ui.selectable_value(value, choice.clone(), choice);
            }
        });
}

/// Whether `expression` is a regex the matcher takes, and why not.
fn regex_ok(expression: &str) -> Result<(), String> {
    use cena_session::trigger::Rule;
    let text = format!("regex = {}\nsquelch = true\n", toml_string(expression));
    toml::from_str::<Rule>(&text)
        .map(drop)
        .map_err(|why| why.message().to_owned())
}

/// `text` as a TOML string, quoted and escaped.
fn toml_string(text: &str) -> String {
    toml::Value::String(text.to_owned()).to_string()
}
