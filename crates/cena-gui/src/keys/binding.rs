//! What a key does (`plan/52` §2): a **macro** sends commands to the game,
//! fills the command input, or performs one of Hydra's actions. The author:
//! *"a macro is a keybind to send one or more commands ... send to game,
//! fill command input, or perform hydra action."*
//!
//! A send macro's text is cut into commands as `VellumFE` cuts it
//! (`reference/VellumFE/src/core/app_core/commands.rs:180-238`): at each
//! `\r`, each piece one command, empty pieces dropped, and a piece that is
//! `s` and a number of seconds (`s1.5`) a wait before the rest. Each command
//! is one line, as the crate review's R10 asked of every key
//! (`cena_ui::validate_line`). Unlike Vellum, a trailing `\r` means nothing:
//! a macro's kind, not its last character, says whether it is sent.

use std::time::Duration;

/// The longest a wait in a macro may be, so a slip of the finger (`s600`)
/// does not hold a character's commands for ten minutes.
pub(crate) const LONGEST_WAIT: Duration = Duration::from_mins(1);

/// What a key does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Macro {
    /// Send these commands to the game, as if typed: `\r` between two, and
    /// `s1.5` for a wait.
    Send(String),
    /// Put this in the command input, not sent.
    Fill(String),
    /// Perform one of Hydra's actions.
    Act(Action),
}

/// One of Hydra's actions a key performs (`plan/52` §3). Each step of the
/// plan adds its own, so none is named before it does something.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    /// Stop the running behaviors, as the top bar's Stop does (`;stop`).
    Stop,
    /// Open the settings menu.
    Settings,
}

impl Action {
    /// Every action, in the order the Keys page lists them.
    pub const ALL: [Self; 2] = [Self::Stop, Self::Settings];

    /// Its name in the keybinds file.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Settings => "settings",
        }
    }

    /// Its name for a player.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Stop => "Stop the behaviors",
            Self::Settings => "Open the settings",
        }
    }

    /// The action named `name` in the keybinds file.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// One step of a send macro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// A command sent.
    Line(String),
    /// A wait before the rest.
    Wait(Duration),
}

impl Macro {
    /// A send macro's commands and waits, in order; any other kind, none.
    ///
    /// # Errors
    ///
    /// Why it cannot be sent, in words for the player: a command that is not
    /// one line, a wait too long, or nothing to send.
    pub(crate) fn steps(&self) -> Result<Vec<Step>, String> {
        let Self::Send(text) = self else {
            return Ok(Vec::new());
        };
        let mut steps = Vec::new();
        for piece in text.split(['\r', '\n']).map(str::trim) {
            if piece.is_empty() {
                continue;
            }
            if let Some(wait) = wait(piece) {
                steps.push(Step::Wait(wait?));
                continue;
            }
            cena_ui::validate_line(piece).map_err(|why| format!("`{piece}`: {why}"))?;
            steps.push(Step::Line(piece.to_owned()));
        }
        if !steps.iter().any(|step| matches!(step, Step::Line(_))) {
            return Err("it sends no command".to_owned());
        }
        Ok(steps)
    }

    /// Whether it can be done as written.
    ///
    /// # Errors
    ///
    /// Why not, in words for the player.
    pub(crate) fn check(&self) -> Result<(), String> {
        match self {
            Self::Send(_) => self.steps().map(|_| ()),
            Self::Fill(text) => cena_ui::validate_line(text).map_err(|why| why.to_string()),
            Self::Act(_) => Ok(()),
        }
    }

    /// What it does, for a player: that it sends, fills or does what.
    #[must_use]
    pub fn said(&self) -> String {
        match self {
            Self::Send(text) => format!("sends `{}`", shown(text)),
            Self::Fill(text) => format!("fills the input with `{text}`"),
            Self::Act(action) => format!("does {}", action.label()),
        }
    }

    /// It as the keybinds file writes it, a TOML value: a send macro a
    /// string, the others an inline table.
    pub(crate) fn written(&self) -> String {
        let quoted = |text: &str| toml::Value::String(text.to_owned()).to_string();
        match self {
            Self::Send(text) => quoted(text),
            Self::Fill(text) => format!("{{ fill = {} }}", quoted(text)),
            Self::Act(action) => format!("{{ action = {} }}", quoted(action.name())),
        }
    }

    /// The macro a keybinds file's value is: `Ok(None)` for `""`, which
    /// unbinds a key Hydra binds.
    ///
    /// # Errors
    ///
    /// What is wrong with it, in words for the player.
    pub(crate) fn read(value: &toml::Value) -> Result<Option<Self>, String> {
        let made = match value {
            toml::Value::String(text) if text.trim().is_empty() => return Ok(None),
            toml::Value::String(text) => Self::Send(text.clone()),
            toml::Value::Table(table) if table.len() == 1 => {
                match table
                    .iter()
                    .next()
                    .map(|(kind, value)| (kind.as_str(), value))
                {
                    Some(("fill", toml::Value::String(text))) => Self::Fill(text.clone()),
                    Some(("action", toml::Value::String(name))) => Self::Act(
                        Action::named(name)
                            .ok_or_else(|| format!("`{name}` is not an action Hydra has"))?,
                    ),
                    _ => return Err(KINDS.to_owned()),
                }
            }
            _ => return Err(KINDS.to_owned()),
        };
        made.check()?;
        Ok(Some(made))
    }
}

/// What a keybinds file's value may be.
const KINDS: &str = "a key is bound to \"commands\", { fill = \"text\" } or { action = \"name\" }";

/// A send macro's text as the Keys page shows it and a player types it:
/// each `\r` written as the two characters `\r`.
pub(crate) fn shown(text: &str) -> String {
    text.replace('\r', "\\r")
}

/// Text a player typed on the Keys page, each `\r` they wrote as two
/// characters made the one.
pub(crate) fn typed(text: &str) -> String {
    text.replace("\\r", "\r")
}

/// The wait `piece` is, when it is one: `s` and a number of seconds.
fn wait(piece: &str) -> Option<Result<Duration, String>> {
    let seconds: f32 = piece.strip_prefix(['s', 'S'])?.parse().ok()?;
    let wait = Duration::try_from_secs_f32(seconds)
        .ok()
        .filter(|wait| *wait <= LONGEST_WAIT && !wait.is_zero());
    Some(wait.ok_or_else(|| {
        format!(
            "`{piece}` waits from a moment to {} seconds",
            LONGEST_WAIT.as_secs()
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(steps: &[Step]) -> Vec<String> {
        steps
            .iter()
            .map(|step| match step {
                Step::Line(line) => line.clone(),
                Step::Wait(wait) => format!("wait {}ms", wait.as_millis()),
            })
            .collect()
    }

    /// A send macro is cut at each `\r` (or line end) into commands and
    /// waits, in order, empty pieces dropped, as `VellumFE` cuts it.
    #[test]
    fn a_send_macro_is_cut_into_commands_and_waits() {
        let steps = Macro::Send("stance off\r\rprep 118\r s1.5 \rcast\r".to_owned()).steps();
        assert_eq!(
            steps.as_deref().map(lines),
            Ok(vec![
                "stance off".to_owned(),
                "prep 118".to_owned(),
                "wait 1500ms".to_owned(),
                "cast".to_owned()
            ])
        );
        assert_eq!(
            Macro::Send("look\nsearch".to_owned())
                .steps()
                .map(|steps| steps.len()),
            Ok(2)
        );
        assert_eq!(
            Macro::Send("s".to_owned()).steps().as_deref().map(lines),
            Ok(vec!["s".to_owned()]),
            "south is a command, not a wait"
        );
    }

    /// A macro that cannot be sent as written is said, not half sent.
    #[test]
    fn what_cannot_be_sent_is_said() {
        assert!(
            Macro::Send("s1\r  \r".to_owned()).steps().is_err(),
            "no command"
        );
        assert!(
            Macro::Send("look\rs600".to_owned()).steps().is_err(),
            "too long"
        );
        assert!(
            Macro::Send("look\rs0".to_owned()).steps().is_err(),
            "no wait"
        );
        assert!(Macro::Fill("prep 111\rcast".to_owned()).check().is_err());
        assert!(Macro::Fill("prep 111 ".to_owned()).check().is_ok());
    }

    /// Each kind is written as the file reads it back, and `""` unbinds.
    #[test]
    fn each_kind_is_written_as_it_is_read() {
        for made in [
            Macro::Send("stance off\rincant 610".to_owned()),
            Macro::Fill("prep 111 ".to_owned()),
            Macro::Act(Action::Stop),
        ] {
            let text = format!("key = {}", made.written());
            let table: toml::Table = toml::from_str(&text).expect("TOML");
            assert_eq!(Macro::read(&table["key"]), Ok(Some(made)), "{text}");
        }
        assert_eq!(Macro::read(&toml::Value::String(String::new())), Ok(None));
        let unknown: toml::Table = toml::from_str("key = { action = \"fly\" }").expect("TOML");
        assert!(Macro::read(&unknown["key"]).is_err());
        let two: toml::Table =
            toml::from_str("key = { fill = \"a\", action = \"stop\" }").expect("TOML");
        assert!(Macro::read(&two["key"]).is_err());
    }

    /// The Keys page shows `\r` as the two characters a player types.
    #[test]
    fn a_player_types_a_command_break_as_backslash_r() {
        assert_eq!(shown("a\rb"), "a\\rb");
        assert_eq!(typed("a\\rb"), "a\rb");
    }
}
