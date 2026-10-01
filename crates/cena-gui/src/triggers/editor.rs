//! The trigger editor's state, and its list and chosen trigger drawn.

use std::collections::BTreeSet;

use cena_ui::triggers::{Book, Change, Entry, Form, Switch};

use super::form::{self, Watch};

/// The trigger editor.
#[derive(Debug, Default)]
pub(crate) struct Editor {
    /// Whether it is showing.
    pub(crate) open: bool,
    /// What the list is narrowed to: words in a name, a category or what a
    /// trigger does.
    pub(super) search: String,
    /// The trigger being edited on the right: a saved one, or a new one.
    draft: Option<Draft>,
    /// What the editor had to say itself: why a click did nothing.
    notice: Option<String>,
    /// A trigger the player asked to remove, waiting on *Remove* again.
    removing: Option<String>,
    /// Categories folded shut.
    folded: BTreeSet<String>,
    /// The live test's line (`plan/54` step 3).
    test: super::test::Test,
    /// A trigger to start editing once the file is read: a story line's.
    pending: Option<Form>,
    /// *What would it have caught?*: whose log, how many days, what is
    /// asked and what came back (`plan/54` step 4).
    catch: Catching,
}

/// The editor's *What would it have caught?*.
#[derive(Debug, Default)]
struct Catching {
    /// Whose log; the first roster character until one is chosen.
    character: String,
    /// How many days back.
    days: usize,
    /// A check to hand to the window, once asked.
    asked: Option<super::catch::Ask>,
    /// Where its answer lands.
    inbox: super::catch::Inbox,
    /// A check was asked and has not answered.
    waiting: bool,
    /// What the last check found.
    caught: Option<super::catch::Caught>,
}

/// A trigger being edited.
#[derive(Debug, Clone)]
struct Draft {
    /// Its name in the file, or `None` while it is new.
    was: Option<String>,
    /// For a new trigger made by *Duplicate*, the trigger it copies, whose
    /// origin and held send the copy keeps (GU-D-6).
    copy_of: Option<String>,
    /// What the form holds.
    form: Form,
    /// What it watches, as the form's radio says.
    watch: Watch,
}

impl Draft {
    fn of(was: Option<String>, form: Form) -> Self {
        Self {
            watch: Watch::of(&form),
            was,
            copy_of: None,
            form,
        }
    }

    /// Whether the file holds this draft as it is: a trigger of its name
    /// with exactly its form. A save the file refused stays unsaved.
    fn saved_in(&self, book: &Book) -> bool {
        book.triggers
            .iter()
            .any(|entry| entry.name == self.form.name && entry.form == self.form)
    }
}

/// The form made ready to save: names trimmed, what a regex cannot have
/// dropped, and a condition without the line responses it cannot have
/// (`plan/45` §6b), so what is saved reads back as the form shows it.
fn tidy(form: &mut Form) {
    for words in [
        &mut form.name,
        &mut form.category,
        &mut form.stream,
        &mut form.event,
        &mut form.condition,
        &mut form.only_if,
    ] {
        *words = words.trim().to_owned();
    }
    if form.regex {
        form.whole_word = true;
    }
    if !form.condition.is_empty() {
        form.look = None;
        form.squelch = false;
        form.substitute = None;
        form.redirect = None;
    }
    if let Some(look) = &mut form.look
        && look.span.trim().is_empty()
    {
        "match".clone_into(&mut look.span);
    }
    if let Some(flag) = &mut form.flag {
        flag.name = flag.name.trim().to_owned();
    }
    if let Some(redirect) = &mut form.redirect {
        redirect.stream = redirect.stream.trim().to_owned();
    }
    // Only spaces says the line itself, as the file writes it.
    for words in [&mut form.notify, &mut form.alert].into_iter().flatten() {
        if words.trim().is_empty() {
            words.clear();
        }
    }
    if let Some(sound) = &mut form.sound {
        *sound = sound.trim().to_owned();
    }
}

impl Editor {
    /// The form being edited, for a test to fill in as a player would type.
    #[cfg(test)]
    pub(super) fn draft_form(&mut self) -> Option<&mut Form> {
        self.draft.as_mut().map(|draft| &mut draft.form)
    }

    /// A check the player asked for, taken, for the window to run with its
    /// answer's inbox.
    pub(crate) fn take_check(&mut self) -> Option<(super::catch::Ask, super::catch::Inbox)> {
        let ask = self.catch.asked.take()?;
        self.catch.waiting = true;
        Some((ask, std::sync::Arc::clone(&self.catch.inbox)))
    }

    /// Start a new trigger on `line`'s words, from a story line's
    /// right-click (`plan/54` step 4): the words, bold, named for them, and
    /// the line in the live test, so what it does is seen at once.
    pub(crate) fn start_from(&mut self, line: &str) {
        let words = line.trim();
        let name: String = words.chars().take(40).collect();
        self.pending = Some(Form {
            name: name.trim().to_owned(),
            text: words.to_owned(),
            look: Some(cena_ui::triggers::Look {
                bold: true,
                span: "match".to_owned(),
                ..cena_ui::triggers::Look::default()
            }),
            ..Form::default()
        });
        words.clone_into(&mut self.test.line);
    }

    /// Open it, and whether it was shut: a window opening asks for the file.
    pub(crate) fn open(&mut self) -> bool {
        !std::mem::replace(&mut self.open, true)
    }

    /// Draw it over `book` (`None` until the binary has answered), with the
    /// binary's last answer, for the roster's `characters`; what the player
    /// asked to change.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        book: Option<&Book>,
        said: Option<&str>,
        characters: &[String],
    ) -> Vec<Change> {
        let mut asked = Vec::new();
        let Some(book) = book else {
            ui.spinner();
            ui.label("Reading the triggers file...");
            return asked;
        };
        if let Some(form) = self.pending.take() {
            self.edit(None, form, book);
        }
        // A save that has landed: the draft is that trigger now.
        if let Some(draft) = &mut self.draft
            && draft.saved_in(book)
        {
            draft.was = Some(draft.form.name.clone());
        }
        ui.horizontal(|ui| {
            if ui.button("+ New").clicked() {
                self.edit(None, Form::default(), book);
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search names, categories, what they do")
                    .desired_width(280.0),
            );
            ui.separator();
            ui.label("Everywhere:");
            for (kind, on) in &book.kinds {
                let mut now = *on;
                if ui
                    .checkbox(&mut now, kind.as_str())
                    .on_hover_text(format!("Every trigger's {kind}, on or off"))
                    .changed()
                {
                    asked.push(Change::Switch(Switch::Every(kind.clone()), now));
                }
            }
        });
        if let Some(problem) = &book.problem {
            ui.colored_label(ui.visuals().warn_fg_color, problem.as_str());
        }
        if let Some(said) = said {
            ui.label(said);
        }
        if let Some(notice) = &self.notice {
            ui.colored_label(ui.visuals().warn_fg_color, notice.as_str());
        }
        ui.separator();
        // Room kept below for the live test.
        let height = (ui.available_height() - 120.0).max(160.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(380.0);
                egui::ScrollArea::vertical()
                    .id_salt("trigger-list")
                    .max_height(height)
                    .auto_shrink(false)
                    .show(ui, |ui| self.list(ui, book, &mut asked));
            });
            ui.separator();
            ui.vertical(|ui| {
                egui::ScrollArea::vertical()
                    .id_salt("trigger-form")
                    .max_height(height)
                    .auto_shrink(false)
                    .show(ui, |ui| self.chosen(ui, book, characters, &mut asked));
            });
        });
        ui.separator();
        let draft = self
            .draft
            .as_ref()
            .map(|draft| (draft.was.as_deref(), &draft.form));
        super::test::show(ui, &mut self.test, book, draft);
        asked
    }

    /// Every trigger the search keeps, by category, each with its switch.
    fn list(&mut self, ui: &mut egui::Ui, book: &Book, asked: &mut Vec<Change>) {
        if book.triggers.is_empty() {
            ui.label("No triggers yet.");
            return;
        }
        let search = self.search.to_lowercase();
        let kept = |entry: &Entry| {
            search.is_empty()
                || [&entry.name, &entry.category, &entry.summary]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&search))
        };
        let mut at = 0;
        while let Some(first) = book.triggers.get(at) {
            let category = first.category.clone();
            let group: Vec<&Entry> = book.triggers[at..]
                .iter()
                .take_while(|entry| entry.category == category)
                .collect();
            at += group.len();
            let shown: Vec<&Entry> = group.iter().copied().filter(|e| kept(e)).collect();
            if shown.is_empty() {
                continue;
            }
            self.category(ui, book, &category, group.len(), asked);
            if self.folded.contains(&category) && search.is_empty() {
                continue;
            }
            for entry in shown {
                self.row(ui, book, entry, asked);
            }
        }
    }

    /// A category's heading: fold it, count it, switch it.
    fn category(
        &mut self,
        ui: &mut egui::Ui,
        book: &Book,
        category: &str,
        count: usize,
        asked: &mut Vec<Change>,
    ) {
        ui.horizontal(|ui| {
            let folded = self.folded.contains(category);
            if ui.small_button(if folded { "▸" } else { "▾" }).clicked() {
                if folded {
                    self.folded.remove(category);
                } else {
                    self.folded.insert(category.to_owned());
                }
            }
            let shown = if category.is_empty() {
                "(no category)"
            } else {
                category
            };
            ui.strong(format!("{shown} ({count})"));
            if !category.is_empty() {
                let on = book
                    .categories
                    .iter()
                    .find(|(name, _)| name == category)
                    .is_none_or(|(_, on)| *on);
                let mut now = on;
                if ui
                    .checkbox(&mut now, "on")
                    .on_hover_text("Every trigger in this category, on or off")
                    .changed()
                {
                    asked.push(Change::Switch(Switch::Category(category.to_owned()), now));
                }
            }
        });
    }

    /// One trigger: its switch, its name, what it does, and what is wrong.
    fn row(&mut self, ui: &mut egui::Ui, book: &Book, entry: &Entry, asked: &mut Vec<Change>) {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let mut on = entry.enabled;
            if ui.checkbox(&mut on, "").changed() {
                asked.push(Change::Switch(Switch::Trigger(entry.name.clone()), on));
            }
            let chosen = self.draft.as_ref().and_then(|draft| draft.was.as_deref())
                == Some(entry.name.as_str());
            if ui.selectable_label(chosen, entry.name.as_str()).clicked() && !chosen {
                self.edit(Some(entry.name.clone()), entry.form.clone(), book);
            }
            if entry.refused.is_some() {
                ui.colored_label(ui.visuals().error_fg_color, "refused");
            } else if entry.held.is_some() {
                ui.colored_label(ui.visuals().warn_fg_color, "send waits");
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new(&entry.summary).weak().small());
        });
    }

    /// Start editing `form` (the trigger `was`, or a new one), unless the
    /// draft on the right has changes not saved: those are not thrown away
    /// by a click.
    fn edit(&mut self, was: Option<String>, form: Form, book: &Book) {
        if let Some(draft) = &self.draft
            && !draft.saved_in(book)
            && !self.pristine(book)
        {
            self.notice = Some(format!(
                "`{}` has changes not saved: save or revert them first.",
                draft.form.name
            ));
            return;
        }
        self.notice = None;
        self.removing = None;
        self.draft = Some(Draft::of(was, form));
    }

    /// Whether the draft is as the file has it, or new and untouched.
    fn pristine(&self, book: &Book) -> bool {
        self.draft.as_ref().is_none_or(|draft| match &draft.was {
            Some(was) => book
                .triggers
                .iter()
                .any(|entry| entry.name == *was && entry.form == draft.form),
            None => draft.form == Form::default(),
        })
    }

    /// The draft: what the file says of it, the form, and what can be done.
    fn chosen(
        &mut self,
        ui: &mut egui::Ui,
        book: &Book,
        characters: &[String],
        asked: &mut Vec<Change>,
    ) {
        let pristine = self.pristine(book);
        let Some(draft) = &mut self.draft else {
            ui.label("Choose a trigger on the left, or + New.");
            return;
        };
        let entry = draft
            .was
            .as_ref()
            .and_then(|was| book.triggers.iter().find(|entry| entry.name == *was));
        // A copy answers to the trigger it copies for where it came from and
        // its held send, as the writer saves it.
        let original = draft
            .copy_of
            .as_ref()
            .and_then(|of| book.triggers.iter().find(|entry| entry.name == *of));
        match entry {
            Some(entry) => {
                ui.heading(entry.name.as_str());
                status(ui, entry, pristine, asked);
            }
            None => {
                ui.heading("A new trigger");
                if let Some(original) = original {
                    copied(ui, original, &draft.form);
                }
            }
        }
        form::show(ui, &mut draft.form, &mut draft.watch, book, characters);
        if let Some(entry) = entry {
            off_for(ui, entry, characters, asked);
        }
        ui.separator();
        let send_changed = draft.form.send.is_some()
            && entry
                .or(original)
                .is_some_and(|entry| entry.held.is_some() && entry.form.send != draft.form.send);
        if send_changed {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "You changed its send: saving approves the command as you wrote it.",
            );
        }
        ui.horizontal(|ui| {
            let can_save = !pristine && !draft.form.name.trim().is_empty();
            if ui
                .add_enabled(can_save, egui::Button::new("Save"))
                .clicked()
            {
                tidy(&mut draft.form);
                asked.push(Change::Save {
                    was: draft.was.clone(),
                    form: Box::new(draft.form.clone()),
                    copy_of: draft.copy_of.clone(),
                });
            }
            if let Some(entry) = entry {
                if ui
                    .add_enabled(!pristine, egui::Button::new("Revert"))
                    .clicked()
                {
                    *draft = Draft::of(draft.was.clone(), entry.form.clone());
                }
                if ui.button("Duplicate").clicked() {
                    let mut copy = draft.form.clone();
                    copy.name = format!("{} copy", copy.name);
                    *draft = Draft {
                        copy_of: Some(entry.name.clone()),
                        ..Draft::of(None, copy)
                    };
                }
            }
        });
        catch(ui, &mut self.catch, &draft.form, characters);
        let Some(entry) = entry else { return };
        let name = entry.name.clone();
        ui.add_space(8.0);
        if self.removing.as_deref() == Some(name.as_str()) {
            ui.horizontal(|ui| {
                ui.label(format!("Remove `{name}` from the file?"));
                if ui.button("Remove").clicked() {
                    asked.push(Change::Remove(name.clone()));
                    self.removing = None;
                    self.draft = None;
                }
                if ui.button("Keep it").clicked() {
                    self.removing = None;
                }
            });
        } else if ui.button("Remove...").clicked() {
            self.removing = Some(name);
        }
    }
}

/// What the file says of a saved trigger: refused, a send waiting, where it
/// came from.
fn status(ui: &mut egui::Ui, entry: &Entry, pristine: bool, asked: &mut Vec<Change>) {
    if let Some(origin) = &entry.origin {
        ui.label(egui::RichText::new(format!("From {origin}")).weak());
    }
    if let Some(why) = &entry.refused {
        ui.colored_label(
            ui.visuals().error_fg_color,
            format!("Refused, so it does nothing: {why}"),
        );
    }
    if let Some(line) = &entry.held {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!("It came from elsewhere, and sends \"{line}\" only once you approve it."),
        );
        if pristine && ui.button("Approve this command").clicked() {
            asked.push(Change::Approve(entry.name.clone()));
        }
    }
}

/// What a copy made by *Duplicate* keeps of `original`: where it came from,
/// and its held send while `form` leaves the send as it was (GU-D-6).
fn copied(ui: &mut egui::Ui, original: &Entry, form: &Form) {
    let from = original
        .origin
        .as_ref()
        .map_or_else(String::new, |origin| format!(", from {origin}"));
    ui.label(egui::RichText::new(format!("A copy of `{}`{from}", original.name)).weak());
    if let Some(line) = &original.held
        && form.send.as_deref() == Some(line.as_str())
    {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!("The copy sends \"{line}\" only once you approve it, as the original does."),
        );
    }
}

/// Switched off for one character or another (`plan/54` §1 row 4): each
/// box a change at once; a copy that changes more is named, and left to the
/// file.
fn off_for(ui: &mut egui::Ui, entry: &Entry, characters: &[String], asked: &mut Vec<Change>) {
    let mut names: Vec<String> = characters.to_vec();
    for named in &entry.off_for {
        if !names.iter().any(|name| name.eq_ignore_ascii_case(named)) {
            names.push(named.clone());
        }
    }
    if names.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Off for:");
        for name in names {
            let off = entry.off_for.iter().any(|o| o.eq_ignore_ascii_case(&name));
            let mut now = off;
            if ui.checkbox(&mut now, name.as_str()).changed() {
                asked.push(Change::OffFor {
                    name: entry.name.clone(),
                    character: name,
                    off: now,
                });
            }
        }
    });
    if !entry.changed_for.is_empty() {
        ui.label(
            egui::RichText::new(format!(
                "Changed further for {}: kept in the file, and ;trigger shows it.",
                entry.changed_for.join(", ")
            ))
            .weak()
            .small(),
        );
    }
}

/// *What would it have caught?*: the form over a character's log, the lines
/// it matches.
fn catch(ui: &mut egui::Ui, catch: &mut Catching, form: &Form, characters: &[String]) {
    if let Some(caught) = crate::sessions::lock(&catch.inbox).take() {
        catch.waiting = false;
        catch.caught = Some(caught);
    }
    if catch.character.is_empty()
        && let Some(first) = characters.first()
    {
        catch.character.clone_from(first);
    }
    if catch.days == 0 {
        catch.days = 7;
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("What would it have caught in");
        egui::ComboBox::from_id_salt("trigger-catch-who")
            .selected_text(catch.character.as_str())
            .show_ui(ui, |ui| {
                for name in characters {
                    ui.selectable_value(&mut catch.character, name.clone(), name);
                }
            });
        ui.label("'s log over");
        egui::ComboBox::from_id_salt("trigger-catch-days")
            .selected_text(match catch.days {
                1 => "the last day".to_owned(),
                days => format!("the last {days} days"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut catch.days, 1, "the last day");
                ui.selectable_value(&mut catch.days, 7, "the last 7 days");
                ui.selectable_value(&mut catch.days, 30, "the last 30 days");
            });
        let can = !catch.character.is_empty() && !catch.waiting;
        if ui.add_enabled(can, egui::Button::new("Check")).clicked() {
            catch.asked = Some(super::catch::Ask {
                character: catch.character.clone(),
                days: catch.days,
                form: form.clone(),
            });
        }
        if catch.waiting {
            ui.spinner();
        }
    });
    let Some(caught) = &catch.caught else { return };
    if let Some(why) = &caught.why {
        ui.colored_label(ui.visuals().error_fg_color, why.as_str());
        return;
    }
    let more = if caught.more {
        "; more past the first 1000"
    } else {
        ""
    };
    ui.label(format!(
        "{} lines in {} days{more}, newest first:",
        caught.lines.len(),
        caught.days
    ));
    egui::ScrollArea::vertical()
        .id_salt("trigger-caught")
        .max_height(180.0)
        .show(ui, |ui| {
            for entry in caught.lines.iter().take(200) {
                let at = entry.at.get(..8).unwrap_or(&entry.at);
                ui.label(
                    egui::RichText::new(format!(
                        "{} {at} [{}] {}",
                        entry.day, entry.stream, entry.text
                    ))
                    .monospace()
                    .small(),
                );
            }
        });
}
