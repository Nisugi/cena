//! The trigger import's question (`plan/54` step 5): an import that brings
//! commands its triggers would send asks before anything is written. The
//! author: *"a popup indicating it contains the commands, list the commands,
//! offer accept all, accept one, cancel."*
//!
//! *Accept all* imports and approves every command; *Accept the ticked*
//! approves the ones ticked, the rest imported with their commands held
//! (`;trigger approve` releases one later); *Cancel* imports nothing.

use std::collections::BTreeSet;

use cena_ui::HubRequest;
use cena_ui::triggers::ImportQuestion;

use super::App;

/// What the player answered.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Answer {
    /// Import, approving the commands of these triggers.
    Accept(Vec<String>),
    /// Import nothing.
    Cancel,
}

/// The popup's own state: which commands are ticked, for which import.
#[derive(Debug, Default)]
pub(super) struct Asking {
    /// The import the ticks are for.
    id: Option<u64>,
    /// The triggers whose commands are ticked, by name.
    ticked: BTreeSet<String>,
}

impl App {
    /// The import's question, over the hub, while one waits for an answer.
    pub(super) fn import_question(
        &mut self,
        context: &egui::Context,
        question: Option<&ImportQuestion>,
    ) {
        let Some(question) = question else {
            return;
        };
        if self.asking.id != Some(question.id) {
            self.asking = Asking {
                id: Some(question.id),
                ticked: BTreeSet::new(),
            };
        }
        let mut answer = None;
        egui::Window::new("Import triggers")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                answer = ask(ui, question, &mut self.asking.ticked);
            });
        if let Some(answer) = answer {
            self.sessions.answered_import(question.id);
            self.sessions.ask(HubRequest::ImportAnswer {
                id: question.id,
                accept: match answer {
                    Answer::Accept(names) => Some(names),
                    Answer::Cancel => None,
                },
            });
        }
    }
}

/// The question drawn; the answer once one is given.
fn ask(
    ui: &mut egui::Ui,
    question: &ImportQuestion,
    ticked: &mut BTreeSet<String>,
) -> Option<Answer> {
    ui.label(format!(
        "{} brings {} triggers. These would send commands, as if you typed them:",
        question.file, question.triggers
    ));
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .max_height(260.0)
        .show(ui, |ui| {
            for (name, line) in &question.sends {
                let mut on = ticked.contains(name);
                if ui.checkbox(&mut on, format!("{name}: {line}")).changed() {
                    if on {
                        ticked.insert(name.clone());
                    } else {
                        ticked.remove(name);
                    }
                }
            }
        });
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(
            "A command not accepted is imported held: its trigger still colours and sounds, \
             and sends nothing until you approve it.",
        )
        .weak()
        .small(),
    );
    let mut answer = None;
    ui.horizontal(|ui| {
        if ui.button("Accept all").clicked() {
            answer = Some(Answer::Accept(
                question
                    .sends
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect(),
            ));
        }
        if ui
            .add_enabled(!ticked.is_empty(), egui::Button::new("Accept the ticked"))
            .clicked()
        {
            answer = Some(Answer::Accept(ticked.iter().cloned().collect()));
        }
        if ui.button("Cancel").clicked() {
            answer = Some(Answer::Cancel);
        }
    });
    answer
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;

    fn question() -> ImportQuestion {
        ImportQuestion {
            id: 7,
            file: "Maravel.toml".to_owned(),
            triggers: 3,
            sends: vec![
                ("stunned".to_owned(), "stand".to_owned()),
                ("webbed".to_owned(), "stance defensive".to_owned()),
            ],
        }
    }

    struct Scene {
        ticked: BTreeSet<String>,
        answers: Vec<Answer>,
    }

    fn harness<'a>() -> Harness<'a, Scene> {
        Harness::builder().with_size((520.0, 420.0)).build_ui_state(
            |ui, scene: &mut Scene| {
                if let Some(answer) = ask(ui, &question(), &mut scene.ticked) {
                    scene.answers.push(answer);
                }
            },
            Scene {
                ticked: BTreeSet::new(),
                answers: Vec::new(),
            },
        )
    }

    #[test]
    fn the_commands_are_listed_and_each_answer_says_what_it_accepts() {
        let mut harness = harness();
        assert!(harness.query_by_label("webbed: stance defensive").is_some());
        harness.get_by_label("Accept all").click();
        harness.run();
        harness.get_by_label("webbed: stance defensive").click();
        harness.run();
        harness.get_by_label("Accept the ticked").click();
        harness.run();
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(
            harness.state().answers,
            [
                Answer::Accept(vec!["stunned".to_owned(), "webbed".to_owned()]),
                Answer::Accept(vec!["webbed".to_owned()]),
                Answer::Cancel,
            ]
        );
    }
}
