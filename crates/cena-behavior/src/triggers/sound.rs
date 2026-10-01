//! A trigger's `sound` is a file name in Hydra's sounds folder, and nothing
//! else (the crate review of 2026-10-01, BI-B-1 and BE-F-4).
//!
//! A path was played from where it pointed. A triggers file or a Wrayth
//! settings file is something players pass around, and on Windows a path
//! like `\\host\share\ding.wav` is opened by reaching out to `host` over the
//! network, which answers with the player's Windows credentials: the
//! mechanism of Outlook's reminder-sound flaw (CVE-2023-23397). Looking for
//! the file was enough, at every reload of the triggers.
//!
//! So a sound names a file, never a place: no folder, drive, network host
//! or device. The rule is one function, [`refused`], read on strings rather
//! than by the running system's `Path`, so it says the same on every OS;
//! and it is asked everywhere a sound comes in or goes out:
//!
//! - **an import** keeps a sound's file name ([`imported`]): a Wrayth
//!   file's sounds point at the machine that made it, and the sounds folder
//!   was already where they were looked for when that path was not here;
//! - **`;trigger set`** and the trigger editor refuse a sound that is not
//!   a file name (`edit::set`, `edit::save`);
//! - **a reload** says which sounds will not play and why, and the desk
//!   that plays them refuses them before it looks for anything (the
//!   binary's `attention.rs`), so a file written by hand is held to it too.

/// Why `named` cannot be a trigger's sound, or `None` when it can: a file
/// name in the sounds folder, with no folder, drive, network host or device
/// in it. The reason reads after the sound's name: "`x` is a path ...".
#[must_use]
pub fn refused(named: &str) -> Option<&'static str> {
    let name = named.trim();
    if name.is_empty() {
        return Some("names no file");
    }
    if name.starts_with("\\\\") || name.starts_with("//") {
        return Some(
            "is a network or device path, which Windows would open by reaching out to \
             another machine",
        );
    }
    if name.contains(['\\', '/']) {
        return Some("is a path, and a sound is only a file name in the sounds folder");
    }
    if name.contains(':') {
        return Some("names a drive or a stream, and a sound is only a file name");
    }
    if name == "." || name == ".." {
        return Some("is not a file name");
    }
    if name.chars().any(char::is_control) {
        return Some("has a character no file name has");
    }
    if device(name) {
        return Some("is a device's name on Windows, not a file");
    }
    None
}

/// The file name at the end of `written`, a path as another program wrote
/// it: what follows its last `\` or `/`.
#[must_use]
pub fn file_name(written: &str) -> &str {
    written.rsplit(['\\', '/']).next().unwrap_or(written).trim()
}

/// `written`, a sound an import brings, as Hydra keeps it: its file name.
///
/// # Errors
///
/// Even its file name cannot be a sound; the reason, as [`refused`] gives it.
pub fn imported(written: &str) -> Result<String, &'static str> {
    let name = file_name(written);
    refused(name).map_or_else(|| Ok(name.to_owned()), Err)
}

/// Whether `name` is one Windows keeps for a device in every folder:
/// `CON`, `NUL`, `COM1` and the rest, with or without an extension.
fn device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    match stem.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" => true,
        _ => ["COM", "LPT"].iter().any(|port| {
            stem.strip_prefix(port).is_some_and(|number| {
                matches!(
                    number,
                    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
                ) || matches!(number, "\u{b9}" | "\u{b2}" | "\u{b3}")
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows-shaped paths, refused on every OS: the rule reads the
    /// string, not the running system's idea of a path.
    #[test]
    fn a_path_of_any_shape_is_refused_and_a_file_name_is_not() {
        for path in [
            r"\\attacker.example\s\ding.wav",
            r"\\203.0.113.9\s\a.wav",
            "//host/share/ding.wav",
            r"\\?\C:\fx\ding.wav",
            r"\\?\UNC\host\share\ding.wav",
            r"\\.\pipe\ding",
            r"\\.\C:\ding.wav",
            r"C:\fx\ding.wav",
            "C:ding.wav",
            "ding.wav:stream",
            "/home/someone/ding.wav",
            r"..\ding.wav",
            "../ding.wav",
            r"fx\ding.wav",
            "..",
            ".",
            "",
            "   ",
            "CON",
            "nul.wav",
            "Com1.wav",
            "LPT9",
            "ding\u{0}.wav",
        ] {
            assert!(refused(path).is_some(), "{path:?} should be refused");
        }
        for name in [
            "ding.wav",
            "ding",
            "Ding Dong.mp3",
            "console.wav",
            "com10.wav",
            "..wav",
        ] {
            assert_eq!(refused(name), None, "{name:?} is a file name");
        }
        assert_eq!(
            refused(r"\\host\s\ding.wav"),
            Some(
                "is a network or device path, which Windows would open by reaching out to \
                 another machine"
            ),
            "a UNC path says what it is, not just that it is a path"
        );
    }

    #[test]
    fn an_import_keeps_a_sounds_file_name() {
        assert_eq!(
            imported(r"\\attacker.example\s\ding.wav"),
            Ok("ding.wav".into())
        );
        assert_eq!(imported(r"C:\fx\data.wav"), Ok("data.wav".into()));
        assert_eq!(imported("chime"), Ok("chime".into()));
        assert!(imported(r"\\host\share\..").is_err());
        assert!(imported(r"C:\fx\CON").is_err());
        assert!(
            imported(r"\\host\share\").is_err(),
            "a path ending in a folder"
        );
    }
}
