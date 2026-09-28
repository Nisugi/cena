//! What the mind bar says of advancement, beyond the mind itself.
//!
//! The game puts experience on `<progressBar id='mindState'>` as attributes,
//! where the bar's own `value=` and `text=` are the mind's state. The frame
//! kept them (`ProgressBar::attrs`) and the model read none (Rule 2.2a).
//! MEASURED in `crates/cena-behavior/tests/fixtures/arch_kill.xml`:
//!
//! ```text
//! <progressBar id='mindState' value='25' text='fresh and clear' top='45' left='3'
//!   field_exp='270' max_field_exp='1403' ascension_exp='24899176' fashlonae='1'
//!   lumnis='4' exp='43904921' until_next='79' .../>
//! ```
//!
//! Lich reads each into `XMLData` (`lib/common/xmlparser.rb:728-740`), which
//! scripts read (`Experience`, `plan/46` §11 step 7). Four of them are what
//! the `experience` command also says, and fill the same fields; the rest are
//! kept here.
//!
//! **Lumnis, `rpa` and `fashlonae` are sent only while they apply**, so a bar
//! without one says it is gone: each is cleared by a bar that omits it, as
//! Lich clears it. The four numbers are only ever set, never cleared: a bar
//! without them says nothing of them.

use super::Experience;
use crate::state::numbers;

/// The mind bar's advancement attributes that nothing else states.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MindBar {
    /// `until_next=`: experience to the next level, or to the next training
    /// point past the cap.
    pub until_next: Option<u32>,
    /// `lumnis=`, while the gift of Lumnis is active; verbatim, as a number.
    pub lumnis: Option<u32>,
    /// `rpa=`, while active; **verbatim**, since the game sends fractions
    /// (`1.5`, per Lich's reading of it as a float).
    pub rpa: Option<String>,
    /// `fashlonae=`, once an orb is redeemed: 1 redeemed, 2 active (Lich's
    /// note, `xmlparser.rb:734`).
    pub fashlonae: Option<u8>,
}

/// Read the mind bar's `attrs` into `exp`.
pub(super) fn read(exp: &mut Experience, attrs: &[(String, String)]) {
    let attr = |name: &str| {
        attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let number = |name: &str| attr(name).and_then(numbers::grouped);
    if let Some(value) = number("field_exp") {
        exp.field_experience = Some(value);
    }
    if let Some(value) = number("max_field_exp") {
        exp.field_experience_max = Some(value);
    }
    if let Some(value) = attr("ascension_exp").and_then(numbers::grouped) {
        exp.ascension_experience = Some(value);
    }
    if let Some(value) = attr("exp").and_then(numbers::grouped) {
        exp.experience = Some(value);
    }
    if let Some(value) = number("until_next") {
        exp.mind_bar.until_next = Some(value);
    }
    exp.mind_bar.lumnis = number("lumnis");
    exp.mind_bar.rpa = attr("rpa").map(str::to_owned);
    exp.mind_bar.fashlonae = attr("fashlonae").and_then(numbers::grouped);
}
