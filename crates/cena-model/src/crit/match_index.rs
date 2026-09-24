//! The first-word bucket index that makes matching 2,394 patterns per line
//! affordable, and the one pattern Rust's `regex` cannot compile.
//!
//! Split from `crit.rs` under Rule 4.1 (`plan/05:352-353`).
//!
//! # The index, and why it is not `RegexSet`
//!
//! `plan/06:177` warns in as many words: "Never port a performance
//! conclusion; port the discipline of measuring, then measure *here*." So the
//! Lich conclusion was not ported -- it was re-measured in Rust, on these
//! 2,394 patterns, and it happens to survive. Per call, best of 200:
//!
//! ```text
//! line          naive-all    RegexSet     RegexSet(64MiB)   bucketed
//! HIT-first      50.186us    656.274us     17.420us          2.227us
//! HIT-mid        42.186us    659.249us      9.164us          3.134us
//! MISS-item      59.563us    762.744us      8.306us          3.656us
//! MISS-speech   117.734us      1.0697ms     7.756us         11.641us
//! ```
//!
//! `RegexSet` is the slowest of the three at this scale, and the reason is its
//! lazy-DFA cache thrashing: rebuilding the same set with a 64 MiB
//! `dfa_size_limit` is 30-80x faster. Even then, bucketing wins on three of
//! the four lines.
//!
//! # UNVERIFIED, and the argument beside it was wrong
//!
//! **The table above cannot be reproduced from this repository.** There is no
//! bench, no command, and (when this was written) no `RegexSet` code anywhere:
//!
//! ```text
//! $ grep -rn 'RegexSet' crates/ --include=*.rs
//! (comments only -- this block and Cargo.toml's note)
//! ```
//!
//! `plan/05` §-2 says a finding without proof is speculation and that a
//! number must carry the command that produced it. These numbers carry
//! neither, so they are labelled **UNVERIFIED** rather than quietly trusted
//! (review MO-7). They are kept rather than deleted because they record a
//! real comparison someone ran and the decision that came out of it; what is
//! removed is the pretence that they are checkable.
//!
//! This block also argued that the DFA cache is "**per session**, and
//! `plan/12` binds 3-25 sessions in one process, so 64 MiB each is
//! disqualifying". **That is not how `regex` allocates.** A cache belongs to
//! a `Regex` (or `RegexSet`) and is pooled per concurrently-matching thread,
//! not per session. With the tables behind an `Arc`, the cost is roughly
//! workers x limit, not sessions x limit -- and 3-25 sessions on a handful of
//! runtime threads is a much smaller number than the argument assumed.
//!
//! The same pooling applies to the **2,394 separate `Regex` values this code
//! does use**, whose aggregate cache footprint has never been measured at
//! all. So the memory argument, as stated, was evidence against the option it
//! rejected and silent about the option it chose.
//!
//! **The decision still stands**: bucketing wins three of the four lines
//! outright, and it is simpler than tuning a `dfa_size_limit`. It is the
//! rationale that was wrong, which is worth separating -- a right answer held
//! for a wrong reason survives until the reason is load-bearing somewhere
//! else.
//!
//! # 2026-09-20: a set over the residual, measured
//!
//! The buckets stay. What changed is the residual list: ~195 unanchored
//! patterns tried one by one on every call, which was most of the cost.
//! They now sit behind one `RegexSet` with a raised DFA budget. MEASURED
//! (release, warm, every line of the 85 combat replay blobs):
//! `CritTables::parse` went **17-20us to 1.6us per call**. The set only
//! nominates; `entry_matches` still decides, so the veto is untouched.
//! `tests/combat_gate.rs` compares the result with a full scan of all 2,394
//! entries on every real fixture line, beside `tests/crit_index.rs`'s
//! synthesised ones.
//!
//! # The index is exact, not an approximation
//!
//! A pattern anchored `^Word` can only match a line whose first word is
//! `Word`, so bucketing by that word cannot lose a match. Patterns not so
//! anchored go in a residual list checked on every line.
//!
//! # What is actually verified, and what is not
//!
//! This claimed "VERIFIED by exhaustive comparison: for all 2,394 patterns
//! literalised into lines ... **0 disagreements / 2394**". Two problems
//! (review MO-5):
//!
//! * **2,394 patterns cannot all be literalised.** 45 of them carry an
//!   unescaped `(`, `)`, `|`, `{`, `}` or `$`, or are unanchored, and the
//!   test's `synthesise_matching_line` returns `None` for exactly those. The
//!   comparison covered 2,349.
//!
//!   (The review said 46, counting a pattern whose parentheses are
//!   **escaped** -- `^Burn exposes the spine \(from the front\).` -- which
//!   the synthesiser handles. A character-class scan over the data cannot see
//!   the escape. `tests/crit_index.rs` pins the figure by running the
//!   function rather than by approximating it.)
//! * **No bucketed-versus-full-scan comparison existed in the repo.** The
//!   claim described a one-off exercise, in the voice of a standing
//!   guarantee, with nothing to re-run.
//!
//! Both are closed. `tests/crit_index.rs` now asserts the skip count is
//! exactly 45, so a pattern becoming unliteralisable is a visible change
//! rather than a silent widening of the gap, and it compares the index
//! against a full scan over every entry it CAN synthesise.
//!
//! The 45 remain checked by shape rather than by match: `crit.rs`'s loader
//! compiles every pattern, and `crit_parity.rs`'s digest covers every field of
//! every entry. What no test can do is synthesise a line for a pattern whose
//! language is not a single literal -- that needs a regex-to-string generator,
//! which is a larger thing than the gap it would close.
//!
//! Measured shape (`tests/crit_index.rs`, which asserts each): **454 buckets,
//! 195 residual**, largest bucket 72. This doc said 455/194, contradicting a
//! passing test eight lines of code away.

use regex::{Regex, RegexSet, RegexSetBuilder};

/// The one pattern in the corpus that Rust's `regex` crate cannot compile, and
/// the rewrite that makes it compilable.
///
/// `slash/head/3` is written
/// `/^(?!.*removes skull.)Blow to head./`
/// (`reference/lich-5/lib/<game>/critranks/slash_critical_table.rb:91`).
/// Rust's `regex` has no lookaround by design -- it guarantees linear time,
/// which lookaround forbids.
///
/// The lookahead exists for exactly one reason: to stop slash/head/3 stealing
/// impact/head's message, which is `/^Blow to head removes skull./`
/// (`impact_critical_table.rb:205`). So it is a *negative* condition on the
/// whole line and can be evaluated separately from the match, which is what
/// `EXCLUSIONS` does.
///
/// **Equivalence proven against Ruby, not assumed.** Both forms were run
/// against the real Ruby regex on 10 adversarial lines -- including
/// `"Blow to headache."` (both true, the `.` is any-char) and
/// `"Blow to head removes skullcap."` (both false) -- with **0 disagreements /
/// 10**.
///
/// Rejected: the `fancy-regex` crate. A second regex engine, with backtracking
/// and no linear-time guarantee, for one entry out of 2,394, is exactly the
/// complexity `plan/05` §-1 says must be paid for by a requirement you can
/// name. A two-line post-filter that is provably equivalent is the simplest
/// thing that works.
const EXCLUSIONS: [(&str, &str); 1] = [(
    "(?!.*removes skull.)",
    // The line this pattern must NOT match, as an ordinary regex. Kept as the
    // lookahead's own body so the relationship is legible.
    ".*removes skull.",
)];

/// The residual set's lazy-DFA budget. The header's (unverified) table blames
/// `RegexSet`'s default cache for a 30-80x slowdown at 2,394 patterns; at 195
/// the question is smaller, and the number is generous rather than tuned.
const RESIDUAL_DFA_LIMIT: usize = 1 << 26;

/// One compiled pattern plus the exclusion its Lich source carried.
struct Compiled {
    regex: Regex,
    /// Evaluated as a veto: if this matches the line, the entry does not,
    /// however well `regex` matched. `None` for 2,393 of 2,394 entries.
    exclusion: Option<Regex>,
}

/// Compiled patterns, indexed by the first literal word of each anchored
/// pattern.
///
/// Not a `static` and not behind a `OnceLock`: an ordinary value, constructed
/// once and shared as `Arc<CritTables>` across sessions. That is what keeps
/// `ALLOWED_STATICS` (plan/05 Rule 5.2, :400-408) empty of crit entries --
/// there is no process global here to justify, because there is no process
/// global.
pub struct MatchIndex {
    compiled: Vec<Compiled>,
    /// `(first word, entry indices)`, sorted by word for binary search.
    buckets: Vec<(String, Vec<usize>)>,
    /// Entries whose pattern is not anchored to a literal first word. These
    /// are checked on every line.
    residual: Vec<usize>,
    /// One set over the residual patterns, so the ~195 of them cost one pass
    /// rather than 195. `None` if it would not build: the residual list is
    /// then scanned linearly, which is what this did before 2026-09-20.
    residual_gate: Option<RegexSet>,
}

/// A pattern that would not compile, with its index and the regex error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError {
    /// Position of the failing pattern in the slice passed to `build`.
    pub index: usize,
    /// The regex source that failed: the entry's own pattern, or the
    /// `EXCLUSIONS` replacement when it is that half which would not compile.
    pub pattern: String,
    /// The `regex` crate's error, rendered with `to_string`.
    pub message: String,
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "pattern {} ({:?}) does not compile: {}",
            self.index, self.pattern, self.message
        )
    }
}

impl std::error::Error for PatternError {}

impl MatchIndex {
    /// An index over no patterns, which matches nothing.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            compiled: Vec::new(),
            buckets: Vec::new(),
            residual: Vec::new(),
            residual_gate: None,
        }
    }

    /// Compile every pattern and build the index.
    ///
    /// Eager, not lazy. Measured at ~94ms for all 2,394 patterns, paid once
    /// and shared by every session; a lazy per-bucket compile is available if
    /// startup ever matters, but the median bucket is 2 patterns so there is
    /// little to win and a `OnceCell` per bucket to lose.
    ///
    /// # Errors
    ///
    /// Returns `PatternError` naming the index and the pattern if any of them
    /// fails to compile. Exactly one pattern in the shipped data needs a
    /// rewrite first (`EXCLUSIONS`); a second one appearing is a Lich update
    /// that must be handled deliberately rather than dropped.
    pub fn build(patterns: &[String]) -> Result<Self, PatternError> {
        let mut compiled = Vec::with_capacity(patterns.len());
        for (index, pattern) in patterns.iter().enumerate() {
            compiled.push(compile_one(index, pattern)?);
        }

        let mut by_word: std::collections::BTreeMap<String, Vec<usize>> =
            std::collections::BTreeMap::new();
        let mut residual = Vec::new();
        for (index, pattern) in patterns.iter().enumerate() {
            match anchored_first_word(pattern) {
                Some(word) => by_word.entry(word).or_default().push(index),
                None => residual.push(index),
            }
        }

        // The set is over the COMPILED sources, so the one rewritten
        // pattern is gated as rewritten; its veto still runs afterwards.
        let residual_gate = RegexSetBuilder::new(
            residual
                .iter()
                .filter_map(|&i| compiled.get(i))
                .map(|c: &Compiled| c.regex.as_str()),
        )
        .dfa_size_limit(RESIDUAL_DFA_LIMIT)
        .build()
        .ok();

        Ok(Self {
            compiled,
            buckets: by_word.into_iter().collect(),
            residual,
            residual_gate,
        })
    }

    /// The indices of every pattern matching `line`, ascending.
    ///
    /// `line` is matched with both ends stripped, as Lich does
    /// (`reference/lich-5/lib/<game>/critranks.rb:112` is `line.strip`).
    /// Leading whitespace matters because the patterns are `^`-anchored;
    /// TRAILING whitespace matters because 2,379 of the 2,394 patterns end
    /// in an unescaped `.`, which a trailing space would satisfy. Trimming
    /// only the front made `"Blow to head "` match where Ruby rejects it.
    #[must_use]
    pub fn matches(&self, line: &str) -> Vec<usize> {
        let line = line.trim();
        let word = leading_word(line);

        let mut hits = Vec::new();
        if let Ok(position) = self
            .buckets
            .binary_search_by(|(w, _)| w.as_str().cmp(&word))
        {
            for &index in &self.buckets[position].1 {
                if self.entry_matches(index, line) {
                    hits.push(index);
                }
            }
        }
        match &self.residual_gate {
            Some(gate) => {
                for index in gate
                    .matches(line)
                    .into_iter()
                    .filter_map(|i| self.residual.get(i).copied())
                {
                    if self.entry_matches(index, line) {
                        hits.push(index);
                    }
                }
            }
            None => {
                for &index in &self.residual {
                    if self.entry_matches(index, line) {
                        hits.push(index);
                    }
                }
            }
        }
        hits.sort_unstable();
        hits
    }

    fn entry_matches(&self, index: usize, line: &str) -> bool {
        let Some(entry) = self.compiled.get(index) else {
            return false;
        };
        if !entry.regex.is_match(line) {
            return false;
        }
        // The veto. `None` for every entry but slash/head/3.
        !entry
            .exclusion
            .as_ref()
            .is_some_and(|excluded| excluded.is_match(line))
    }

    /// How many patterns carry an exclusion.
    ///
    /// The parity test asserts this is exactly 1, so a second unsupported
    /// pattern arriving in a future Lich update goes red rather than being
    /// quietly rewritten.
    #[must_use]
    pub fn exclusion_count(&self) -> usize {
        self.compiled
            .iter()
            .filter(|c| c.exclusion.is_some())
            .count()
    }

    /// Did the residual set build? `tests/combat_gate.rs` holds it true, so
    /// a silent fall back to the linear scan is a red test, not a slow hunt.
    #[must_use]
    pub const fn residual_is_gated(&self) -> bool {
        self.residual_gate.is_some()
    }

    /// `(bucket count, residual count, largest bucket)`, for the test that
    /// keeps the index's measured shape honest.
    #[must_use]
    pub fn shape(&self) -> (usize, usize, usize) {
        (
            self.buckets.len(),
            self.residual.len(),
            self.buckets.iter().map(|(_, v)| v.len()).max().unwrap_or(0),
        )
    }
}

fn compile_one(index: usize, pattern: &str) -> Result<Compiled, PatternError> {
    let mut source = pattern.to_owned();
    let mut exclusion = None;
    for (lookahead, excluded) in EXCLUSIONS {
        if source.contains(lookahead) {
            source = source.replace(lookahead, "");
            let compiled = Regex::new(excluded).map_err(|e| PatternError {
                index,
                pattern: excluded.to_owned(),
                message: e.to_string(),
            })?;
            exclusion = Some(compiled);
        }
    }
    let regex = Regex::new(&source).map_err(|e| PatternError {
        index,
        pattern: pattern.to_owned(),
        message: e.to_string(),
    })?;
    Ok(Compiled { regex, exclusion })
}

/// The literal first word of a `^`-anchored pattern, lowercased.
///
/// `None` when the pattern is not anchored, or is anchored to something that
/// is not a run of letters -- those go to the residual list. Mirrors Lich's
/// own rule at `reference/lich-5/lib/<game>/critranks.rb:96-101`, including
/// the apostrophe, which appears in contractions.
fn anchored_first_word(pattern: &str) -> Option<String> {
    let rest = pattern.strip_prefix('^')?;
    let word = leading_word(rest);
    if word.is_empty() {
        return None;
    }
    // A quantifier on the last letter means the first word is NOT fixed, so no
    // single bucket key describes it, and such patterns must fall through to
    // the residual list (scanned on every line).
    //
    // The one case today is `^Scaldl?ing blast peels ...` (steam/back/6). The
    // `l?` is deliberate: GemStone emits "Scaldling", which is a typo in the
    // game, and the optional `l` makes the entry match both that and the
    // "Scalding" it will emit if Simutronics ever fixes it. Bucketing keyed it
    // on "scaldl" -- a word neither spelling starts with -- so the entry was
    // unreachable for BOTH. Routing it here is what makes the hedge work.
    if matches!(
        rest[word.len()..].chars().next(),
        Some('?' | '*' | '{' | '|')
    ) {
        return None;
    }
    Some(word)
}

/// The leading run of letters and apostrophes, lowercased.
fn leading_word(text: &str) -> String {
    text.chars()
        .take_while(|c| c.is_ascii_alphabetic() || *c == '\'')
        .collect::<String>()
        .to_lowercase()
}
