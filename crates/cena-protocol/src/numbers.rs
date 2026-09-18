//! `current/max` extraction from a progress bar's `text=` attribute.
//!
//! # Why this is not a port
//!
//! Vellum's `parse_progress_numbers` (`reference/VellumFE/src/parser/numbers.rs:9-31`)
//! has two defects, both confirmed by **compiling that exact function and
//! running it**, not by reading it:
//!
//! ```text
//! health -10/125  pct=0   -> (10, 125)     // sign discarded
//! health -1/125   pct=0   -> (1, 125)
//! health 213/223  pct=95  -> (213, 223)    // correct
//! numbed          pct=90  -> (90, 100)     // max fabricated
//! ```
//!
//! 1. **The sign is discarded.** It splits on non-digits, so `-10` becomes
//!    `10`. A character at -10 health -- dying -- reports as +10. A Heal
//!    behavior reading that concludes the character is fine while they bleed
//!    out, which for Cena is the difference between a curated behavior that
//!    works and one that kills people. So [`Amount::current`] is `i32`.
//! 2. **A maximum is invented.** A label-only bar (`text='numbed'`) returns
//!    `(percentage, 100)`, a max the wire never sent, indistinguishable from a
//!    real one. So the return is `Option`: absent means absent.
//!
//! The `value=` attribute does not rescue either case -- it is a **percentage**
//! (`text='health 213/223'` ships `value='95'`), and at negative health it
//! reads `0`. The number the caller wants lives only in `text=`.

use crate::frame::Amount;

/// Parse `current/max` out of a progress bar's `text=` attribute.
///
/// Returns `None` when the text does not state a pair -- a label-only bar such
/// as `numbed`, or `43904921 experience`, where inventing a denominator would
/// be a lie the caller cannot detect.
///
/// ```
/// use cena_protocol::numbers::parse_amount;
/// use cena_protocol::frame::Amount;
///
/// assert_eq!(parse_amount("health 213/223"), Some(Amount { current: 213, max: 223 }));
/// assert_eq!(parse_amount("health -10/125"), Some(Amount { current: -10, max: 125 }));
/// assert_eq!(parse_amount("numbed"), None);
/// ```
#[must_use]
pub fn parse_amount(text: &str) -> Option<Amount> {
    let trimmed = text.trim();
    // `rfind`, not `find`: a label may itself contain a slash.
    let slash = trimmed.rfind('/')?;
    let current = last_signed_number(&trimmed[..slash])?;
    let max = first_signed_number(&trimmed[slash + 1..])?;
    Some(Amount { current, max })
}

/// Parse a `time=` countdown into whole seconds.
///
/// Buffs, debuffs and cooldowns carry `time='00:00:37'`, and a behavior that
/// wants to know when a cooldown expires needs a duration rather than the
/// string. Rule 2.1 (`plan/05:270-274`) puts that conversion here.
///
/// The wire form is uniform: every one of 55,444 `time=` values in a 20-file
/// corpus census matches `HH:MM:SS`. `MM:SS` is accepted too, because it costs
/// one line and the alternative is silently returning `None` if the game ever
/// drops the hour. Anything else is `None` rather than a guess -- a fabricated
/// duration is the same class of lie as the fabricated maximum above.
///
/// ```
/// use cena_protocol::numbers::parse_duration_secs;
///
/// assert_eq!(parse_duration_secs("00:00:37"), Some(37));
/// assert_eq!(parse_duration_secs("01:02:03"), Some(3723));
/// assert_eq!(parse_duration_secs("soon"), None);
/// ```
#[must_use]
pub fn parse_duration_secs(text: &str) -> Option<u32> {
    let mut secs: u32 = 0;
    let mut parts = 0;
    for field in text.trim().split(':') {
        // Every field must be wholly numeric: `parse` rejects `"3a"`, and not
        // "helping" it is the same strictness `signed` documents above.
        let value: u32 = field.parse().ok()?;
        secs = secs.checked_mul(60)?.checked_add(value)?;
        parts += 1;
    }
    // `MM:SS` or `HH:MM:SS`. A bare `"37"` is rejected: the wire never sends
    // one, and accepting it would read a stray number as a duration.
    (2..=3).contains(&parts).then_some(secs)
}

/// Split into tokens on whitespace and bracket/percent noise.
fn tokens(input: &str) -> impl DoubleEndedIterator<Item = &str> {
    input.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '%')
}

/// The last token in `input` that is a signed integer.
fn last_signed_number(input: &str) -> Option<i32> {
    tokens(input).rev().find_map(signed)
}

/// The first token in `input` that is a signed integer.
fn first_signed_number(input: &str) -> Option<i32> {
    tokens(input).find_map(signed)
}

/// A whole token parsed as a signed integer.
///
/// Two deliberate strictnesses, each of which a first draft got wrong and a
/// test caught:
///
/// 1. **The sign survives.** Vellum splits on every non-digit, which is what
///    discards the minus in `-10` and is the whole negative-health bug.
/// 2. **The token must be a number end to end.** Trimming stray characters
///    off the ends turns `12a` into `12` -- a partial parse of something
///    unexpected, which is precisely how a wrong number reaches a behavior
///    that acts on it. `str::parse` already rejects `12a`, so the correct
///    implementation is to not "help" it.
fn signed(token: &str) -> Option<i32> {
    if token.is_empty() {
        return None;
    }
    token.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_number_lives_in_text_not_in_value() {
        // The fixture trap: value='99' is a percentage, text carries 225/226.
        assert_eq!(
            parse_amount("health 225/226"),
            Some(Amount {
                current: 225,
                max: 226
            })
        );
        assert_eq!(
            parse_amount("spirit 8/9"),
            Some(Amount { current: 8, max: 9 })
        );
    }

    #[test]
    fn negative_health_keeps_its_sign() {
        // Vellum returns (10, 125) here. VERIFIED by running its function.
        // A dying character must not read as healthy.
        assert_eq!(
            parse_amount("health -10/125"),
            Some(Amount {
                current: -10,
                max: 125
            })
        );
        assert_eq!(
            parse_amount("health -1/125"),
            Some(Amount {
                current: -1,
                max: 125
            })
        );
        assert_eq!(
            parse_amount("health 0/125"),
            Some(Amount {
                current: 0,
                max: 125
            })
        );
    }

    #[test]
    fn a_label_only_bar_has_no_amount_rather_than_a_fabricated_one() {
        // Vellum returns (percentage, 100) for all of these.
        assert_eq!(parse_amount("numbed"), None);
        assert_eq!(parse_amount("becoming numbed"), None);
        assert_eq!(parse_amount("43904921 experience"), None);
        assert_eq!(parse_amount(""), None);
        assert_eq!(parse_amount("   "), None);
    }

    #[test]
    fn the_last_slash_is_the_separator_not_the_first() {
        // `parse_amount` uses `rfind`, and the comment says why: "a label may
        // itself contain a slash". Nothing asserted it, so `find` passed the
        // whole suite -- an attack on this crate's tests found exactly that.
        // With `find`, the first slash splits and the pair is misread.
        assert_eq!(
            parse_amount("a/b 213/223"),
            Some(Amount {
                current: 213,
                max: 223
            })
        );
        assert_eq!(
            parse_amount("Combat Maneuvers/Shield 5/10"),
            Some(Amount {
                current: 5,
                max: 10
            })
        );
    }

    #[test]
    fn a_countdown_parses_to_seconds_and_a_label_does_not() {
        assert_eq!(parse_duration_secs("00:00:37"), Some(37));
        assert_eq!(parse_duration_secs("00:02:00"), Some(120));
        assert_eq!(parse_duration_secs("01:02:03"), Some(3723));
        // `MM:SS`, accepted.
        assert_eq!(parse_duration_secs("02:30"), Some(150));
        // A bare number is not a duration: the wire never sends one, and
        // reading a stray value as seconds is a fabricated answer.
        assert_eq!(parse_duration_secs("37"), None);
        assert_eq!(parse_duration_secs(""), None);
        assert_eq!(parse_duration_secs("soon"), None);
        assert_eq!(parse_duration_secs("00:0a:37"), None);
        assert_eq!(parse_duration_secs("1:2:3:4"), None);
        // Overflow is None, never a panic and never a wrong number.
        assert_eq!(parse_duration_secs("99999999:99999999:99999999"), None);
    }

    #[test]
    fn malformed_pairs_do_not_produce_half_an_answer() {
        assert_eq!(parse_amount("health /223"), None);
        assert_eq!(parse_amount("health 213/"), None);
        assert_eq!(parse_amount("/"), None);
        // A token that is not wholly a number is not a number.
        assert_eq!(parse_amount("health 12a/223"), None);
        // Overflow is not a panic and not a wrong number.
        assert_eq!(parse_amount("health 99999999999/1"), None);
    }
}
