//! Game-specific data, under the namespace `plan/05` Rule 3.4 prescribes.
//!
//! # Why this directory exists for one table
//!
//! Rule 3.4 says *"Game-specific code lives under a game namespace"*
//! (`plan/05:332-336`), enforced by
//! `cena-arch-tests/tests/file_rules.rs::game_names_outside_game_modules_are_flagged`,
//! which flags game-name literals in code outside `/src/gemstone/` and
//! `/src/dragonrealms/`.
//!
//! [`endpoint`]'s host table contains `gs4.simutronics.net` and
//! `storm.gs4.game.play.net`, so it trips that test. Three ways out were
//! weighed (author's call, 2026-09-19):
//!
//! | | |
//! |---|---|
//! | **this** -- move it under the namespace | the rule's own remedy |
//! | exempt the path in the test | widens the ratchet by one hole |
//! | assemble the literals from parts | the test's comment names this as what **defeats** it |
//!
//! The third was declined explicitly. A scan that is routinely worked around
//! stops being a ratchet, and a hostname split across `concat!` is data hidden
//! from the next reader for no gain.
//!
//! # Why the crate ROOT, and not `eaccess/gemstone/`
//!
//! It was first placed beside its only caller, at `src/eaccess/gemstone/`. The
//! arch test still flagged it: its exclusion is the literal `/src/gemstone/`,
//! which a nested path does not contain. That matches Rule 3.4's own wording --
//! `cena-model/src/gemstone/`, directly under `src` -- so the placement, not
//! the test, was wrong.
//!
//! # What the rule is actually aimed at, and why this still belongs here
//!
//! Rule 3.4's target is *"an `if game == ...` branch"* in a shared module --
//! the DragonRealms-deferral guard (`plan/12` §9d, `CLAUDE.md`). [`endpoint`]
//! has no such branch: it is a lookup table of opaque endpoint strings, and
//! nothing in it asks which game is being played.
//!
//! It is still game-specific **data**, which is what the namespace is for, so
//! this is the rule applied rather than merely satisfied.
//!
//! # The DragonRealms row, under a `gemstone/` path
//!
//! [`endpoint`]'s fourth row maps `prime.dr.game.play.net:4901` to
//! `dr.simutronics.net:11024`. That is a DragonRealms endpoint sitting under
//! `gemstone/`, which is **imprecise and deliberate**:
//!
//! - It is one row of a four-row table ported whole from Lich
//!   (`lib/lich.rb:737-768`). Splitting the table to place one row correctly
//!   would mean two tables and two lookups for no behavioural difference.
//! - `CLAUDE.md` defers DragonRealms **all-or-nothing** and forbids a
//!   `GameAdapter` abstraction for it. A `dragonrealms/` directory created now
//!   to hold a single unreachable row would be the first piece of exactly the
//!   structure that deferral exists to avoid.
//!
//! When DragonRealms is genuinely taken on, the row moves with the rest of the
//! DR work. Until then it is inert -- Cena never launches a DR game code -- and
//! recorded here so its placement reads as a decision, not an oversight.

pub mod endpoint;
pub mod weblogin;
