//! What a crossing reads out of the game's answer: a word, a number.
//!
//! Upstream does both with a regex over the next lines (`trail.rb`,
//! `caravan_to_sos.rb`). Here the lines are the model's, already whole.

/// The word that follows `after` in any of these lines: `look trail`, and
/// `...the trail heads off to the northeast.` Upstream's `matchfindword`.
pub(super) fn word_after(lines: &[String], after: &str) -> Option<String> {
    lines.iter().find_map(|line| {
        let (_, rest) = line.split_once(after)?;
        let word: String = rest
            .trim_start()
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect();
        (!word.is_empty()).then_some(word)
    })
}

/// The number of the line that names this: `2) the Sea of Fire`. A caravan's
/// list is renumbered between visits, and the name is what stays.
pub(super) fn numbered(lines: &[String], named: &str) -> Option<u32> {
    let wanted = format!(") {named}");
    lines.iter().find_map(|line| {
        let (before, _) = line.split_once(&wanted)?;
        let digits: String = before
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        digits.parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|line| (*line).to_owned()).collect()
    }

    #[test]
    fn the_word_is_what_follows_and_stops_at_the_full_stop() {
        let heard =
            lines(&["You peer into the mist and see that the trail heads off to the northeast."]);
        assert_eq!(
            word_after(&heard, "the trail heads off to the ").as_deref(),
            Some("northeast")
        );
        assert_eq!(word_after(&heard, "the path heads"), None);
    }

    #[test]
    fn a_destination_is_found_by_name_whatever_its_number() {
        let heard = lines(&["  1) Wehnimer's Landing", "  12) the Sea of Fire"]);
        assert_eq!(numbered(&heard, "the Sea of Fire"), Some(12));
        assert_eq!(numbered(&heard, "Wehnimer's Landing"), Some(1));
        assert_eq!(numbered(&heard, "Vornavis"), None);
    }
}
