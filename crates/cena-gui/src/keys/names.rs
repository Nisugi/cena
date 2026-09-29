//! The keys' names (`plan/47` step 7): each key by the name winit gives its
//! physical code, which keys type, the numpad's names in the fork, and the
//! keys this build cannot see yet. Moved out of `keys.rs` at its cap.

use egui::Key;

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

/// Keys winit names that no path reaches yet: they need the fork's hook
/// widened (this module's docs).
const NOT_YET: &[&str] = &[
    "Pause",
    "ScrollLock",
    "PrintScreen",
    "NumLock",
    "ContextMenu",
    "CapsLock",
];

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
    if NOT_YET.iter().any(|name| name.eq_ignore_ascii_case(key)) {
        return Err(format!(
            "Hydra cannot see {key} yet: it needs the egui fork's key hook widened (plan/47 step 7)."
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
