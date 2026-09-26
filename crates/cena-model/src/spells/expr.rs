//! A small evaluator for the Ruby the spell table's durations and costs are
//! written in (`plan/37` Stage 2).
//!
//! Lich evaluates each `<duration>` and `<cost>` as Ruby against the
//! character. The measured vocabulary is small -- numbers, arithmetic,
//! comparison, `?:` and one-line `if/elsif/else/end`, `.to_i`, `.to_f`,
//! `.round`, `[a, b].min`, and a handful of names (`Spells.minorspiritual`,
//! `Stats.level`, `Skills.slreligion`, `Society.rank`, `Spellsong.timeleft`,
//! `Spell[N].known?`) -- so this parses exactly that and nothing more. The
//! names are resolved by the caller, which has the character. Anything else
//! -- an assignment, a block, a regex, a method this does not know -- is
//! `None`, never a guess.

/// A name the expression asks about, for the caller to resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Name {
    /// `Module.method`, e.g. `Spells.minorspiritual`, `Stats.level`.
    Path(String, String),
    /// `Spell[N].method`, e.g. `Spell[401].known?`.
    Spell(u16, String),
}

/// A value an expression computes.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A number.
    Num(f64),
    /// A truth.
    Bool(bool),
    /// `[a, b]`, for `.min` and `.max`.
    List(Vec<f64>),
}

impl Value {
    fn num(&self) -> Option<f64> {
        match self {
            Self::Num(n) => Some(*n),
            _ => None,
        }
    }

    fn truth(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            // Ruby: every number is true, `nil` and `false` are not.
            Self::Num(_) | Self::List(_) => true,
        }
    }
}

/// Evaluate `source` to a number, asking `resolve` for every name.
#[must_use]
pub fn evaluate(source: &str, resolve: &dyn Fn(&Name) -> Option<Value>) -> Option<f64> {
    let tokens = tokenize(source)?;
    let mut parser = Parser {
        tokens,
        at: 0,
        resolve,
    };
    let value = parser.statement()?;
    if parser.at != parser.tokens.len() {
        return None;
    }
    value.num()
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    /// The number, and its text as written.
    Num(f64, String),
    Ident(String),
    Punct(&'static str),
}

const PUNCT: &[&str] = &[
    ">=", "<=", "==", "!=", "&&", "||", "(", ")", "[", "]", ",", ".", "+", "-", "*", "/", "?", ":",
    ";", ">", "<", "!",
];

fn tokenize(source: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || (chars[i] == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)))
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            out.push(Token::Num(text.parse().ok()?, text));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            // A `?` right after a method's name is the name's: `known?`.
            if chars.get(i) == Some(&'?') && out.last() == Some(&Token::Punct(".")) {
                i += 1;
            }
            out.push(Token::Ident(chars[start..i].iter().collect()));
        } else {
            let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let punct = PUNCT.iter().find(|p| rest.starts_with(**p))?;
            out.push(Token::Punct(punct));
            i += punct.len();
        }
    }
    Some(out)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    at: usize,
    resolve: &'a dyn Fn(&Name) -> Option<Value>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn punct(&mut self, p: &str) -> bool {
        if self.peek() == Some(&Token::Punct(leak(p))) {
            self.at += 1;
            return true;
        }
        false
    }

    fn word(&mut self, w: &str) -> bool {
        if matches!(self.peek(), Some(Token::Ident(i)) if i == w) {
            self.at += 1;
            return true;
        }
        false
    }

    /// `if c; a; elsif c; b; else; c; end`, or an expression. Stray `;`
    /// between parts are Ruby's statement separators and are skipped.
    fn statement(&mut self) -> Option<Value> {
        if !self.word("if") {
            let value = self.expr()?;
            while self.punct(";") {}
            return Some(value);
        }
        let mut chosen: Option<Value> = None;
        loop {
            let cond = self.expr()?.truth();
            self.separator();
            let value = self.expr()?;
            self.separator();
            if cond && chosen.is_none() {
                chosen = Some(value);
            }
            if self.word("elsif") {
                continue;
            }
            if self.word("else") {
                self.separator();
                let value = self.expr()?;
                self.separator();
                if chosen.is_none() {
                    chosen = Some(value);
                }
            }
            if !self.word("end") {
                return None;
            }
            while self.punct(";") {}
            return chosen;
        }
    }

    fn separator(&mut self) {
        while self.punct(";") || self.word("then") {}
    }

    fn expr(&mut self) -> Option<Value> {
        let cond = self.or()?;
        if self.punct("?") {
            let yes = self.expr()?;
            if !self.punct(":") {
                return None;
            }
            let no = self.expr()?;
            return Some(if cond.truth() { yes } else { no });
        }
        Some(cond)
    }

    fn or(&mut self) -> Option<Value> {
        let mut left = self.and()?;
        while self.punct("||") || self.word("or") {
            let right = self.and()?;
            left = Value::Bool(left.truth() || right.truth());
        }
        Some(left)
    }

    fn and(&mut self) -> Option<Value> {
        let mut left = self.cmp()?;
        while self.punct("&&") || self.word("and") {
            let right = self.cmp()?;
            left = Value::Bool(left.truth() && right.truth());
        }
        Some(left)
    }

    fn cmp(&mut self) -> Option<Value> {
        let left = self.add()?;
        for op in [">=", "<=", "==", "!=", ">", "<"] {
            if self.punct(op) {
                let (a, b) = (left.num()?, self.add()?.num()?);
                return Some(Value::Bool(match op {
                    ">=" => a >= b,
                    "<=" => a <= b,
                    "==" => (a - b).abs() < f64::EPSILON,
                    "!=" => (a - b).abs() >= f64::EPSILON,
                    ">" => a > b,
                    _ => a < b,
                }));
            }
        }
        Some(left)
    }

    fn add(&mut self) -> Option<Value> {
        let mut left = self.mul()?;
        loop {
            if self.punct("+") {
                left = Value::Num(left.num()? + self.mul()?.num()?);
            } else if self.punct("-") {
                left = Value::Num(left.num()? - self.mul()?.num()?);
            } else {
                return Some(left);
            }
        }
    }

    fn mul(&mut self) -> Option<Value> {
        let mut left = self.unary()?;
        loop {
            if self.punct("*") {
                left = Value::Num(left.num()? * self.unary()?.num()?);
            } else if self.punct("/") {
                let right = self.unary()?.num()?;
                if right == 0.0 {
                    return None;
                }
                left = Value::Num(left.num()? / right);
            } else {
                return Some(left);
            }
        }
    }

    fn unary(&mut self) -> Option<Value> {
        if self.punct("-") {
            return Some(Value::Num(-self.unary()?.num()?));
        }
        if self.punct("!") || self.word("not") {
            return Some(Value::Bool(!self.unary()?.truth()));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Option<Value> {
        let mut value = self.primary()?;
        while self.punct(".") {
            let Some(Token::Ident(method)) = self.peek().cloned() else {
                return None;
            };
            self.at += 1;
            value = match (method.as_str(), value) {
                ("to_i" | "floor", Value::Num(n)) => Value::Num(n.floor()),
                ("to_f", Value::Num(n)) => Value::Num(n),
                ("round", Value::Num(n)) => Value::Num(n.round()),
                ("ceil", Value::Num(n)) => Value::Num(n.ceil()),
                ("min", Value::List(l)) => Value::Num(l.into_iter().reduce(f64::min)?),
                ("max", Value::List(l)) => Value::Num(l.into_iter().reduce(f64::max)?),
                _ => return None,
            };
        }
        Some(value)
    }

    fn primary(&mut self) -> Option<Value> {
        match self.peek().cloned()? {
            Token::Num(n, _) => {
                self.at += 1;
                Some(Value::Num(n))
            }
            Token::Punct("(") => {
                self.at += 1;
                let value = self.expr()?;
                self.punct(")").then_some(value)
            }
            Token::Punct("[") => {
                self.at += 1;
                let mut items = vec![self.expr()?.num()?];
                while self.punct(",") {
                    items.push(self.expr()?.num()?);
                }
                self.punct("]").then_some(Value::List(items))
            }
            Token::Ident(word) => {
                self.at += 1;
                match word.as_str() {
                    "true" => Some(Value::Bool(true)),
                    "false" | "nil" => Some(Value::Bool(false)),
                    "Spell" => self.spell(),
                    _ => self.path(word),
                }
            }
            Token::Punct(_) => None,
        }
    }

    /// `Spell[N].method`.
    fn spell(&mut self) -> Option<Value> {
        if !self.punct("[") {
            return None;
        }
        let Some(Token::Num(_, text)) = self.peek().cloned() else {
            return None;
        };
        self.at += 1;
        if !(self.punct("]") && self.punct(".")) {
            return None;
        }
        let Some(Token::Ident(method)) = self.peek().cloned() else {
            return None;
        };
        self.at += 1;
        let number: u16 = text.parse().ok()?;
        (self.resolve)(&Name::Spell(number, method))
    }

    /// `Module.method`.
    fn path(&mut self, module: String) -> Option<Value> {
        if !self.punct(".") {
            return None;
        }
        let Some(Token::Ident(method)) = self.peek().cloned() else {
            return None;
        };
        self.at += 1;
        (self.resolve)(&Name::Path(module, method))
    }
}

/// The token for a punctuation string: `PUNCT`'s own static.
fn leak(p: &str) -> &'static str {
    PUNCT.iter().find(|q| **q == p).copied().unwrap_or("")
}
