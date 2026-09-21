//! What the bank says you have.
//!
//! Ports the reading half of `gemstone/bank.rb` (389 lines): the `Pattern`
//! module's account and note rules, plus the two pure helpers that turn them
//! into an answer. The verbs -- `deposit`, `withdraw`, `deposit_f2p` and the
//! Pinefar banker dance -- send commands and wait, so they belong to
//! `cena-behavior` at M6, the same split `resolve.rs` records for `stash.rb`.
//!
//! MEASURED over `bank.rb`'s 18 entry points: **10 send/wait, 8 pure.**
//!
//! # A real balance the port cannot see
//!
//! `ACCOUNT_LINE` (`bank.rb:69`) is
//!
//! ```text
//! /^\s+(?<bank>.+?) Bank: (?<silver>[\d,]+)$/
//! ```
//!
//! which requires the bank's name to **end** in `Bank`. VERIFIED against the
//! author's own `bank account` output (`2026-09-01_15-12-27`):
//!
//! ```text
//!      First Elanith Secured Bank: 586,836,811     matches
//!              Icemule Trace Bank: 3,800,267       matches
//!       Vornavis Bank of Solhaven: 10,627,060      DOES NOT MATCH
//!                 Four Winds Bank: 425,802,149     matches
//!              Kraken's Fall Bank: 26,743,838      matches
//!                          Total: 1,053,810,125
//! ```
//!
//! `Vornavis Bank of Solhaven` has `Bank` in the middle, so **10,627,060
//! silver is invisible** to Lich's `banks` map. The consequence is worse than
//! a missing row: `balance` falls back to
//! `banks[local_bank(...)] || 0` (`bank.rb:125`), so a character standing in
//! Solhaven reads a balance of **0** while holding 10.6 million, and anything
//! trusting that figure behaves as though the account were empty.
//!
//! Here the separator is the **last** `": "` on the line, which is what the
//! wire's fixed-width layout actually guarantees, and the name is whatever
//! precedes it. No bank name has to be any particular shape.
//!
//! # What is ported from the regex rather than from wire
//!
//! The free-to-play lines (`ACCOUNT_BALANCE`, `ACCOUNT_MAX`, and the
//! single-account `You currently have an account` opener) do **not** appear in
//! the corpus -- the author's account is not free-to-play, so nothing here has
//! ever sent them. They are ported as `bank.rb:71-72` writes them and marked
//! UNVERIFIED in the tests, rather than left out: a capped account reading its
//! balance as "unknown" would be worse than reading it from an untested rule.

use crate::state::chunks::{Chunk, ChunkLine};

/// Bank nouns, `bank.rb:81`'s `NOTE_NOUN`.
pub const NOTE_NOUNS: [&str; 3] = ["note", "scrip", "chit"];

/// One town's balance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankBalance {
    /// The bank's name as the listing spells it, e.g.
    /// `"Vornavis Bank of Solhaven"`.
    pub bank: String,
    /// Silver on deposit.
    pub silver: u64,
}

/// What `bank account` reported.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Account {
    /// Every town's balance, in the order the listing gave them.
    balances: Vec<BankBalance>,
    /// The `Total:` line, when the listing carried one.
    ///
    /// **Read, not summed.** Lich falls back to `banks.values.sum`
    /// (`bank.rb:126`), which is how its dropped `Vornavis` row would have
    /// gone unnoticed -- the sum is self-consistent and wrong. Keeping the
    /// game's own figure makes a disagreement visible; see [`Self::agrees`].
    total: Option<u64>,
    /// A single-account balance, from `in the amount of N silvers`.
    single: Option<u64>,
    /// The free-to-play cap, from `a maximum of N silvers`.
    max: Option<u64>,
    /// Whether this bank refused access.
    no_access: bool,
    /// Whether a listing has been read at all.
    known: bool,
}

impl Account {
    /// Read a whole `bank account` response.
    pub fn read_chunk(&mut self, chunk: &Chunk) -> bool {
        let opened = chunk.lines().iter().any(|line| opens_account(&line.text()));
        if !opened {
            return false;
        }
        let mut fresh = Self {
            known: true,
            ..Self::default()
        };
        for line in chunk.lines() {
            fresh.read_line(&line.text());
        }
        let changed = *self != fresh;
        *self = fresh;
        changed
    }

    /// Read one line of the response.
    fn read_line(&mut self, text: &str) {
        let trimmed = text.trim();
        if no_access(trimmed) {
            self.no_access = true;
            return;
        }
        // **The listing's rows AND its total are indented; the prose around
        // them is not.** That is what separates a balance from a sentence
        // quoting a figure, and it applies to `Total:` for the same reason it
        // applies to a town row -- an unindented `Total: 9` is something
        // being said, not the listing speaking.
        //
        // An earlier version guarded only the town rows, and the test meant
        // to pin the guard passed a mutation that removed it. Writing a test
        // that actually reached the guard found this second hole.
        let indented = text.len() != trimmed.len();
        if !indented {
            return;
        }
        if let Some(silver) = trimmed
            .strip_prefix("Total:")
            .and_then(|rest| number(rest.trim()))
        {
            self.total = Some(silver);
            return;
        }
        // `in the amount of N silvers` / `a maximum of N silvers`.
        if let Some(silver) = after(trimmed, "in the amount of") {
            self.single = Some(silver);
        }
        if let Some(silver) = after(trimmed, "a maximum of") {
            self.max = Some(silver);
        }
        // A town row. **Split on the LAST `": "`**, not on a `" Bank: "`
        // literal -- see the module doc for the 10.6 million silver that
        // costs.
        if let Some((bank, amount)) = trimmed.rsplit_once(": ")
            && let Some(silver) = number(amount)
            && !bank.is_empty()
        {
            self.balances.push(BankBalance {
                bank: bank.to_owned(),
                silver,
            });
        }
    }

    /// Every town's balance, as the listing gave them.
    #[must_use]
    pub fn balances(&self) -> &[BankBalance] {
        &self.balances
    }

    /// The balance at a named bank.
    #[must_use]
    pub fn at(&self, bank: &str) -> Option<u64> {
        self.balances
            .iter()
            .find(|b| b.bank == bank)
            .map(|b| b.silver)
    }

    /// The bank serving a town, by the room's location.
    ///
    /// `local_bank` (`bank.rb:139`), whose two-pass match is kept: an exact
    /// prefix first, then a looser containment either way. Lich guards
    /// against `Map#location` being `false` -- `false.to_s` is the non-empty
    /// string `"false"`, which matches no bank but is not empty either -- and
    /// that guard is unnecessary here, because a location that is not a
    /// string cannot be passed.
    #[must_use]
    pub fn local(&self, location: &str) -> Option<&BankBalance> {
        let location = location.trim();
        if location.is_empty() {
            return None;
        }
        // Matched against the bank's name with `Bank` REMOVED, because that
        // word is in the name and never in the location: the row says
        // `Four Winds Bank` and the room says `Four Winds Isle`.
        //
        // Lich gets this for free and by accident -- `ACCOUNT_LINE`'s capture
        // stops before ` Bank`, so its keys are already stripped. Keeping the
        // whole name here is what lets `Vornavis Bank of Solhaven` be read at
        // all (see the module doc), so the stripping moves to the one place
        // that needs it rather than being baked into the stored fact.
        self.balances
            .iter()
            .find(|b| location.starts_with(town_of(&b.bank)))
            .or_else(|| {
                self.balances.iter().find(|b| {
                    let town = town_of(&b.bank);
                    location.contains(town) || town.contains(location)
                })
            })
    }

    /// The total the game stated, or `None` if it stated none.
    ///
    /// **Not a sum of [`Self::balances`].** See the field doc.
    #[must_use]
    pub fn total(&self) -> Option<u64> {
        self.total
    }

    /// The sum of the rows this listing was read from.
    #[must_use]
    pub fn sum_of_balances(&self) -> u64 {
        self.balances.iter().map(|b| b.silver).sum()
    }

    /// Whether the stated total matches the rows read.
    ///
    /// `None` when the game stated no total. **`Some(false)` means a row was
    /// missed**, which is exactly the condition that hid 10,627,060 silver
    /// from the port -- and which a sum-based total cannot express.
    #[must_use]
    pub fn agrees(&self) -> Option<bool> {
        self.total.map(|total| total == self.sum_of_balances())
    }

    /// A single-account balance, where the bank reports one figure.
    #[must_use]
    pub fn single(&self) -> Option<u64> {
        self.single
    }

    /// The free-to-play cap, when the bank stated one.
    #[must_use]
    pub fn max(&self) -> Option<u64> {
        self.max
    }

    /// Whether this bank refused access.
    #[must_use]
    pub fn no_access(&self) -> bool {
        self.no_access
    }

    /// Whether any `bank account` response has been read.
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.known
    }
}

/// A bank note the game has described.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// The note's `exist` id, when the line carried a link.
    pub id: Option<String>,
    /// What it is worth.
    pub silver: u64,
}

/// Read a `read <note>` reply.
///
/// `NOTE_VALUE` (`bank.rb:75`). VERIFIED on real wire -- the corpus carries
/// the form with a link, which Lich's regex ignores and this keeps:
///
/// ```text
/// The Mist Harbor <a exist="46215417" noun="note">promissory note</a>
///   has a value of 42,170 silver and reads, "Hold in right hand to use."
/// ```
#[must_use]
pub fn note_line(line: &ChunkLine) -> Option<Note> {
    let text = line.text();
    let silver = after(&text, "has a value of")?;
    // `and reads` is what separates a note's own value from any other
    // sentence carrying the phrase.
    if !text.contains("and reads") {
        return None;
    }
    let id = line
        .objects()
        .find(|link| {
            matches!(&link.kind, cena_protocol::frame::LinkKind::Exist { noun, .. }
                if NOTE_NOUNS.contains(&noun.as_str()))
        })
        .and_then(|link| match &link.kind {
            cena_protocol::frame::LinkKind::Exist { id, .. } => Some(id.clone()),
            _ => None,
        });
    Some(Note { id, silver })
}

/// A bank's name with the word `Bank` taken out.
///
/// `"Four Winds Bank"` -> `"Four Winds"`, and
/// `"Vornavis Bank of Solhaven"` -> `"Vornavis of Solhaven"`. Used only for
/// matching a room's location, never for the name a caller sees.
fn town_of(bank: &str) -> &str {
    bank.strip_suffix(" Bank").unwrap_or(bank)
}

/// Whether a line opens a `bank account` response.
///
/// `ACCOUNT_START` (`bank.rb:73`).
fn opens_account(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("You currently have the following amounts on deposit")
        || trimmed.starts_with("You currently have an account")
        || no_access(trimmed)
}

/// `NO_ACCESS` (`bank.rb:47`), case-insensitive as Lich writes it.
fn no_access(text: &str) -> bool {
    text.to_lowercase().contains("you don't have access")
}

/// The number following a phrase, e.g. `"a maximum of 50,000 silvers"` -> 50000.
fn after(text: &str, phrase: &str) -> Option<u64> {
    let (_, rest) = text.split_once(phrase)?;
    number(rest.split_whitespace().next()?)
}

/// A comma-grouped silver figure.
fn number(text: &str) -> Option<u64> {
    let digits: String = text
        .trim()
        .trim_end_matches(['.', ','])
        .chars()
        .filter(|c| *c != ',')
        .collect();
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}
