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
//! # The gate, measured
//!
//! `plan/06` §1.9: *"Never port a performance conclusion; port the
//! discipline of measuring, then measure here."* Lich gates each table with
//! a union of literal fragments (`defs/pattern_gate.rb`). Here each family
//! carries a `RegexSet` ([`defs`]), and the crit index one over its
//! unanchored residual (`crit/match_index.rs`). `crit/match_index.rs` records
//! `RegexSet` as the slowest option at 2,394 patterns; at 9 to 362 per
//! family, with a raised DFA budget, it is the fastest thing measured.
//!
//! MEASURED, release, warm caches, the 85 replay blobs:
//!
//! ```text
//!                                  linear      gated
//! every family once per line      105 us      3.6 us    (tests/combat_gate.rs prints its own)
//! CritTables::parse, per call    17-20 us     1.6 us
//! the state machine, per line     142 us       10 us
//! parser + chunk + FSM            ~145 us      13 us    (tests/combat_replay_events.rs)
//! ```
//!
//! Lich's own figure is 35us/line with its gate. An earlier note here said
//! 252us/line for the last row: that was ONE COLD PASS, and ~100ms of it was
//! compiling the tables. A `regex` also builds its DFA lazily, so every
//! timing here runs an untimed pass first.
//!
//! **Classify-once-per-line was considered and not done.** The machine asks
//! `DamageLine` up to four times a line and `FlareLine` twice; with the gate
//! each ask is ~0.2-0.4us, so the whole redundancy is under 3us of the 10.
//! Threading a cache through eight branch modules to win that is not KISS.
//!
//! `tests/combat_gate.rs` proves the gate agrees with a linear scan on every
//! fixture line in every family and role, and that no family fell back.
//!
//! # NOT ported, and why
//!
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
