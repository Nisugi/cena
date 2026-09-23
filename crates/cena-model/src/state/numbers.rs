//! The one reader for a comma-grouped integer the game printed.
//!
//! # Why one, and why strict
//!
//! Eight private copies of this existed -- `number`/`signed` twice
//! (`currency.rs`, `experience_report.rs`), and comma-strippers in `bank.rs`,
//! `standing.rs` (two), `training.rs`, `bounty_status.rs` and `blocks.rs`. They
//! disagreed about the one thing that matters: the two `number`s kept **every
//! digit and dropped everything else**, so `"12a3"` read as `123` and
//! `"-5 (was 9)"` as `59`.
//!
//! That is the partial parse `cena-protocol`'s `numbers.rs` refuses in its
//! own `signed`: *"a partial parse of something unexpected, which is precisely
//! how a wrong number reaches a behavior that acts on it."* A figure the wire
//! did not send is worse than no figure, because a caller cannot tell the two
//! apart -- `None` at least says "not understood".
//!
//! So this accepts exactly what Lich's captures accept, `-?[\d,]+`, and
//! nothing else: the whole token must be a number, commas are the only
//! separator, and the sign survives. `T` decides the range, so an unsigned
//! target refuses a minus sign by construction rather than by a check here.

use std::str::FromStr;

/// `"1,024"` -> `1024`, `"-3"` -> `-3`; `None` for anything that is not a
/// comma-grouped integer **end to end** (`"12a3"`, `"1 024"`, `""`, `","`).
///
/// Surrounding whitespace is trimmed; nothing else is forgiven.
pub(crate) fn grouped<T: FromStr>(text: &str) -> Option<T> {
    let text = text.trim();
    let magnitude = text.strip_prefix('-').unwrap_or(text);
    // At least one digit, and nothing but digits and commas. `FromStr` then
    // rejects what is left over -- a `-` into an unsigned type, or a value out
    // of `T`'s range -- rather than wrapping or clamping it.
    if !magnitude.bytes().any(|b| b.is_ascii_digit())
        || !magnitude.bytes().all(|b| b.is_ascii_digit() || b == b',')
    {
        return None;
    }
    text.replace(',', "").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::grouped;

    #[test]
    fn a_grouped_figure_reads_whole() {
        assert_eq!(grouped::<u64>("1,453,539,090"), Some(1_453_539_090));
        assert_eq!(grouped::<u32>(" 3673 "), Some(3673));
        assert_eq!(grouped::<i64>("-1,234"), Some(-1234));
        assert_eq!(grouped::<u32>("0"), Some(0));
    }

    #[test]
    fn a_partial_number_is_refused_not_repaired() {
        // The defect the copies had: filtering to digits turned these into
        // numbers the wire never sent.
        assert_eq!(grouped::<u64>("12a3"), None);
        assert_eq!(grouped::<u64>("1 024"), None);
        assert_eq!(grouped::<i64>("-5 (was 9)"), None);
        assert_eq!(grouped::<u64>(""), None);
        assert_eq!(grouped::<u64>(","), None);
        assert_eq!(grouped::<u64>("-"), None);
    }

    #[test]
    fn the_target_type_decides_sign_and_range() {
        assert_eq!(grouped::<u64>("-5"), None, "unsigned refuses a sign");
        assert_eq!(grouped::<u32>("4,294,967,296"), None, "out of range");
        assert_eq!(grouped::<i32>("-2,147,483,648"), Some(i32::MIN));
    }
}
