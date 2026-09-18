//! Tier 2: replay over the real corpus. **Gated**, and skips cleanly.
//!
//! The corpus is `E:\Gemstone\data\log archive` -- 10,849 `.xml` files,
//! 49.55 GB, Oct 2024 to Sep 2026, 50+ characters. It is two years of real
//! wire traffic and it is not in the repository, so this tier runs only when
//! `CENA_CORPUS` points at it. Unset, every test here returns early and
//! reports why: CI and other machines stay green without the archive.
//!
//! ```text
//! $ CENA_CORPUS='E:\Gemstone\data\log archive' cargo test -p cena-protocol --test corpus_replay
//! ```
//!
//! # What it asserts, and what it deliberately does not
//!
//! **Asserts:** no panic, no [`Frame::MalformedTag`], and no
//! [`Frame::UnknownTag`] whose name is absent from [`cena_protocol::tags`].
//! Those three are the Rule 2.2 contract.
//!
//! **Does not assert output equality.** A golden over 49 GB would be
//! unmaintainable, and regenerating it would hide the regressions it was meant
//! to catch. Tier 1 owns meaning; this tier owns "never breaks on real
//! traffic".
//!
//! **Does not assert stream-stack balance.** The corpus measures 19,142
//! `pushStream` against 14,714 `popStream` (1.301) because `pushStream` is not
//! a stack discipline, so asserting balance would fail on correct input. What
//! is asserted instead is that the prompt barrier drains it.
//!
//! # The `.log` files are not fixtures
//!
//! The archive also holds 11,862 `.log` files in a **tag-stripped** format.
//! They are not wire traffic and must never be used as protocol fixtures, so
//! this walk takes `.xml` only.

use cena_protocol::frame::Frame;
use cena_protocol::{Parser, tags};
use std::path::{Path, PathBuf};

/// The env var that points at the archive.
const CORPUS_VAR: &str = "CENA_CORPUS";

/// How many files a default run touches.
///
/// The full 49.55 GB is not what anyone wants from `cargo test`. This is a
/// deterministic stratified slice; `CENA_CORPUS_FILES=0` means all of them.
const DEFAULT_FILE_BUDGET: usize = 60;

/// The corpus root, or `None` when the tier is not enabled.
fn corpus_root() -> Option<PathBuf> {
    let raw = std::env::var(CORPUS_VAR).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    assert!(
        path.is_dir(),
        "{CORPUS_VAR} is set to {} but that is not a directory. An enabled \
         gate that cannot find the corpus must fail loudly -- silently \
         skipping would make this tier vacuous exactly when someone meant to \
         run it.",
        path.display()
    );
    Some(path)
}

/// Print why the tier did not run, so a skip is never mistaken for a pass.
fn skipped() {
    // The example path is NOT spelled out here: the archive directory carries
    // the game's name, and `game_names_outside_game_modules_are_flagged`
    // (Rule 3.4, plan/05:332-336) scans this file. That test caught this
    // literal, which is the drift case it exists for. The module doc comment
    // has the full path; comments are stripped before that scan.
    println!(
        "SKIPPED: {CORPUS_VAR} is not set, so the corpus replay did not run. \
         Set it to the log archive directory (see this file's module docs for \
         the path) to enable this tier."
    );
}

/// How many files to read this run.
fn file_budget() -> usize {
    std::env::var("CENA_CORPUS_FILES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_FILE_BUDGET)
}

/// Every `.xml` under `root`, sorted, so a run is reproducible.
fn xml_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "xml") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// A deterministic spread across the whole archive, not the first N files.
///
/// Taking `head` would read one character's logs from one month. Every *n*th
/// path over a sorted list crosses characters and years, which is where the
/// protocol actually varies.
fn stratified(files: &[PathBuf], budget: usize) -> Vec<&PathBuf> {
    if budget == 0 || files.len() <= budget {
        return files.iter().collect();
    }
    let step = files.len() / budget;
    files.iter().step_by(step.max(1)).take(budget).collect()
}

/// Is this "tag name" actually prose that happened to sit in angle brackets?
///
/// The game's own help text writes commands with angle-bracket placeholders:
///
/// ```text
/// To change a skill, enter this command:  <skill#> +/- <#ranks>
/// To change your score, enter this command:  <stat name> +/- <#>
/// ```
///
/// VERIFIED in `GST-Tedore/2026/08/xml/2026-08-22_02-03-36.xml` and
/// `GST-Nerten/2026/09/xml/2026-09-03_14-20-49.xml`. An XML name may not be
/// empty, may not contain `#`, and may not contain a space, so none of these
/// can be an element however the wire meant them.
///
/// `Frame::UnknownTag` carrying them to the user as text is the **correct**
/// outcome under Rule 2.2, so this tier does not count them as vocabulary
/// gaps. Note the parser is still right to emit `UnknownTag` here: it cannot
/// know the author's intent, and text is what the user needs to see.
fn is_prose_not_an_element(name: &str, raw: &str) -> bool {
    // The raw tag decides. An element's attributes are `key='value'` pairs, so
    // a body with whitespace and no `=` is prose: `<stat name>` has a space
    // and no attribute, while `<nav rm='1'/>` has both.
    let body = raw
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end_matches('/');
    let prose_body = body.contains(char::is_whitespace) && !body.contains('=');
    name.is_empty()
        || name.contains('#')
        || raw.contains('#')
        || prose_body
        || name
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphabetic() && c != '_')
}

/// A corpus line, with the Lich header and CRLF handled.
fn replay_file(path: &Path) -> Result<Findings, std::io::Error> {
    let bytes = std::fs::read(path)?;
    let mut parser = Parser::new();
    let mut findings = Findings::default();
    for frame in parser.push_bytes(&bytes) {
        findings.count(&frame, path);
    }
    // Flush any unterminated trailing line.
    for frame in parser.push_bytes(b"\n") {
        findings.count(&frame, path);
    }
    Ok(findings)
}

/// What a replay turned up.
#[derive(Default)]
struct Findings {
    frames: usize,
    /// Tag names absent from `KNOWN_WIRE_TAGS`, with the file they came from.
    novel_tags: Vec<(String, String)>,
    /// Unterminated tags, with the file.
    malformed: Vec<(String, String)>,
}

impl Findings {
    fn count(&mut self, frame: &Frame, path: &Path) {
        self.frames += 1;
        let file = path.display().to_string();
        match frame {
            // The RAW tag is what decides, not the extracted name: `tag_name`
            // stops at whitespace, so `<stat name>` arrives here as `stat` and
            // the prose filter would never see the space that proves it is not
            // markup. Judged on the name alone, `<stat name>` looks like a new
            // protocol element; judged on the raw bytes it is plainly help
            // text. This cost a red run to notice.
            Frame::UnknownTag { name, raw } if !tags::is_known(name) => {
                if !is_prose_not_an_element(name, raw) {
                    self.novel_tags.push((name.clone(), file));
                }
            }
            Frame::MalformedTag { raw } => {
                self.malformed.push((raw.clone(), file));
            }
            _ => {}
        }
    }

    fn merge(&mut self, other: Self) {
        self.frames += other.frames;
        self.novel_tags.extend(other.novel_tags);
        self.malformed.extend(other.malformed);
    }
}

#[test]
fn the_corpus_replays_without_panicking_or_meeting_an_unknown_tag() {
    let Some(root) = corpus_root() else {
        skipped();
        return;
    };
    let files = xml_files(&root);
    assert!(
        !files.is_empty(),
        "{CORPUS_VAR} points at {} but it holds no .xml files. The .log files \
         there are a tag-stripped format and are not wire traffic.",
        root.display()
    );

    let budget = file_budget();
    let chosen = stratified(&files, budget);
    let mut findings = Findings::default();
    let mut read = 0usize;
    for path in &chosen {
        match replay_file(path) {
            Ok(f) => {
                findings.merge(f);
                read += 1;
            }
            Err(e) => panic!("corpus file {} could not be read: {e}", path.display()),
        }
    }

    println!(
        "corpus replay: {read} of {} files ({} in the archive), {} frames",
        chosen.len(),
        files.len(),
        findings.frames
    );

    assert!(read > 0, "no corpus file was read");
    assert!(
        findings.frames > 0,
        "the corpus produced zero frames, so this test proved nothing"
    );

    // Novel tags are the real finding: a name the wire sent that the ported
    // vocabulary does not know.
    //
    // Prose in angle brackets was already excluded as the frames were
    // counted, by `is_prose_not_an_element`, which judges the raw tag. That
    // exclusion is not a loophole -- it is Rule 2.2 working: the game's help
    // text writes `<skill#> +/- <#ranks>` and `<stat name> +/- <#>`, and those
    // must reach the user as text, which is exactly what Frame::UnknownTag
    // does with them.
    let mut names: Vec<&(String, String)> = findings.novel_tags.iter().collect();
    names.sort();
    names.dedup_by(|a, b| a.0 == b.0);
    assert!(
        names.is_empty(),
        "the corpus carries {} tag name(s) absent from KNOWN_WIRE_TAGS. Each \
         is either a real protocol addition (add it to the table) or a parser \
         bug. First few: {:#?}",
        names.len(),
        names.iter().take(10).collect::<Vec<_>>()
    );

    assert!(
        findings.malformed.is_empty(),
        "the corpus is real traffic and measured 0 unterminated tags in a \
         5,079,826-tag sample, so a MalformedTag here means the reassembly is \
         wrong, not the wire. First few: {:#?}",
        findings.malformed.iter().take(5).collect::<Vec<_>>()
    );
}

#[test]
fn the_prompt_barrier_drains_the_stream_stack_on_real_traffic() {
    // NOT stream-stack balance: the corpus is genuinely unbalanced (1.301
    // push:pop) because pushStream is not a stack. What must hold is that a
    // prompt closes whatever is open, which is what bounds corruption to one
    // round.
    let Some(root) = corpus_root() else {
        skipped();
        return;
    };
    let files = xml_files(&root);
    let chosen = stratified(&files, file_budget().clamp(1, 12));

    // The invariant is "the prompt drains whatever was open", and it is
    // asserted by replaying the frame stream and checking the stack depth
    // immediately after each prompt's own `StreamPopForced` run -- not at the
    // next frame, which may already have pushed a new stream.
    //
    // VERIFIED that two stronger readings are both false on real traffic:
    //
    // - "the stack is empty when the prompt arrives".
    //   `GSIV-Nisugi/2024/11/.../2024-11-14_14-07-54.xml:59` sends
    //   `<pushStream id='inv'/>Your worn items are:` and the next stream event
    //   is a `<prompt>` with no pop of any kind. Draining it is the barrier's
    //   entire purpose.
    // - "the stack is empty at the frame after the prompt".
    //   `GSIV-Nisugi/2025/04/.../2025-04-18_11-57-38.xml` sends
    //   `<pushStream id="thoughts"/>[OOC] ...` with no pop, so the frame after
    //   a prompt is routinely inside a freshly pushed stream.
    let mut prompts = 0usize;
    let mut drains = 0usize;
    for path in chosen {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut parser = Parser::new();
        let frames = parser.push_bytes(&bytes);
        let mut depth = 0usize;
        let mut in_barrier = false;
        for frame in &frames {
            match frame {
                Frame::StreamPush { .. } => {
                    // A push ends the barrier's run of forced pops.
                    in_barrier = false;
                    depth += 1;
                }
                Frame::StreamPop { .. } => {
                    in_barrier = false;
                    depth = depth.saturating_sub(1);
                }
                Frame::StreamPopForced { .. } => {
                    depth = depth.saturating_sub(1);
                }
                Frame::Prompt { .. } => {
                    prompts += 1;
                    in_barrier = true;
                }
                _ if in_barrier => {
                    // First non-stream frame after the prompt: the barrier's
                    // forced pops have all been emitted by now, so nothing
                    // may still be open. This is what bounds a torn stream to
                    // one round instead of leaking into the next room.
                    assert_eq!(
                        depth,
                        0,
                        "in {}, a prompt left {depth} stream(s) open. The \
                         barrier must drain the stack, which is what makes the \
                         measured 1.301 push/pop imbalance harmless.",
                        path.display()
                    );
                    drains += 1;
                    in_barrier = false;
                }
                _ => {}
            }
        }
    }
    assert!(
        drains > 0,
        "no prompt drain was ever observed, so this test proved nothing"
    );
    println!("prompt barrier: {drains} drains observed");
    assert!(
        prompts > 0,
        "no prompt was seen in the sampled corpus files, so this test proved \
         nothing"
    );
    println!("prompt barrier verified across {prompts} prompts");
}

#[test]
fn the_gate_itself_is_wired_correctly() {
    // Proves the SKIP path is a real branch rather than a test that always
    // passes: the two states are distinguishable and this says which one ran.
    if let Some(root) = corpus_root() {
        let files = xml_files(&root);
        println!(
            "gate ENABLED: {} holds {} .xml files",
            root.display(),
            files.len()
        );
        assert!(!files.is_empty());
    } else {
        skipped();
        // The var really is absent, not empty-but-set-and-mistaken.
        assert!(
            std::env::var(CORPUS_VAR).is_ok_and(|v| v.trim().is_empty())
                || std::env::var(CORPUS_VAR).is_err(),
            "corpus_root() returned None while {CORPUS_VAR} holds a value"
        );
    }
}
