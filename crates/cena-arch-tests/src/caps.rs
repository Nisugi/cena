//! The cap files and the facts they are compared with: `caps.baseline`,
//! `caps.floor`, the live `CapException` table, and which files are split
//! parents.
//!
//! Mechanism only; the ratchets themselves live in `tests/ratchet.rs`. Moved
//! down out of that file under `plan/05:352-353` when hardening the ratchets
//! (review findings 3 and 4) would otherwise have put it past its own cap.

use crate::harness::{relative, workspace_root};
use crate::lexical::{TokenKind, tokens};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// The prefix that marks a per-exception cap in `caps.baseline`, so it is
/// never mistaken for a split parent (whose names are also paths).
pub const EXCEPTION_PREFIX: &str = "exception:";

/// Read a `<name> <value>` cap file beside this crate, ignoring comments and
/// blanks.
pub fn caps_file(file: &str) -> BTreeMap<String, usize> {
    let path = workspace_root()
        .join("crates")
        .join("cena-arch-tests")
        .join(file);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} must exist and be readable: {e}. It is the cap ratchet's \
             baseline -- deleting it would silently disable Rule 4.1's \
             increase check, which is the exact failure the check exists to \
             prevent.",
            path.display()
        )
    });
    let mut caps = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line.split_once(' ').unwrap_or_else(|| {
            panic!("malformed baseline line {line:?}: expected `<name> <value>`")
        });
        let value = value.trim().parse().unwrap_or_else(|e| {
            panic!("malformed baseline value in {line:?}: {e}");
        });
        caps.insert(name.to_owned(), value);
    }
    caps
}

/// Whether a baseline name is a split-parent path (not a scalar, not an
/// exception cap).
pub fn is_split_parent_entry(name: &str) -> bool {
    name.contains('/') && !name.starts_with(EXCEPTION_PREFIX)
}

/// Pull `const NAME: usize = N;` out of source text.
pub fn scan_usize(text: &str, prefix: &str) -> Option<usize> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(prefix))
        .and_then(|rest| rest.trim_end_matches(';').trim().parse().ok())
}

/// Every `CapException { path: "...", cap: N, .. }` literal in `text`, as
/// `(path, cap)`, read from TOKENS.
///
/// The count used to be lines whose trimmed text started `CapException {`, so
/// `CapException{` -- which `#[rustfmt::skip]` preserves -- was not an entry,
/// and the caps themselves were never read at all (review finding 3).
///
/// # Panics
///
/// On an entry whose `cap` is not a plain integer literal (`2 * 2100`, a
/// const). The ratchet compares numbers; an expression it cannot evaluate
/// would be an entry it cannot guard, so it is refused rather than skipped.
pub fn cap_exceptions(text: &str) -> Vec<(String, usize)> {
    let toks = tokens(text);
    let mut out = Vec::new();
    for (k, t) in toks.iter().enumerate() {
        let literal = t.is("CapException")
            && toks.get(k + 1).is_some_and(|n| n.is("{"))
            && !(k > 0 && toks[k - 1].is("struct"));
        if !literal {
            continue;
        }
        let mut path = None;
        let mut cap = None;
        let mut depth = 0usize;
        for (j, tok) in toks.iter().enumerate().skip(k + 1) {
            if tok.is("{") {
                depth += 1;
            } else if tok.is("}") {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            let field = |name: &str| {
                depth == 1 && tok.is(name) && toks.get(j + 1).is_some_and(|n| n.is(":"))
            };
            if field("path") {
                path = toks
                    .get(j + 2)
                    .and_then(|v| v.string_value())
                    .map(str::to_owned);
            } else if field("cap") {
                let value = toks.get(j + 2).filter(|v| v.kind == TokenKind::Number);
                let plain = toks.get(j + 3).is_some_and(|n| n.is(",") || n.is("}"));
                let parsed = value
                    .filter(|_| plain)
                    .and_then(|v| v.text.replace('_', "").parse().ok());
                cap = Some(parsed.unwrap_or_else(|| {
                    panic!(
                        "a CapException cap at line {} is not a plain integer \
                         literal; the ratchet cannot guard what it cannot read",
                        tok.line
                    )
                }));
            }
        }
        let path = path.unwrap_or_else(|| panic!("CapException at line {} has no path", t.line));
        let cap = cap.unwrap_or_else(|| panic!("CapException {path} has no cap"));
        out.push((path, cap));
    }
    out
}

/// Every split parent in `sources` -- a `name.rs` with a sibling `name/`
/// directory holding at least one scanned `.rs` file -- with its line count,
/// keyed by workspace-relative path.
///
/// Discovered, not listed. The list this replaced was the nine parents that
/// existed on 2026-09-19; by the next review fourteen more had been written,
/// among them the largest files in the workspace, and nothing asked about any
/// of them (review finding 4).
pub fn split_parents(sources: &[(PathBuf, String)]) -> BTreeMap<String, usize> {
    let children: Vec<PathBuf> = sources
        .iter()
        .filter(|(p, _)| p.extension().is_some_and(|e| e == "rs"))
        .filter_map(|(p, _)| p.parent().map(PathBuf::from))
        .collect();
    sources
        .iter()
        .filter(|(p, _)| p.extension().is_some_and(|e| e == "rs"))
        .filter(|(p, _)| children.contains(&p.with_extension("")))
        .map(|(p, text)| (relative(p), text.lines().count()))
        .collect()
}

/// The split-parent cap rule `caps.baseline` records: measured size plus 50,
/// rounded up to the next 50 -- and never above `default`, where the ordinary
/// cap already binds.
pub fn suggested_split_parent_cap(lines: usize, default: usize) -> usize {
    (lines + 50).div_ceil(50).saturating_mul(50).min(default)
}

/// How `live` exception caps disagree with `baseline`: raised, unrecorded, or
/// recorded for an exception that no longer exists.
pub fn exception_cap_drift(
    live: &[(String, usize)],
    baseline: &std::collections::BTreeMap<String, usize>,
) -> Vec<String> {
    let mut drift = Vec::new();
    for (path, cap) in live {
        match baseline.get(&format!("{EXCEPTION_PREFIX}{path}")) {
            Some(limit) if cap > limit => {
                drift.push(format!("{path}: cap {cap} is ABOVE the baseline {limit}"));
            }
            Some(_) => {}
            None => drift.push(format!(
                "{path}: cap {cap} has no `{EXCEPTION_PREFIX}{path}` line in caps.baseline"
            )),
        }
    }
    for name in baseline.keys() {
        if let Some(path) = name.strip_prefix(EXCEPTION_PREFIX)
            && !live.iter().any(|(p, _)| p == path)
        {
            drift.push(format!(
                "{name}: in caps.baseline with no CapException -- remove it, or it \
                 pre-authorises the next exception for that path unseen"
            ));
        }
    }
    drift
}
