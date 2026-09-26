//! The group's rules, pure (`plan/39` §6, Stage 2): what a hunting group
//! decides together, as functions over the members' reports.
//!
//! No socket, no clock, no random numbers, no sessions. Time is an
//! [`Instant`](std::time::Instant) the caller passes, and a random choice is
//! a number it rolls, so a test can hold both. Stage 3 fills the
//! [`Report`]s from each member's game state and runs these on the hunt
//! engine; nothing here knows how the reports arrive.
//!
//! | Rule | Module | bigshot, and the answer that changed it |
//! |---|---|---|
//! | the role: lead, follow or solo | [`report`] | `solo?`/`leading?`/`following?` (`bigshot.lic:7199-7212`); read off the game's group (`plan/39` §8, questions 2 and 8) |
//! | who loots | [`looter`](mod@looter) | `ma_looter` (`bigshot.lic:7104-7135`) |
//! | whether to rest, and how to leave | [`rest`] | `should_rest?` (`bigshot.lic:9046-9077`); everyone dropped (question 9) |
//! | whether to hunt again | [`rest`] | `should_hunt?` (`bigshot.lic:8979-9002`) |
//! | who preps first at the rest | [`rest`] | `quiet_followers` (`bigshot.lic:7525-7549`) |
//! | a member apart from the leader, and each wait's deadline | [`muster`](mod@muster) | none in bigshot, which drops a lost follower (`bigshot.lic:954-966`); question 7's table and `lost_wait` (question 5) |
//! | who recovers a dead member | [`muster`](mod@muster) | none in bigshot; question 10 |
//! | who leads when the leader is lost | [`successor`](mod@successor) | none in bigshot; question 4 |
//!
//! # Every wait has a deadline
//!
//! bigshot has twenty waits on its followers and none ends (`plan/39` §0e).
//! Each wait these rules imply is [`Muster`]'s, and it ends at
//! [`Settings::lost_wait`] after it began (`plan/12` §5.5): a member lost to
//! a dropped connection is then handed over, one walking over is fetched,
//! and one that still cannot move sends the group to rest as soon as it can
//! ([`Muster::Overdue`]). The rest merge's hold for a stunned member is the
//! same member's [`Muster::Hold`], so it has one deadline, not two.

pub mod looter;
pub mod muster;
pub mod report;
pub mod rest;
pub mod settings;
pub mod successor;

pub use looter::looter;
pub use muster::{Muster, muster, recoverer};
pub use report::{Hindrance, Leading, Report, Role, role};
pub use rest::{PrepOrder, RestCall, all_dropped, prep_order, should_rest, unready};
pub use settings::{FriedTrigger, LOST_WAIT, Settings, looter_pattern};
pub use successor::successor;
