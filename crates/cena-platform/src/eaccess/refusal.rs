//! What an `L\tPROBLEM\t<n>` refusal means, and whether to retry it.
//!
//! Split from [`super::wire`] under `plan/05` Rule 4.1 when that file passed
//! the 400-line cap. The seam is clean: this is a lookup table over one wire
//! value, with no dependency on the rest of the vocabulary.
//!
//! # All four codes are VERIFIED, and three of them must not be retried
//!
//! `plan/10` §4.7 item 2a reads them from **Saga 0.9.9's own user-facing
//! English strings** -- Simutronics' client, read directly, not inferred from
//! control flow. The retry verdict is the operational half: §9.1's blanket
//! 3-retry is wrong for **2** and **3**, which are server-side configuration
//! facts that will not change between attempts, so retrying burns three logins
//! to reach the same refusal and delays the real message to the user.

/// Explain an `L` refusal, including the code Lich does not document.
///
/// Split out so the PROBLEM 3 finding is testable without a live login --
/// it is INFERRED from a single observation, and an inference that cannot be
/// re-read is one that quietly becomes folklore.
#[must_use]
pub fn describe_launch_refusal(l: &str) -> String {
    let Some(rest) = l.trim().strip_prefix("L\tPROBLEM") else {
        return format!("launch refused: {}", l.trim());
    };
    // All four codes are VERIFIED (`plan/10` §4.7 item 2a) from Saga 0.9.9's
    // own user-facing English strings -- Simutronics' client, read directly.
    //
    // This replaced a message that documented only PROBLEM 1 and offered an
    // INFERRED reading of 3 ("the character code is not valid on the selected
    // instance"), plus a slot-count heuristic. `plan/10` had already superseded
    // both: 3 is server-side, and the slot count reflects the account's
    // ENTITLEMENT rather than the selected instance -- which the author's
    // multi-entitlement account (Shattered + Premium) makes plain.
    //
    // The retry column is the operational point: §9.1's blanket 3-retry is
    // wrong for 2 and 3, which are server-side facts that will not change
    // between attempts.
    let detail = match rest.trim_start().split('\t').next().map(str::trim) {
        Some("1") => {
            "the account's access level does not permit playing this instance \
             -- a lapsed or missing subscription -- or the account service \
             timed out. DO NOT RETRY: this is account state, and the fix is a \
             subscription, not another login."
        }
        Some("2") => {
            "the server has no STORM launch entry for this game's \
             configuration. SERVER-SIDE, not an account problem. DO NOT RETRY: \
             it will not change between attempts."
        }
        Some("3") => {
            "the server has no configuration for the selected game. \
             SERVER-SIDE. DO NOT RETRY. Check first that the game code was \
             listed by M and that F, G and P each echoed the code that was \
             sent -- a rejected G leaves the session pointed at an instance \
             the server cannot launch, which produces exactly this. \
             Byte-diffing the L request is a dead end: its bytes are correct \
             in this failure."
        }
        Some("4") => "the account service failed while assigning the character. RETRY: transient.",
        _ => {
            "unrecognised PROBLEM sub-code. plan/10 §4.7 documents 1-4, all \
             VERIFIED from Saga 0.9.9's English strings; a fifth means the \
             server grew one."
        }
    };
    format!("launch refused ({}): {detail}", l.trim())
}
