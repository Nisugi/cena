//! The six combat stances, and the percent bands they occupy.
//!
//! Ports `NAMES`, `BANDS` and `normalize` from `gemstone/stance.rb:26-115`
//! (172 lines). **The sending half is not here** — `change`, `safest`,
//! `ACCEPTED`/`DECLINED`/`CONFIRM` and the `dothistimeout` wait all need the
//! authority token and roundtime, so they are M6, the same split `stash.rb`,
//! `bank.rb`, `fog.rb` and `move.rb` took (`plan/20` §0b).
//!
//! `plan/20` §0c already recorded this gap: *"a verbatim string today…
//! nothing needs it until a behavior SETS stance, which is M6"*. That was
//! right about the sender and wrong about the reader — the band table is
//! needed to answer "am I in the stance I asked for?", which a behavior must
//! do **before** it sends anything.
//!
//! # What the wire actually sends
//!
//! ```text
//! <progressBar id='pbarStance' value='100' text='defensive (100%)'/>
//! ```
//!
//! MEASURED over the 208 live Lich XML logs — **four distinct `(value, text)`
//! pairs in 19,526 readings**:
//!
//! | value | text | count |
//! |---:|---|---:|
//! | 0 | `offensive (0%)` | 8,466 |
//! | 80 | `guarded (80%)` | 1,656 |
//! | 99 | `defensive (99%)` | **2** |
//! | 100 | `defensive (100%)` | 9,402 |
//!
//! `value=` and the percent inside `text=` agreed in **all 19,526**, so
//! [`Stance::from_percent`] over either gives the same answer. Only three of
//! the six names appear; the other three are Lich's, and a character who never
//! stood in them cannot evidence them.
//!
//! # READING ACCEPTS ANY PERCENT; SETTING IS RESTRICTED TO TENS
//!
//! Lich's `normalize_percent` (`stance.rb:163-170`) **raises** unless the
//! percent is a multiple of ten. That is a rule about what you may ASK FOR,
//! and porting it into the reader would reject a real game state.
//!
//! The corpus proves it: `defensive (99%)` occurs, and both readings come from
//! the same moment — a `gold-bristled hinterboar feints to the left. You spot
//! the ruse too late as you move to block` — so **a game event can knock the
//! stance off a round number**. A reader that accepted only multiples of ten
//! would have called that unknown, or worse, snapped it to 100.
//!
//! So [`Stance::from_percent`] takes any `0..=100`, and the multiple-of-ten
//! rule belongs with `change` in M6 where it is a caller-argument check.

use std::fmt;

/// A combat stance.
///
/// The six of `NAMES` (`stance.rb:26`), ordered **most offensive first**,
/// which is the order Lich lists them and the order the bands ascend in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stance {
    /// 0% to defense.
    Offensive,
    /// 1–20%.
    Advance,
    /// 21–40%.
    Forward,
    /// 41–60%.
    Neutral,
    /// 61–80%.
    Guarded,
    /// 81–100%.
    Defensive,
}

impl Stance {
    /// Every stance, most offensive first.
    pub const ALL: [Self; 6] = [
        Self::Offensive,
        Self::Advance,
        Self::Forward,
        Self::Neutral,
        Self::Guarded,
        Self::Defensive,
    ];

    /// The name the game uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offensive => "offensive",
            Self::Advance => "advance",
            Self::Forward => "forward",
            Self::Neutral => "neutral",
            Self::Guarded => "guarded",
            Self::Defensive => "defensive",
        }
    }

    /// The percent-of-defense band this stance occupies.
    ///
    /// `BANDS` (`stance.rb:31-38`). Contiguous and total over `0..=100`,
    /// which a test asserts rather than trusting — a gap would make some real
    /// game state nameless, and an overlap would make [`Self::from_percent`]
    /// depend on declaration order.
    #[must_use]
    pub fn band(self) -> (u32, u32) {
        match self {
            Self::Offensive => (0, 0),
            Self::Advance => (1, 20),
            Self::Forward => (21, 40),
            Self::Neutral => (41, 60),
            Self::Guarded => (61, 80),
            Self::Defensive => (81, 100),
        }
    }

    /// The stance a percent falls in.
    ///
    /// **Accepts any `0..=100`**, not only multiples of ten — see the module
    /// doc on `defensive (99%)`. `None` above 100, which the game has never
    /// sent and which would be a protocol change rather than a stance.
    #[must_use]
    pub fn from_percent(percent: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|s| {
            let (low, high) = s.band();
            (low..=high).contains(&percent)
        })
    }

    /// Read a stance name, or a prefix of one.
    ///
    /// `normalize`'s string arm (`stance.rb:100-106`): **at least three
    /// characters**, matched as a prefix, case-insensitively. `off`, `adv`,
    /// `for`, `neu`, `gua`, `def` are what scripts actually type.
    ///
    /// The three-character floor is Lich's and it is load-bearing: `"o"` would
    /// match `offensive` alone today, but `"f"` already matches `forward` only
    /// by accident of the list, and a future stance would silently change what
    /// a one-letter abbreviation means.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let cleaned = text.trim().to_lowercase();
        if cleaned.len() < 3 {
            return None;
        }
        Self::ALL
            .into_iter()
            .find(|s| s.as_str().starts_with(&cleaned))
    }

    /// Read what `pbarStance`'s `text=` says: `"defensive (100%)"`.
    ///
    /// Returns the stance **and** the percent, because the two are separately
    /// useful and the wire sends both in one string. MEASURED: the percent
    /// here agreed with the bar's own `value=` in all 19,526 readings, so a
    /// caller may use either — but this parses the text rather than assuming
    /// that, since an assumption that holds today is not a rule.
    #[must_use]
    pub fn parse_bar_text(text: &str) -> Option<(Self, u32)> {
        let (name, rest) = text.trim().split_once(" (")?;
        let percent = rest.strip_suffix("%)")?.parse().ok()?;
        Some((Self::parse(name)?, percent))
    }
}

impl fmt::Display for Stance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
