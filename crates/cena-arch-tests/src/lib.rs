//! Architecture tests — the rules the compiler cannot express.
//!
//! Cena is a workspace of crates, so Rule 1.1's *cycle* half is enforced by
//! `cargo`: an upward edge that closes a loop gives `error: cyclic package
//! dependency` (`plan/05:252`). But a forbidden edge that happens to be
//! acyclic — `cena-ui -> cena-session`, which `plan/12:81` and `plan/05:232`
//! both forbid — compiles clean. `crate_dependency_edges_match_the_plan`
//! covers that residue, which is exactly what `plan/06:133-135` says the
//! architecture test is for.
//!
//! Deliberately NOT ported from `reference/VellumFE/tests/architecture.rs`:
//! its *source-scanning* layering tests (`:28`, `:48`, `:70`, `:97`, `:122`,
//! `:169`, `:288`). Those exist only because Vellum is one crate
//! (`Cargo.toml:4-5`); asking Cargo for the resolved edges states the same
//! rule without paying in string scans.
//!
//! A note on the harness: nothing here trusts the shape of the input.
//! Dependencies come from `cargo tree --target all` rather than a manifest
//! parser, source files come from walking the whole crate directory rather
//! than an opt-in list, needles match whitespace-collapsed spans rather than
//! raw lines, and Rule 5.2 is an allowlist rather than a needle. Each of those
//! replaced something that was VERIFIED green on a real violation.

pub mod caps;
pub mod harness;
pub mod lexical;
pub mod plan_rules;
pub mod structure;
