//! Item structure recovered from tokens: fields, statics, functions and
//! attributes, each with the scope it sits in.
//!
//! Mechanism only; it states no rule. Moved down out of the rule files under
//! `plan/05:352-353` rather than grown inside them.
//!
//! # Why a scope walk and not another line needle
//!
//! Review findings 1, 2, 5, 7 and 11 were one defect in five places: a rule
//! asked a question about STRUCTURE -- "is this a field of a struct", "which
//! function is this static in", "is this test compiled out by its module" --
//! and answered it by looking at one line. Each answer was right for the
//! spelling its author imagined and wrong for the next one:
//!
//! - `pub(crate) handle: SessionHandle,` is a field; a needle anchored on
//!   `handle: SessionHandle,` at line start did not see it.
//! - `pub struct` opened no struct body for a walk-back that knew only
//!   `struct `, so any `fn` above it dropped the hit.
//! - `fn f() -> &'static R { static R: ... }` was skipped outright because the
//!   line contained `fn`.
//! - `#[cfg(any())] mod off { #[test] fn t() {} }` compiled a test out while a
//!   check of the attributes directly above `fn t` saw only `#[test]`.
//!
//! A scope stack answers all four the same way, from the same tokens.
//! It is still not a parser: macro bodies are walked as if they were code, and
//! a type is recovered as text rather than resolved.

use crate::lexical::{Token, TokenKind, tokens};

/// A field of a struct, a union, or an enum variant -- named or positional.
#[derive(Clone, Debug)]
pub struct Field {
    /// 1-indexed line the field starts on.
    pub line: usize,
    /// The field's name, or its position (`"0"`) in a tuple struct/variant.
    pub name: String,
    /// The declared type as written, spaces only where two words meet.
    pub ty: String,
    /// The struct, union or variant that declares it.
    pub container: String,
    /// The enum, when `container` is one of its variants.
    pub enum_name: Option<String>,
}

/// A `static` item, wherever it is declared.
#[derive(Clone, Debug)]
pub struct Static {
    /// 1-indexed line of the `static` keyword.
    pub line: usize,
    /// The static's identifier.
    pub name: String,
    /// Whether it is `static mut`.
    pub mutable: bool,
    /// The innermost function whose body declares it; `None` at item level.
    pub function: Option<String>,
}

/// A function declaration: a `fn` keyword followed by a name.
#[derive(Clone, Debug)]
pub struct Function {
    /// 1-indexed line of the `fn` keyword.
    pub line: usize,
    /// The function's name.
    pub name: String,
    /// Everything between the previous item boundary and `fn`, attributes
    /// excluded: `pub`, `pub(crate) const`, `async`, ...
    pub qualifiers: String,
    /// The return type as written, or empty.
    pub returns: String,
    /// The outer attributes directly on this function.
    pub attributes: Vec<Attribute>,
    /// Whether any enclosing scope is conditionally compiled: a `#[cfg(..)]`
    /// on an enclosing `mod`/`impl`/block, or a `#![cfg(..)]` inside one.
    pub gated: bool,
}

/// One `#[...]` or `#![...]` attribute.
#[derive(Clone, Debug)]
pub struct Attribute {
    /// 1-indexed line of the `#`.
    pub line: usize,
    /// `#![...]` rather than `#[...]`.
    pub inner: bool,
    /// The tokens between the brackets.
    pub body: Vec<Token>,
}

impl Attribute {
    /// The first identifier: `cfg`, `allow`, `test`, `path`, `cfg_attr` ...
    pub fn head(&self) -> &str {
        self.body.first().map_or("", |t| t.text.as_str())
    }

    /// Whether any identifier in the body is `ident`, at any depth.
    pub fn mentions(&self, ident: &str) -> bool {
        self.body
            .iter()
            .any(|t| t.kind == TokenKind::Ident && t.text == ident)
    }

    /// The comma-separated items of every `head(...)` group in the body, at
    /// any depth -- so `cfg_attr(x, allow(a, b))` yields `a` and `b` for
    /// `allow`. Each item is its tokens joined as written.
    pub fn lists(&self, head: &str) -> Vec<String> {
        let mut out = Vec::new();
        let body = &self.body;
        for (i, t) in body.iter().enumerate() {
            if !(t.kind == TokenKind::Ident && t.text == head) {
                continue;
            }
            if !body.get(i + 1).is_some_and(|n| n.is("(")) {
                continue;
            }
            let mut depth = 0usize;
            let mut item: Vec<Token> = Vec::new();
            for tok in &body[i + 1..] {
                match tok.text.as_str() {
                    "(" | "[" if tok.kind == TokenKind::Punct => {
                        depth += 1;
                        if depth == 1 {
                            continue;
                        }
                    }
                    ")" | "]" if tok.kind == TokenKind::Punct => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    "," if tok.kind == TokenKind::Punct && depth == 1 => {
                        out.push(join(&item));
                        item.clear();
                        continue;
                    }
                    _ => {}
                }
                item.push(tok.clone());
            }
            if !item.is_empty() {
                out.push(join(&item));
            }
        }
        out
    }

    /// The string value of every `key = "..."` in the body, at any depth --
    /// so `cfg_attr(x, path = "y")` yields `y` for `path`.
    pub fn values(&self, key: &str) -> Vec<String> {
        let body = &self.body;
        (0..body.len())
            .filter(|&i| body[i].kind == TokenKind::Ident && body[i].text == key)
            .filter(|&i| body.get(i + 1).is_some_and(|t| t.is("=")))
            .filter_map(|i| body.get(i + 2)?.string_value().map(str::to_owned))
            .collect()
    }
}

/// Everything [`outline`] recovers from one file.
#[derive(Debug, Default)]
pub struct Outline {
    /// Every field of every struct, union and enum variant.
    pub fields: Vec<Field>,
    /// Every `static`, including one inside a `thread_local!`.
    pub statics: Vec<Static>,
    /// Every named function.
    pub functions: Vec<Function>,
    /// Every attribute, inner and outer.
    pub attributes: Vec<Attribute>,
}

/// What opened a scope.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Root,
    /// The `{}` body of a struct, union or enum variant.
    Named,
    /// The `()` body of a tuple struct or tuple variant.
    Tuple,
    /// The `{}` body of an enum.
    EnumBody,
    /// A function body.
    Fn,
    /// Anything else: an impl, a module, a block, a macro, a paren group.
    Other,
}

#[derive(Debug)]
struct Scope {
    kind: Kind,
    name: String,
    enum_name: Option<String>,
    function: Option<String>,
    gated: bool,
    /// First token of the segment being accumulated at this depth.
    seg_start: usize,
    /// Outer attributes seen since the last boundary, awaiting their item.
    pending: Vec<Attribute>,
    /// `<` depth, for the commas inside `HashMap<K, V>`.
    angle: usize,
    /// Positional index of the next tuple field.
    position: usize,
}

impl Scope {
    fn new(kind: Kind, seg_start: usize) -> Self {
        Self {
            kind,
            name: String::new(),
            enum_name: None,
            function: None,
            gated: false,
            seg_start,
            pending: Vec::new(),
            angle: 0,
            position: 0,
        }
    }
}

/// Walk `text`'s tokens once, recovering fields, statics, functions and
/// attributes with the scopes they sit in.
pub fn outline(text: &str) -> Outline {
    let toks = tokens(text);
    let mut out = Outline::default();
    let mut attr_mask = vec![false; toks.len()];
    let mut stack = vec![Scope::new(Kind::Root, 0)];
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.kind == TokenKind::Punct && t.text == "#" {
            let inner = toks.get(i + 1).is_some_and(|n| n.is("!"));
            let open = if inner { i + 2 } else { i + 1 };
            if toks.get(open).is_some_and(|n| n.is("["))
                && let Some(close) = matching(&toks, open)
            {
                let attr = Attribute {
                    line: t.line,
                    inner,
                    body: toks[open + 1..close].to_vec(),
                };
                attr_mask[i..=close].fill(true);
                let top = stack.last_mut().expect("root scope");
                if inner {
                    top.gated |= attr.head() == "cfg";
                } else {
                    top.pending.push(attr.clone());
                }
                out.attributes.push(attr);
                i = close + 1;
                continue;
            }
        }
        let depth = stack.len();
        let top = stack.last_mut().expect("root scope");
        let kind = top.kind;
        match (t.kind, t.text.as_str()) {
            (TokenKind::Punct, "{" | "(" | "[") => {
                let header: Vec<&Token> = (top.seg_start..i)
                    .filter(|&k| !attr_mask[k])
                    .map(|k| &toks[k])
                    .collect();
                let mut scope = open_scope(&t.text, &header, top);
                scope.seg_start = i + 1;
                scope.gated |= top.gated || top.pending.iter().any(|a| a.head() == "cfg");
                scope.function = if scope.kind == Kind::Fn {
                    Some(scope.name.clone())
                } else {
                    top.function.clone()
                };
                stack.push(scope);
            }
            (TokenKind::Punct, "}" | ")" | "]") if depth > 1 => {
                let closed = stack.pop().expect("len > 1");
                if matches!(closed.kind, Kind::Named | Kind::Tuple) {
                    field(&toks, &attr_mask, &closed, i, &mut out.fields);
                }
                if t.text == "}" {
                    let parent = stack.last_mut().expect("root scope");
                    parent.seg_start = i + 1;
                    parent.pending.clear();
                }
            }
            (TokenKind::Punct, ",") => {
                if matches!(kind, Kind::Named | Kind::Tuple) && top.angle == 0 {
                    field(&toks, &attr_mask, top, i, &mut out.fields);
                    top.position += 1;
                }
                if matches!(kind, Kind::Named | Kind::Tuple | Kind::EnumBody) && top.angle == 0 {
                    top.seg_start = i + 1;
                    top.pending.clear();
                }
            }
            (TokenKind::Punct, ";") => {
                top.seg_start = i + 1;
                top.pending.clear();
            }
            (TokenKind::Punct, "<") if matches!(kind, Kind::Named | Kind::Tuple) => {
                top.angle += 1;
            }
            (TokenKind::Punct, ">") if matches!(kind, Kind::Named | Kind::Tuple) => {
                let arrow = i > 0 && (toks[i - 1].is("-") || toks[i - 1].is("="));
                if !arrow {
                    top.angle = top.angle.saturating_sub(1);
                }
            }
            (TokenKind::Ident, "fn") if next_ident(&toks, i).is_some() => {
                out.functions.push(function_at(&toks, &attr_mask, top, i));
            }
            (TokenKind::Ident, "static") => {
                out.statics.push(static_at(&toks, top, i));
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The function whose `fn` keyword is `toks[i]`, in scope `top`.
fn function_at(toks: &[Token], attr_mask: &[bool], top: &Scope, i: usize) -> Function {
    let qualifiers: Vec<Token> = (top.seg_start..i)
        .filter(|&k| !attr_mask[k])
        .map(|k| toks[k].clone())
        .collect();
    Function {
        line: toks[i].line,
        name: next_ident(toks, i).unwrap_or_default(),
        qualifiers: join(&qualifiers),
        returns: return_type(toks, i),
        attributes: top.pending.clone(),
        gated: top.gated,
    }
}

/// The static whose `static` keyword is `toks[i]`, in scope `top`.
fn static_at(toks: &[Token], top: &Scope, i: usize) -> Static {
    let mutable = toks.get(i + 1).is_some_and(|n| n.is("mut"));
    let at = if mutable { i + 2 } else { i + 1 };
    let named = toks.get(at).filter(|n| n.kind == TokenKind::Ident);
    let typed = toks.get(at + 1).is_some_and(|n| n.is(":"));
    Static {
        line: toks[i].line,
        // An unparseable shape is still reported, never dropped.
        name: named
            .filter(|_| typed)
            .map_or_else(|| "<unparsed>".to_owned(), |n| n.text.clone()),
        mutable,
        function: top.function.clone(),
    }
}

/// Classify the scope a delimiter opens from the tokens before it.
fn open_scope(delimiter: &str, header: &[&Token], parent: &Scope) -> Scope {
    let after = |keyword: &str| {
        header.windows(2).find_map(|w| {
            (w[0].is(keyword) && w[1].kind == TokenKind::Ident).then(|| w[1].text.clone())
        })
    };
    let first_ident = || {
        header
            .iter()
            .find(|t| t.kind == TokenKind::Ident)
            .map(|t| t.text.clone())
    };
    let mut scope = Scope::new(Kind::Other, 0);
    let record = after("struct").or_else(|| after("union"));
    if delimiter == "{" {
        if let Some(name) = record {
            (scope.kind, scope.name) = (Kind::Named, name);
        } else if let Some(name) = after("enum") {
            (scope.kind, scope.name) = (Kind::EnumBody, name);
        } else if parent.kind == Kind::EnumBody {
            scope.kind = Kind::Named;
            scope.name = first_ident().unwrap_or_default();
            scope.enum_name = Some(parent.name.clone());
        } else if let Some(name) = after("fn") {
            (scope.kind, scope.name) = (Kind::Fn, name);
        }
    } else if delimiter == "(" {
        if let Some(name) = record {
            (scope.kind, scope.name) = (Kind::Tuple, name);
        } else if parent.kind == Kind::EnumBody {
            scope.kind = Kind::Tuple;
            scope.name = first_ident().unwrap_or_default();
            scope.enum_name = Some(parent.name.clone());
        }
    }
    scope
}

/// Record the field in `scope`'s current segment, ending before `end`.
fn field(toks: &[Token], mask: &[bool], scope: &Scope, end: usize, out: &mut Vec<Field>) {
    let seg: Vec<&Token> = (scope.seg_start..end)
        .filter(|&k| !mask[k])
        .map(|k| &toks[k])
        .collect();
    let mut k = 0;
    // Visibility: `pub`, `pub(crate)`, `pub(in some::path)`.
    if seg.first().is_some_and(|t| t.is("pub")) {
        k = 1;
        if seg.get(1).is_some_and(|t| t.is("(")) {
            while k < seg.len() && !seg[k].is(")") {
                k += 1;
            }
            k += 1;
        }
    }
    let rest = &seg[k.min(seg.len())..];
    if rest.is_empty() {
        return;
    }
    let (name, ty) = if scope.kind == Kind::Named {
        // `name: Type` -- one colon, not the first half of a `::` path.
        let named = rest.len() > 2
            && rest[0].kind == TokenKind::Ident
            && rest[1].is(":")
            && !rest[2].is(":");
        if !named {
            return;
        }
        (rest[0].text.clone(), &rest[2..])
    } else {
        (scope.position.to_string(), rest)
    };
    let ty: Vec<Token> = ty.iter().map(|t| (*t).clone()).collect();
    out.push(Field {
        line: seg[0].line,
        name,
        ty: join(&ty),
        container: scope.name.clone(),
        enum_name: scope.enum_name.clone(),
    });
}

/// The identifier after `toks[at]`, if there is one.
fn next_ident(toks: &[Token], at: usize) -> Option<String> {
    toks.get(at + 1)
        .filter(|t| t.kind == TokenKind::Ident)
        .map(|t| t.text.clone())
}

/// The tokens after the `->` of the signature starting at `fn_at`, up to its
/// body, its `;` or its `where`.
fn return_type(toks: &[Token], fn_at: usize) -> String {
    let mut depth = 0usize;
    let mut start = None;
    for (k, t) in toks.iter().enumerate().skip(fn_at + 1) {
        if t.kind == TokenKind::Punct {
            match t.text.as_str() {
                "(" | "[" => depth += 1,
                ")" | "]" => depth = depth.saturating_sub(1),
                "{" | ";" if depth == 0 => {
                    return start.map_or_else(String::new, |s| join(&toks[s..k]));
                }
                ">" if depth == 0 && start.is_none() && k > 0 && toks[k - 1].is("-") => {
                    start = Some(k + 1);
                }
                _ => {}
            }
        } else if t.is("where") && depth == 0 {
            return start.map_or_else(String::new, |s| join(&toks[s..k]));
        }
    }
    String::new()
}

/// The index of the bracket closing the one at `open`.
fn matching(toks: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (k, t) in toks.iter().enumerate().skip(open) {
        if t.kind != TokenKind::Punct {
            continue;
        }
        match t.text.as_str() {
            "[" | "(" | "{" => depth += 1,
            "]" | ")" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

/// Tokens as written, with a space only where two words would otherwise run
/// together: `Option<SessionHandle>`, `&'a str`, `pub(crate)`, `mut x`.
pub fn join(toks: &[Token]) -> String {
    let word = |t: &Token| t.kind != TokenKind::Punct;
    let mut out = String::new();
    for (k, t) in toks.iter().enumerate() {
        if k > 0 && word(&toks[k - 1]) && word(t) {
            out.push(' ');
        }
        out.push_str(&t.text);
    }
    out
}

/// Names of the functions in `text` that are **live tests**: marked `#[test]`
/// and not compiled out or ignored by anything around them.
///
/// # Why "declared" was not enough, and then why "the attributes above" was not
///
/// This first collected any line beginning `fn `, which made a withdrawal-side
/// ratchet blind to withdrawal: a reviewer added `#[ignore]` to every test in
/// a rule file and nothing failed (review AR-3). It then checked the attribute
/// lines directly above the `fn` for `#[ignore]` and `#[cfg(`, and four
/// ordinary spellings still compiled a test out while it counted as live
/// (review finding 5), each VERIFIED against that version in a scratch binary:
///
/// - `#![cfg(any())]` at the top of the file -- an INNER attribute;
/// - `#[cfg(any())] mod off { #[test] fn t() {} }` -- the gate is on the
///   enclosing module, several lines up and past a `{`;
/// - `#[cfg_attr(all(), ignore)]` -- `ignore`, applied conditionally;
/// - the same `cfg_attr` split over four lines, which ended the upward walk
///   at `)]` before it reached anything.
///
/// So a test is live when it carries `test` (or `tokio::test`), no enclosing
/// scope is `cfg`-gated, and none of its own attributes is `cfg`, `ignore`,
/// or a `cfg_attr` that can produce either.
pub fn live_test_names(text: &str) -> std::collections::BTreeSet<String> {
    let is_test = |a: &Attribute| {
        let body = join(&a.body);
        body == "test" || body.starts_with("tokio::test")
    };
    let suppresses = |a: &Attribute| match a.head() {
        "cfg" | "ignore" => true,
        "cfg_attr" => a.mentions("ignore") || a.mentions("cfg"),
        _ => false,
    };
    outline(text)
        .functions
        .into_iter()
        .filter(|f| !f.gated)
        .filter(|f| f.attributes.iter().any(is_test))
        .filter(|f| !f.attributes.iter().any(suppresses))
        .map(|f| f.name)
        .collect()
}
