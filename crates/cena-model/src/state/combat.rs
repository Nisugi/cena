//! Combat: the classifiers over one chunk line.
//!
//! Ports the **definition grammar** of Lich's combat module --
//! `lib/gemstone/combat/defs/` and the per-line half of `parser.rb` -- as
//! stateless classifiers, the shape `plan/12` §3a asks for and the one every
//! other classifier in this crate takes. `inventory/11` is the measured
//! account of what is being ported; this module doc records what changed in
//! the crossing.
//!
//! # The patterns are data, cut by a committed tool
//!
//! `tools/extract_combat_defs.rb` reads the def modules with Ruby -- the only
//! thing that can read a Ruby regex literal exactly -- and writes three TSVs
//! by the part of an exchange they describe:
//!
//! | File | Families | Rows |
//! |---|---|---|
//! | `combat_attacks.tsv` | initiations, their prefixes, the coup kill line | 376 |
//! | `combat_results.tsv` | damage, the eight roll grammars, outcomes | 222 |
//! | `combat_effects.tsv` | flares, statuses, spell losses, brackets, UCS | 356 |
//!
//! **954 rows**, first-match-wins within each family in the order Lich
//! assembles them. The `order` column is that position and the loader keeps
//! it; `attacks.rb:762-777` explains why it is load-bearing (priority defs
//! before generic swings, second person before third).
//!
//! # The one transformation: patterns run against PLAIN text
//!
//! Lich's patterns run against the raw XML feed, so 178 of them carry
//! `MK_PRE` / `MK_POST` -- optional groups that tolerate a `<pushBold/>` or an
//! `<a ...>` falling inside the text (`pattern_gate.rb:25-32`). Here a
//! [`ChunkLine`](crate::state::chunks::ChunkLine) is plain text with every link kept beside it as a typed
//! [`Run`](cena_protocol::runs::Run), so those groups can never match and the
//! extractor deletes them. That is the whole edit: a pure deletion of groups
//! that were optional to begin with. VERIFIED at extraction: zero residual
//! tags in any non-flagged row.
//!
//! **Eight patterns read the `exist` id out of the tag itself** and cannot be
//! made plain mechanically -- `<a exist="(?<id>\d+)"`. They are flagged
//! `markup=1` in the data and hand-ported in [`defs::HAND_PORTED`], each as a
//! text capture plus a link-position lookup. The loader substitutes them by
//! `(family, order)`, so their position in the first-match order is exactly
//! Lich's, and a test asserts every flagged row has a port.
//!
//! # What `plan/12` §3a's bargain buys here
//!
//! Lich's `parser.rb` recovers `exist`, `noun` and bold by re-scanning the
//! match text with `TARGET_LINK_PATTERN`, `OPEN_LINK_TAIL_PATTERN` and
//! `BOLD_WRAPPER_PATTERN`, and four of its recorded bugs are re-scanning bugs
//! (`state/chunks.rs` lists them). [`target`] answers the same questions from
//! run positions: a capture's byte span overlaps some runs, and the runs
//! carry the links. The "possessive inside the link" case that needed
//! `OPEN_LINK_TAIL_PATTERN` -- a capture ending at the apostrophe of
//! `<a>skald's</a>` -- is not a case at all, because a span that ends inside a
//! run still overlaps it.
//!
//! # NOT ported, and why
//!
//! - **`PatternGate`**, Lich's literal-substring prefilter -- *not yet*.
//!   `plan/06` §1.9: *"Never port a performance conclusion; port the
//!   discipline of measuring, then measure here."* Each family is a
//!   `Vec<Regex>` tried in order, which is what Lich does after its gate, and
//!   the cost of having no gate was MEASURED on the 71 fixtures' real feed
//!   lines (`tests/combat_bench.rs`, `--nocapture`):
//!
//!   ```text
//!   954 patterns compiled in 664ms
//!   728 lines x 20 rounds: 739µs/line, every family on every line
//!   ```
//!
//!   That is ~0.76µs per regex attempt, times ~950 attempts -- the shape of
//!   the number says everything: the patterns are cheap and there are a lot
//!   of them. Lich's own figure is 35µs/line *with* its gate, so a gate is
//!   worth roughly 20x here, and which gate is the next measurement:
//!   `crit/match_index.rs` records `RegexSet` as the slowest option at 2,394
//!   patterns, but these families are 9 to 362 each, which is a different
//!   regime. The correctness work does not wait on it.
//!
//!   MEASURED again with the state machine on top, on the same fixtures
//!   (`tests/combat_replay_events.rs`, `--release -- --nocapture`):
//!
//!   ```text
//!   85 blobs, 733 lines through parser + FSM in 185ms: 252µs/line
//!   ```
//!
//!   That is the parser, the chunk, every classifier the machine consults
//!   per line, and the crit lookahead -- ~7x Lich's gated 35µs, with a
//!   fresh parser and `GameState` per blob inside the number.
//! - **`messages.rb`**: a separate hook the FSM never reads, and each def
//!   carries a Ruby lambda for its payload. A later pass if a consumer wants it.
//! - **`supplements.rb`**: player-supplied YAML patterns. Whether pattern
//!   supplements fall under `plan/12`'s no-user-scripting decision is an
//!   author question, not one to answer by porting them.
//!
//! # The consumer above the classifiers
//!
//! [`parse`] is `processor.rb`'s `parse_events`, the 1,470-line state machine
//! these classifiers feed, and [`tracker`] is what it keeps between chunks.
//! `GameState::close_chunk` hands every prompt-bounded chunk to
//! [`CombatTracker::consume_chunk`]; the [`event`] types are what comes out.
//! Its module doc records what was and was not carried across.

pub mod attack;
pub mod bracket;
pub mod damage;
pub mod defs;
pub mod event;
pub mod flare;
pub mod outcome;
pub mod parse;
pub mod resolution;
pub mod spell_loss;
pub mod status;
pub mod target;
pub mod tracker;
pub mod ucs;

pub use event::{AttackEvent, ChunkFacts, Fact};
pub use target::Actor;
pub use tracker::CombatTracker;
