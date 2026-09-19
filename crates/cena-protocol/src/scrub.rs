//! Redaction for wire-log excerpts cut into committed fixtures.
//!
//! # Why this is code and not a checklist
//!
//! `druby://` host URIs appear in **2,221 of 10,849** corpus files (20.5%,
//! `grep -rlI --include='*.xml' 'druby://' . | wc -l` in
//! `E:\Gemstone\data\log archive`). Lich broadcasts its `DRb` endpoint through
//! in-game group whispers, so those lines carry the author's real link-local
//! IPv6 addresses, interface indices and ephemeral ports. The real shape is
//! `druby://<link-local v6>%<iface>:<port>`; VERIFIED present in the corpus,
//! but NOT reproduced here -- this file is the thing that removes such
//! addresses, so hardcoding a live one would publish in the fixer the very
//! datum the fixer exists to delete. Examples below use the RFC 3849
//! documentation prefix `2001:db8::/32`. At that prevalence a
//! manual redaction pass will eventually miss one, and fixtures get re-cut.
//!
//! So the rule is a function, and `tests/fixtures_are_scrubbed.rs` runs it
//! over every committed fixture. That test can go RED (plan/05 §0): it was
//! VERIFIED red by writing an unscrubbed excerpt into the fixture directory,
//! and it named the file and the pattern.
//!
//! # What it does, and the three things it does not
//!
//! 1. **Host URIs** are replaced wholesale. Nothing about a `DRb` endpoint is
//!    game data, so no structure is preserved.
//! 2. **Player names** are pseudonymised **consistently** -- the same input
//!    name always yields the same output name, so `exist=`-to-name
//!    correlation still tests correctly, which is the whole point of keeping
//!    a `<component id='room players'>` fixture at all.
//! 3. **Third-party speech lines are dropped, not rewritten.** Whispers,
//!    group OOC and private channels carry other people's words; consent for
//!    those is not obtainable, and a pseudonym does not make a stranger's
//!    sentence publishable.
//!
//! Deliberately **not** scrubbed, because they are game data and the parser
//! is tested on them: `exist=` ids, `nav rm=` room ids, item nouns,
//! `<settingsInfo client= crc=>`.
//!
//! # Limits, stated rather than implied
//!
//! `scrub` pseudonymises only names it is **told** about, via
//! [`Scrubber::pseudonymise`]. It does not guess which capitalised word is a
//! person: `Inochi` and `Rawknuckle's` are the same shape, one is a player and
//! one is a tavern. Deciding that automatically would either mangle room names
//! or miss players, and both failures are silent. The cutter names the players
//! it saw; the test then proves those names are absent.

use std::collections::BTreeMap;

/// The replacement written over a redacted host URI.
///
/// A syntactically valid URI, so a parser under test still sees a URI shape
/// where the wire had one -- the fixture exercises the same code path.
const REDACTED_HOST: &str = "druby://redacted.invalid:0";

/// Line-start markers whose whole line is dropped.
///
/// Each is a stream of third-party speech. `preset id="whisper"` and the
/// `society`/`bounty` streams are named in the corpus findings; the
/// `whispers` string catches the plain-text form Lich writes when the `DRb`
/// handshake goes out over a group whisper, which is how the host URIs got
/// into the corpus in the first place.
const THIRD_PARTY_MARKERS: &[&str] = &[
    "druby://",
    "whispers to the group",
    "whispers,",
    "<preset id=\"whisper\">",
    "<preset id='whisper'>",
    "pushStream id=\"society\"",
    "pushStream id='society'",
    "pushStream id=\"bounty\"",
    "pushStream id='bounty'",
    // **ESP and speech, which the module doc promised and this list omitted**
    // (review PR-7). Rule 3 above says private channels are DROPPED rather
    // than pseudonymised, because consent for a stranger's words is not
    // obtainable and a pseudonym does not make their sentence publishable.
    //
    // `thoughts` is the ESP/telepathy channel
    // (`reference/wiki_clean/Wrayth protocol.txt:61`), and the wiki's own
    // example is a named player's words:
    //
    //     You hear the faint thoughts of <name> echo in your mind:
    //
    // It is also an "Exclusive" stream (`:76`), so with the window closed the
    // text falls through into main wrapped in a style -- which means the
    // `pushStream` marker is not always present and the prose form has to be
    // caught too.
    "pushStream id=\"thoughts\"",
    "pushStream id='thoughts'",
    "<preset id=\"thought\">",
    "<preset id='thought'>",
    "thoughts of",
    "pushStream id=\"speech\"",
    "pushStream id='speech'",
    // `voln` routes to `thoughts` when closed (`:77`), so it is the same
    // channel arriving under another name.
    "pushStream id=\"voln\"",
    "pushStream id='voln'",
];

/// Consistent pseudonyms, and the redaction rules that need no configuration.
#[derive(Debug, Default)]
pub struct Scrubber {
    /// Real name -> pseudonym. Ordered so a re-run reports identically.
    names: BTreeMap<String, String>,
}

impl Scrubber {
    /// A scrubber with no names registered yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Map `real` to `pseudonym` for every later [`scrub`](Self::scrub).
    ///
    /// Registering the same `real` twice overwrites the mapping; that is the
    /// caller's decision to make, and a fixture cut in one pass registers each
    /// name once.
    pub fn pseudonymise(&mut self, real: &str, pseudonym: &str) {
        self.names.insert(real.to_owned(), pseudonym.to_owned());
    }

    /// Scrub one wire excerpt.
    ///
    /// Line-oriented: drops third-party lines whole, then rewrites what
    /// remains. Input may be CRLF; output is always LF-terminated with a final
    /// newline, because a golden carrying `\r` makes every diff unreadable
    /// (corpus findings, determinism item 4).
    #[must_use]
    pub fn scrub(&self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for raw in input.lines() {
            let line = raw.trim_end_matches('\r');
            if THIRD_PARTY_MARKERS.iter().any(|m| line.contains(m)) {
                continue;
            }
            out.push_str(&self.rewrite(line));
            out.push('\n');
        }
        out
    }

    /// Apply the non-dropping rewrites to a single line.
    fn rewrite(&self, line: &str) -> String {
        // FIRST: unwrap Lich's line prefix, so every rewrite below sees the wire
        // rather than a decorated copy of it.
        let mut text = strip_vellum_images(strip_lich_timestamp(line));
        text = redact_host_uris(&text);
        replace_names(&text, &self.names)
    }
}

/// Replace every whole-word occurrence of a real name, in **one pass**.
///
/// # The three defects this replaces
///
/// It was `for (real, pseudonym) in &self.names { text = text.replace(..) }`,
/// and each part of that was wrong (review PR-7):
///
/// 1. **Chained.** Each pass ran over the previous pass's OUTPUT, so with
///    `{Al -> Bob, Bob -> Cy}` -- ordinary for a `BTreeMap` -- an `Al` became
///    `Bob` and then `Cy`, and two players collapsed into one. A fixture that
///    merges two people is worse than an unscrubbed one, because it looks
///    correct.
/// 2. **Substring.** `Eon` rewrote the inside of `Eonake`, corrupting a name
///    that was never registered and producing text no parser should have to
///    see.
/// 3. **Case-sensitive.** The wire capitalises names at the start of a
///    sentence and Lich lowercases them in some commands, so a registered
///    `Alderin` left every `alderin` in place. So did the checker in
///    `tests/fixtures_are_scrubbed.rs`, which is why nothing caught it.
///
/// One pass over the input fixes 1. Word boundaries fix 2. Case-insensitive
/// matching fixes 3, and the pseudonym adopts the case pattern of what it
/// replaced so a sentence still reads as a sentence.
fn replace_names(text: &str, names: &std::collections::BTreeMap<String, String>) -> String {
    if names.is_empty() {
        return text.to_owned();
    }
    let lower = text.to_lowercase();
    // Longest first, so `Eon` cannot claim the start of a registered
    // `Eonake` before `Eonake` is tried.
    let mut ordered: Vec<(&String, &String)> = names.iter().collect();
    ordered.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(b.0)));

    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'outer: while i < text.len() {
        if text.is_char_boundary(i) && !is_name_byte(i.checked_sub(1).map(|p| bytes[p])) {
            for (real, pseudonym) in &ordered {
                let end = i + real.len();
                if end <= text.len()
                    && lower.is_char_boundary(i)
                    && lower.is_char_boundary(end)
                    && lower[i..end] == real.to_lowercase()
                    && !is_name_byte(bytes.get(end).copied())
                {
                    out.push_str(&match_case(&text[i..end], pseudonym));
                    i = end;
                    continue 'outer;
                }
            }
        }
        let ch = text[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Is this byte part of a name, for word-boundary purposes?
///
/// `None` (start or end of input) is a boundary. Letters, digits and `_` are
/// not: `Eonake` must not match a registered `Eon`, and `Alderin2` is a
/// different token from `Alderin`.
fn is_name_byte(b: Option<u8>) -> bool {
    b.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Give `pseudonym` the case pattern of the `matched` text it replaces.
///
/// The wire capitalises a name at the start of a sentence and lowercases it in
/// some command echoes. Matching case-insensitively and then emitting the
/// pseudonym verbatim would turn `alderin` into `Bob` mid-sentence, which is a
/// tell that the fixture was rewritten -- and the fixtures exist to look like
/// wire traffic.
fn match_case(matched: &str, pseudonym: &str) -> String {
    let upper = matched.chars().filter(|c| c.is_alphabetic());
    let all_upper =
        matched.chars().any(char::is_alphabetic) && upper.clone().all(char::is_uppercase);
    if all_upper {
        return pseudonym.to_uppercase();
    }
    let leading_lower = matched.chars().next().is_some_and(char::is_lowercase);
    if leading_lower {
        return pseudonym.to_lowercase();
    }
    pseudonym.to_owned()
}

/// Remove `<vellumImg .../>` elements.
///
/// VERIFIED client injection, not wire traffic:
/// `reference/VellumFE/src/core/inline_image.rs:5`. A fixture cut from a log
/// that a different client wrote would otherwise teach this parser a tag the
/// game never sends -- the fixture would pass and the wire would not.
#[must_use]
pub fn strip_vellum_images(line: &str) -> String {
    let needle = "<vellumImg";
    if !line.contains(needle) {
        return line.to_owned();
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find(needle) {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        match after.find('>') {
            Some(end) => rest = &after[end + 1..],
            // An unterminated tag at end of line: drop the remainder rather
            // than emit half an element.
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Strip Lich's `HH:MM:SS: ` line prefix.
///
/// > **AUTHOR, 2026-09-19:** *"lich xml file is basically the wire"* ... *"You
/// > can't just remove the timestamps from the ones you use as a fixture?"*
///
/// Both are right, and together they are why this exists. The XML in a Lich
/// `.xml` log **is** the wire -- Lich does not rewrite tags -- but it prefixes
/// some lines with a local clock. That prefix is not wire traffic, and a fixture
/// keeping it teaches the parser a lie: VERIFIED that
/// `01:46:11: <component id='room objs'>a rock</component>` parses as a spurious
/// `Text("01:46:11: ")` frame followed by the component.
///
/// Removing it is a better answer than skipping such files. MEASURED: of a
/// 48-file sample across 8 characters, most carry the prefix on some lines, and
/// `tests/FIXTURES.md`'s original rule -- take only files with **zero**
/// timestamped lines -- was discarding most of the archive to avoid a
/// ten-character prefix.
///
/// **The exact shape is `HH:MM:SS: `** -- the third colon belongs to Lich's
/// format, then exactly one space. MEASURED over 12 files across 4 characters:
/// **83,515** lines match `HH:MM:SS: ` and **zero** match `HH:MM:SS ` without
/// the trailing colon, so there is one form and this is it.
///
/// **Anchored at the start of the line, and only there.** A timestamp inside
/// prose ("meet me at 01:46:11: sharp") is display text the game sent, and
/// stripping it would edit the wire rather than unwrap it.
///
/// **THE SECOND SHAPE, added 2026-09-19.** A newer Lich build writes a full
/// date and a spelled-out zone instead:
///
/// ```text
/// 2026-09-01 15:13:58 Central Standard Time: <component id='room objs'>...
/// ```
///
/// MEASURED in `E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi\2026\09\xml`:
/// **4,427,147** occurrences across the month, one single zone spelling, and in
/// the largest file **28,879 of 29,122 lines** carry it -- every one of them
/// line-anchored, zero mid-line (`grep -c 'Central Standard Time: '` equals the
/// anchored `grep -cE`, both 28,879).
///
/// The two shapes are **disjoint by era**, which is why this is not a
/// duplicate to be folded together: of a 200-file sample of the archive,
/// **zero** carry either prefix; of the 20 `dev/lich-5` files, all but each
/// login preamble carry this one. The archive is the older build.
///
/// Without this rule those logs are unusable as fixtures, and the failure mode
/// is the one the `HH:MM:SS: ` docs above already record -- VERIFIED by parsing
/// a prefixed line, which yielded
/// `Text("2026-09-01 15:13:58 Central Standard Time: ")` ahead of the real
/// component. That is a fact the game never sent, baked into a golden.
#[must_use]
pub fn strip_lich_timestamp(line: &str) -> &str {
    strip_zoned_timestamp(strip_bare_timestamp(line))
}

/// `HH:MM:SS: ` -- the older build's prefix.
fn strip_bare_timestamp(line: &str) -> &str {
    let b = line.as_bytes();
    if b.len() < 10 {
        return line;
    }
    let digits = |i: usize| b[i].is_ascii_digit();
    if digits(0)
        && digits(1)
        && b[2] == b':'
        && digits(3)
        && digits(4)
        && b[5] == b':'
        && digits(6)
        && digits(7)
        && b[8] == b':'
        && b[9] == b' '
    {
        return &line[10..];
    }
    line
}

/// `YYYY-MM-DD HH:MM:SS <Zone Name>: ` -- the newer build's prefix.
///
/// The zone is matched as "ASCII letters and spaces, then `: `" rather than
/// against a list of zone names: the author travels between zones, and a list
/// would silently stop stripping in a new one. The date and clock are matched
/// exactly, so the letters-and-spaces run is only reachable on a line that
/// already opens with a full timestamp -- which is what keeps this from eating
/// prose.
///
/// **Every word must be Capitalised**, and that is not cosmetic -- it is what
/// separates a zone name from a sentence. A letters-and-spaces scan alone
/// accepted `is when the caravan leaves` and ate the prose after it; the
/// test `a_date_in_prose_does_not_lose_its_sentence` FAILED on exactly that,
/// before this condition existed. Zone spellings are title-case without
/// exception (`Central Standard Time`, `UTC`, `Pacific Daylight Time`), so
/// requiring an uppercase first letter per word costs nothing and refuses
/// ordinary prose.
///
/// **Also bounded**, as a second line of defence. MEASURED: `Central Standard
/// Time` is 21 characters; the longest such spelling in use, `Australian
/// Central Western Standard Time`, is 40. The bound is 48.
fn strip_zoned_timestamp(line: &str) -> &str {
    const MAX_ZONE: usize = 48;
    let b = line.as_bytes();
    // `YYYY-MM-DD HH:MM:SS ` is 20 bytes, then at least `X: `.
    if b.len() < 23 {
        return line;
    }
    let digits = |i: usize| b[i].is_ascii_digit();
    let shaped = digits(0)
        && digits(1)
        && digits(2)
        && digits(3)
        && b[4] == b'-'
        && digits(5)
        && digits(6)
        && b[7] == b'-'
        && digits(8)
        && digits(9)
        && b[10] == b' '
        && digits(11)
        && digits(12)
        && b[13] == b':'
        && digits(14)
        && digits(15)
        && b[16] == b':'
        && digits(17)
        && digits(18)
        && b[19] == b' ';
    if !shaped {
        return line;
    }
    let mut i = 20;
    let limit = (20 + MAX_ZONE).min(b.len());
    let mut at_word_start = true;
    while i < limit {
        let c = b[i];
        if c == b' ' {
            at_word_start = true;
        } else if c.is_ascii_alphabetic() && (!at_word_start || c.is_ascii_uppercase()) {
            at_word_start = false;
        } else {
            break;
        }
        i += 1;
    }
    // A zone name is at least one character, does not end mid-space, and must be
    // followed immediately by `: `.
    if i > 20 && b[i - 1] != b' ' && i + 1 < b.len() && b[i] == b':' && b[i + 1] == b' ' {
        return &line[i + 2..];
    }
    line
}

/// Replace every `druby://...` URI with `REDACTED_HOST`.
///
/// A URI ends at whitespace or at a character that cannot appear in one on
/// this wire -- quote, angle bracket, or the sentence-final period Lich writes
/// after the endpoint. Reached only for lines that survived
/// `THIRD_PARTY_MARKERS`, which already drops `druby://`; it exists so the
/// redaction does not depend on that ordering, and so a URI reaching this
/// function by another route is still removed.
#[must_use]
pub fn redact_host_uris(line: &str) -> String {
    let needle = "druby://";
    if !line.contains(needle) {
        return line.to_owned();
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find(needle) {
        out.push_str(&rest[..start]);
        out.push_str(REDACTED_HOST);
        let after = &rest[start + needle.len()..];
        let end = after
            .find(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '<' | '>'))
            .unwrap_or(after.len());
        // Trailing sentence punctuation belongs to the prose, not the URI.
        let uri_end = after[..end].rfind(|c: char| c != '.').map_or(0, |i| {
            i + after[i..].chars().next().map_or(1, char::len_utf8)
        });
        rest = &after[uri_end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_lich_timestamp_prefix_is_stripped() {
        // The exact shape, MEASURED at 83,515 occurrences against 0 of any other.
        assert_eq!(
            strip_lich_timestamp("01:46:11: <component id='room objs'>a rock</component>"),
            "<component id='room objs'>a rock</component>"
        );
    }

    #[test]
    fn a_line_with_no_prefix_is_untouched() {
        let line = "<playerID id='966483'/>";
        assert_eq!(strip_lich_timestamp(line), line);
    }

    #[test]
    fn a_timestamp_inside_prose_is_not_stripped() {
        // Anchored at the start of the line and nowhere else. A time in the
        // middle of a sentence is display text the game sent, and removing it
        // would edit the wire rather than unwrap it.
        let line = "You hear: meet me at 01:46:11: sharp";
        assert_eq!(strip_lich_timestamp(line), line);
    }

    #[test]
    fn a_zoned_timestamp_prefix_is_stripped() {
        // The newer Lich build's shape. MEASURED at 4,427,147 occurrences in
        // `dev/lich-5/logs/GSIV-Nisugi/2026/09`, every one line-anchored.
        assert_eq!(
            strip_lich_timestamp(
                "2026-09-01 15:13:58 Central Standard Time: \
                 <component id='room objs'>a rock</component>"
            ),
            "<component id='room objs'>a rock</component>"
        );
    }

    #[test]
    fn a_zoned_prefix_in_any_zone_is_stripped() {
        // Matched as letters-and-spaces rather than against a list, because the
        // author travels and a list would silently stop stripping.
        for zone in [
            "UTC",
            "Pacific Daylight Time",
            "Australian Central Western Standard Time",
        ] {
            let line = format!("2026-09-01 15:13:58 {zone}: <prompt time='1'>&gt;</prompt>");
            assert_eq!(
                strip_lich_timestamp(&line),
                "<prompt time='1'>&gt;</prompt>",
                "zone {zone:?} was not stripped"
            );
        }
    }

    #[test]
    fn a_date_in_prose_does_not_lose_its_sentence() {
        // The falsification this bound exists for. An unbounded
        // letters-and-spaces scan would eat the start of any sentence that
        // happened to follow a timestamp.
        let line = "2026-09-01 15:13:58 is when the caravan leaves: be there.";
        assert_eq!(
            strip_lich_timestamp(line),
            line,
            "a zone name must be followed by `: ` immediately, not eventually"
        );
    }

    #[test]
    fn a_blank_zone_is_not_a_prefix() {
        // The `b[i - 1] != b' '` guard. Without it a "zone" consisting of
        // nothing but spaces satisfies the scan and the line is stripped --
        // MEASURED: dropping that condition left the other 15 tests GREEN, so
        // this case is the only thing enforcing it (plan/05 §0).
        for line in [
            "2026-09-01 15:13:58  : <a>x</a>",
            "2026-09-01 15:13:58 Central Standard Time : <a>x</a>",
        ] {
            assert_eq!(
                strip_lich_timestamp(line),
                line,
                "a zone name must abut its colon"
            );
        }
    }

    #[test]
    fn a_zone_longer_than_the_bound_is_left_alone() {
        // Better to leave a prefix in (a visible, fixable fixture defect) than
        // to eat 60 characters of game text (a silent one).
        let long = "A".repeat(60);
        let line = format!("2026-09-01 15:13:58 {long}: text");
        assert_eq!(strip_lich_timestamp(&line), line);
    }

    #[test]
    fn a_zoned_prefix_is_only_stripped_at_the_line_start() {
        // Same anchoring rule as the bare form: a timestamp mid-sentence is
        // display text the game sent.
        let line = "You hear: 2026-09-01 15:13:58 Central Standard Time: go";
        assert_eq!(strip_lich_timestamp(line), line);
    }

    #[test]
    fn a_near_miss_shape_is_not_stripped() {
        // Without the third colon it is not Lich's prefix. MEASURED: zero lines
        // in the archive take that form, so anything shaped like it is content.
        assert_eq!(
            strip_lich_timestamp("01:46:11 something"),
            "01:46:11 something"
        );
        // And a short line cannot be a prefix at all.
        assert_eq!(strip_lich_timestamp("01:46:1"), "01:46:1");
    }

    #[test]
    fn scrub_unwraps_the_prefix_before_the_other_rewrites() {
        // The ordering that matters: every rewrite below the unwrap must see the
        // wire, not a decorated copy of it.
        let s = Scrubber::new();
        assert_eq!(
            s.scrub(
                "01:46:11: <a>x</a>
"
            ),
            "<a>x</a>
"
        );
    }

    use super::*;

    #[test]
    fn host_uri_is_redacted_with_its_address_and_port() {
        // Shape VERIFIED in the corpus:
        // GSIV-Nisugi/2025/01/xml/2025-01-07_19-35-17.xml
        // RFC 3849 documentation address, not a real one -- see the module docs.
        let line = "Lich is at druby://2001:db8::1%12:50087. Go.";
        let out = redact_host_uris(line);
        // **`2001:db8`, the address actually in the input.** This asserted
        // `!contains("fe80")`, which the input never held -- it could not
        // fail, and the port assertion below was the only one working
        // (review PR-7). The real corpus carries link-local `fe80::` URIs;
        // the fixture uses the RFC 3849 documentation prefix, so the
        // assertion has to name what the fixture has.
        assert!(!out.contains("2001:db8"), "address survived: {out}");
        assert!(!out.contains("::1"), "address tail survived: {out}");
        assert!(!out.contains("50087"), "port survived: {out}");
        assert!(out.ends_with(". Go."), "prose was eaten: {out}");
    }

    #[test]
    fn a_line_carrying_a_host_uri_is_dropped_entirely() {
        // Redaction alone is not enough: the line is a group whisper, so the
        // sentence around the URI is a third party's words.
        let scrubber = Scrubber::new();
        let out =
            scrubber.scrub("keep me\nsomeone whispers, \"druby://fe80::1%1:1\"\nkeep me too\n");
        assert_eq!(out, "keep me\nkeep me too\n");
    }

    #[test]
    fn the_same_name_always_gets_the_same_pseudonym() {
        // This is what makes exist=-to-name correlation still testable.
        let mut scrubber = Scrubber::new();
        scrubber.pseudonymise("Inochi", "Alderin");
        let out = scrubber.scrub(
            "<component id='room players'>Also here: \
             <a exist=\"-11047747\" noun=\"Inochi\">Inochi</a></component>\n",
        );
        assert!(!out.contains("Inochi"), "name survived: {out}");
        assert_eq!(out.matches("Alderin").count(), 2, "both sites: {out}");
        assert!(out.contains("exist=\"-11047747\""), "id was eaten: {out}");
    }

    #[test]
    fn vellum_images_are_stripped_because_the_game_never_sends_them() {
        let line = "before<vellumImg src='sunset' rows='4' align='left'/>after";
        assert_eq!(strip_vellum_images(line), "beforeafter");
    }

    #[test]
    fn crlf_is_normalised_and_the_output_ends_in_a_newline() {
        let scrubber = Scrubber::new();
        let out = scrubber.scrub("a\r\nb\r\n");
        assert_eq!(out, "a\nb\n");
    }

    #[test]
    fn a_clean_line_is_returned_unchanged() {
        // The fast paths must not rewrite ordinary wire traffic.
        let line = "<prompt time=\"1764475407\">&gt;</prompt>";
        let scrubber = Scrubber::new();
        assert_eq!(scrubber.scrub(line), format!("{line}\n"));
    }
}
