//! The keys' names (`plan/47` step 7): each key by the name winit gives its
//! physical code, which keys type, the numpad's names in the fork, and the
//! keys egui has no name for, which the fork catches for Hydra. Moved out of
//! `keys.rs` at its cap.

use std::collections::HashSet;

use egui::Key;
use winit::keyboard::KeyCode;

use super::Chord;

/// The keys that type, by winit name.
pub(super) const TYPING: &[&str] = &[
    "KeyA",
    "KeyB",
    "KeyC",
    "KeyD",
    "KeyE",
    "KeyF",
    "KeyG",
    "KeyH",
    "KeyI",
    "KeyJ",
    "KeyK",
    "KeyL",
    "KeyM",
    "KeyN",
    "KeyO",
    "KeyP",
    "KeyQ",
    "KeyR",
    "KeyS",
    "KeyT",
    "KeyU",
    "KeyV",
    "KeyW",
    "KeyX",
    "KeyY",
    "KeyZ",
    "Digit0",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    "Minus",
    "Equal",
    "Comma",
    "Period",
    "Slash",
    "Backslash",
    "Semicolon",
    "Quote",
    "Backquote",
    "BracketLeft",
    "BracketRight",
    "Space",
];

/// The sixteen numpad keys: winit's name, and the fork's.
pub(super) const NUMPAD: &[(&str, &str)] = &[
    ("Numpad0", "num_0"),
    ("Numpad1", "num_1"),
    ("Numpad2", "num_2"),
    ("Numpad3", "num_3"),
    ("Numpad4", "num_4"),
    ("Numpad5", "num_5"),
    ("Numpad6", "num_6"),
    ("Numpad7", "num_7"),
    ("Numpad8", "num_8"),
    ("Numpad9", "num_9"),
    ("NumpadAdd", "num_plus"),
    ("NumpadSubtract", "num_minus"),
    ("NumpadMultiply", "num_multiply"),
    ("NumpadDivide", "num_divide"),
    ("NumpadEnter", "num_enter"),
    ("NumpadDecimal", "num_decimal"),
];

/// Keys egui has no name for, which the author's fork catches before egui
/// sees them when Hydra asks (`eframe::Frame::set_key_capture`, pinned at
/// `ed8b264`, `plan/47` step 7): their winit names and codes.
pub(super) const CAPTURED: [(&str, KeyCode); 5] = [
    ("Pause", KeyCode::Pause),
    ("ScrollLock", KeyCode::ScrollLock),
    ("PrintScreen", KeyCode::PrintScreen),
    ("CapsLock", KeyCode::CapsLock),
    ("ContextMenu", KeyCode::ContextMenu),
];

/// The key winit calls `NumLock`: `NumLock` itself, the operating system's
/// switch for the numpad, on Windows and Linux; macOS's Clear key, which is
/// Hydra's switch for it there. Bound to nothing either way.
pub(crate) const NUM_LOCK: KeyCode = KeyCode::NumLock;

/// The winit name `key` is written as, checked: a key this build can see.
pub(super) fn known(key: &str) -> Result<String, String> {
    if let Some((winit, _)) = NUMPAD
        .iter()
        .find(|(winit, fork)| winit.eq_ignore_ascii_case(key) || fork.eq_ignore_ascii_case(key))
    {
        return Ok((*winit).to_owned());
    }
    if let Some(name) = Key::ALL
        .iter()
        .filter_map(|key| winit_name(*key))
        .find(|name| name.eq_ignore_ascii_case(key))
    {
        return Ok(name);
    }
    if let Some((name, _)) = CAPTURED
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
    {
        return Ok((*name).to_owned());
    }
    if key.eq_ignore_ascii_case("NumLock") || key.eq_ignore_ascii_case("Clear") {
        return Err(format!(
            "{key} switches the numpad between typing and its keys (NumLock; Clear on a Mac), and is bound to nothing."
        ));
    }
    Err(format!(
        "{key} is not a key winit names; see the key names in plan/47 step 7."
    ))
}

/// winit's name for the physical key egui calls `key`: `KeyA` for `A`,
/// `Digit1` for `Num1`, the same name for the function keys, the arrows and
/// the rest; `None` for an egui key that is a character and not a key
/// (`Plus`, `Colon`, `Pipe`...).
pub(crate) fn winit_name(key: Key) -> Option<String> {
    let name = format!("{key:?}");
    if name.len() == 1 {
        return Some(format!("Key{name}"));
    }
    if let Some(digit) = name.strip_prefix("Num")
        && digit.len() == 1
    {
        return Some(format!("Digit{digit}"));
    }
    let renamed = match key {
        Key::Equals => "Equal",
        Key::Backtick => "Backquote",
        Key::OpenBracket => "BracketLeft",
        Key::CloseBracket => "BracketRight",
        Key::Plus
        | Key::Colon
        | Key::Pipe
        | Key::Questionmark
        | Key::Exclamationmark
        | Key::OpenCurlyBracket
        | Key::CloseCurlyBracket
        | Key::Copy
        | Key::Cut
        | Key::Paste => return None,
        _ => return Some(name),
    };
    Some(renamed.to_owned())
}

/// Whether the key of this name types something when pressed. A bound
/// key that does is taken with what it typed: egui sends a letter's text
/// beside its press unless Ctrl or Cmd is held, so Alt+C did its macro and
/// typed a `c` as well (the review of 2026-09-29).
pub(crate) fn types(key: &str) -> bool {
    TYPING.contains(&key)
}

/// The chord a numpad press is, its key named as winit names it; `None` for
/// a release, or a key with no code.
pub(crate) fn numpad_chord(event: &eframe::NumpadKeyEvent) -> Option<Chord> {
    if !event.pressed {
        return None;
    }
    let winit::keyboard::PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    Some(Chord::of(&format!("{code:?}"), event.modifiers))
}

/// The chord of a numpad press the fork caught, to be done as the window
/// with the keyboard binds it; `None` for one let through to be typed.
pub(crate) fn numpad_caught(event: &eframe::NumpadKeyEvent) -> Option<Chord> {
    if !event.consumed {
        return None;
    }
    numpad_chord(event)
}

/// Every key egui has no name for that a binding may use: what the fork
/// catches while the Keys page waits for a key, so it can take one.
pub(crate) fn capturable() -> HashSet<winit::keyboard::KeyCode> {
    CAPTURED.iter().map(|(_, code)| *code).collect()
}

/// The chord a press the fork's key capture caught is; `None` for a
/// release, or a key with no code.
pub(crate) fn captured_chord(event: &eframe::CapturedKeyEvent) -> Option<Chord> {
    if !event.pressed {
        return None;
    }
    let winit::keyboard::PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    Some(Chord::of(&format!("{code:?}"), event.modifiers))
}
