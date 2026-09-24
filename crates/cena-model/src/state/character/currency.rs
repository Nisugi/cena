//! Silver, notes, and the eleven event currencies.
//!
//! # All single lines, all from ordinary play
//!
//! Like `standing.rs` and for the same reason: each line stands alone and most
//! arrive without a command. `wealth` states your silver; a festival merchant
//! tells you your bloodscrip while you shop.
//!
//! # Why so many currencies
//!
//! The game's paid events each mint their own scrip, and they accumulate: a
//! character who has attended Duskruin, Reim, Ebon Gate and Rumor Woods carries
//! four balances that do not convert. MEASURED against the author's store,
//! which holds five at once:
//!
//! ```text
//! currency.silver_total : 81
//! currency.tickets : 17
//! currency.bloodscrip : 1024
//! currency.ethereal_scrip : 7117
//! currency.soul_shards : 9963
//! currency.gemstone_dust : 7
//! ```
//!
//! So this is a named field per currency rather than a map. C21's rule: a
//! closed vocabulary gets typed fields, and this vocabulary is closed by what
//! Simutronics has run. A new event adds a field, which is a visible change.

use crate::state::chunks::Chunk;

/// Every balance a character carries.
///
/// All `Option`: a character who has never been to Duskruin has **unknown**
/// bloodscrip, not zero (§5.2). That distinction is why a consumer can tell
/// "you have none" from "nobody has said", and why a display can omit a
/// currency the player has never earned rather than showing a row of zeroes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Currency {
    /// Silver on your person, from `wealth`.
    pub silver: Option<u64>,
    /// Silver in a container you are carrying.
    pub silver_container: Option<u64>,
    /// `You are carrying a total of N silver.`
    pub silver_total: Option<u64>,
    /// `Total note value: N` -- promissory notes, not coins.
    pub notes: Option<u64>,
    /// Voln favor. Signed: Lich captures `[-\d,]+` (`parser.rb:53`).
    pub voln_favor: Option<i64>,
    /// Simucoin general tickets.
    pub tickets: Option<u64>,
    /// Gold ("Gold - N gold.").
    pub gold: Option<u64>,
    /// Troubled Waters.
    pub blackscrip: Option<u64>,
    /// Duskruin Arena.
    pub bloodscrip: Option<u64>,
    /// Reim.
    pub ethereal_scrip: Option<u64>,
    /// Ebon Gate.
    pub soul_shards: Option<u64>,
    /// Rumor Woods.
    pub raikhen: Option<u64>,
    /// Inquisitor.
    pub aevit: Option<u64>,
    /// Gigas artifact fragments.
    pub gigas_artifact_fragments: Option<u64>,
    /// Redsteel marks.
    pub redsteel_marks: Option<u64>,
    /// `Dust`, held "in your reserves".
    ///
    /// **Named for the item, not the game.** Lich's key is `gemstone_dust`
    /// (`parser.rb:65`), which names the GAME rather than what you are
    /// carrying -- and Rule 3.4's scanner flags that spelling in a shared
    /// module, correctly: the wire says only `N Dust in your reserves`, so
    /// `dust` is both the accurate name and the one that passes.
    pub dust: Option<u64>,
}

impl Currency {
    /// Take whatever balances a chunk's lines state.
    ///
    /// Returns whether anything moved, so a caller marks a group dirty only
    /// when there is something to write. Silver changes on every purchase, so
    /// a `wealth` that restates it must not schedule a save.
    pub fn absorb_chunk(&mut self, chunk: &Chunk) -> bool {
        let mut changed = false;
        for line in chunk.lines() {
            changed |= self.absorb(&line.text());
        }
        changed
    }

    /// One line.
    fn absorb(&mut self, line: &str) -> bool {
        let before = *self;

        // `You have no silver with you.` / `but one` / `N`.
        //
        // The word forms are the game's, and Lich maps them to 0 and 1
        // (`parser.rb:546-555`). Reading them as numbers would silently store
        // nothing for a character who is broke, which reads the same as never
        // having asked.
        if let Some(rest) = line.strip_prefix("You have ")
            && let Some(amount) = rest.strip_suffix(" silver with you.")
        {
            self.silver = match amount {
                "no" => Some(0),
                "but one" => Some(1),
                other => number(other),
            };
        }

        if let Some(rest) = line.strip_prefix("You are carrying ") {
            if let Some((amount, _)) = rest.split_once(" silver stored within your ") {
                self.silver_container = number(amount);
            } else if let Some(amount) = rest.strip_suffix(" gigas artifact fragments.") {
                self.gigas_artifact_fragments = number(amount);
            } else if let Some(amount) = rest.strip_suffix(" gigas artifact fragment.") {
                self.gigas_artifact_fragments = number(amount);
            } else if let Some(amount) = rest.strip_suffix(" Dust in your reserves.") {
                self.dust = number(amount);
            } else if let Some(amount) = rest.strip_suffix(" Dust in your reserve.") {
                self.dust = number(amount);
            } else if let Some(amount) = rest
                .strip_suffix(" redsteel marks.")
                .or_else(|| rest.strip_suffix(" redsteel mark."))
            {
                self.redsteel_marks = number(amount);
            } else if let Some(amount) = rest
                .strip_prefix("a total of ")
                .and_then(|r| r.strip_suffix(" silver."))
            {
                self.silver_total = number(amount);
            }
        }

        if let Some(amount) = line.strip_prefix("Total note value: ") {
            self.notes = number(amount);
        }
        if let Some(amount) = line.strip_prefix("Voln Favor: ") {
            self.voln_favor = signed(amount);
        }
        // The indented `Redsteel Marks:` form, which the same Lich pattern
        // accepts as an alternation (`parser.rb:64`).
        if let Some(amount) = line.trim_start().strip_prefix("Redsteel Marks:") {
            self.redsteel_marks = number(amount);
        }

        self.absorb_ticket(line.trim_start());
        before != *self
    }

    /// The `<Event> - N <currency>.` lines a ticket balance arrives on.
    fn absorb_ticket(&mut self, line: &str) {
        const TICKETS: [(&str, &str); 8] = [
            ("General - ", " tickets."),
            ("Gold - ", " gold."),
            ("Troubled Waters - ", " blackscrip."),
            ("Duskruin Arena - ", " bloodscrip."),
            ("Reim - ", " ethereal scrip."),
            ("Ebon Gate - ", " soul shards."),
            ("Rumor Woods - ", " raikhen."),
            ("Inquisitor - ", " aevit."),
        ];
        for (index, (prefix, suffix)) in TICKETS.into_iter().enumerate() {
            let Some(rest) = line.strip_prefix(prefix) else {
                continue;
            };
            // The singular, because the game says "1 ticket." not "1 tickets."
            let singular = suffix.trim_end_matches('.').trim_end_matches('s');
            let Some(amount) = rest
                .strip_suffix(suffix)
                .or_else(|| rest.strip_suffix(&format!("{singular}.")))
            else {
                continue;
            };
            let value = number(amount);
            match index {
                0 => self.tickets = value,
                1 => self.gold = value,
                2 => self.blackscrip = value,
                3 => self.bloodscrip = value,
                4 => self.ethereal_scrip = value,
                5 => self.soul_shards = value,
                6 => self.raikhen = value,
                _ => self.aevit = value,
            }
            return;
        }
    }
}

/// `"1,024"` -> `1024`. The shared, strict reader: see `state/numbers.rs` for
/// why `"12a3"` is `None` rather than `123`.
fn number(text: &str) -> Option<u64> {
    crate::state::numbers::grouped(text)
}

/// Same, keeping a leading `-`.
fn signed(text: &str) -> Option<i64> {
    crate::state::numbers::grouped(text)
}
