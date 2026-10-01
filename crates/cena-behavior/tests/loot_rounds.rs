//! The selling round's errands over a scripted game (`plan/61` step 5),
//! each from a room of a small town: `loot pool` here, `loot sell` and
//! `loot box` with them. The planner's own tests (`town_pool.rs`,
//! `town_plan.rs`) say what each step should be; these say the driver sends
//! it, reads what comes back, and walks where it must.

mod drive_support;
mod errand_support;
mod ready;

use std::sync::Arc;

use cena_behavior::hunt::{Ending, FullBags, HuntEnd};
use cena_behavior::loot::Errand;
use cena_session::GameState;
use drive_support::arrival;
use errand_support::{at, ended, in_order, inside, reply, set_out_knowing};

/// Home (1), the pool east of it (2), whose worker the map names, and the
/// bank west of it (3).
const TOWN: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
    {"to":2,"kind":"cardinal","cmd":"east","cost":1},
    {"to":3,"kind":"cardinal","cmd":"west","cost":1}]},
  {"id":2,"uid":[1002],"tags":["locksmith pool"],
   "meta":["boxpool:npc:grimy halfling scoundrel"],"exits":[
    {"to":1,"kind":"cardinal","cmd":"west","cost":1}]},
  {"id":3,"uid":[1003],"tags":["bank"],"exits":[
    {"to":1,"kind":"cardinal","cmd":"east","cost":1}]}
]"#;

/// A profile that tips the pool 300 a box.
const POOLS: &str =
    "take = [\"gem\"]\n\n[town]\nsell_locksmith_pool = true\nsell_locksmith_pool_tip = 300\n";

/// The pool's room as the game states it on arrival: a woman, whom a word
/// of eloot's would take for the worker, and the worker the map names.
const AT_THE_POOL: &[u8] = b"<nav rm='1002'/>\n<component id='room objs'>You also see <pushBold/>a <a exist=\"70\" noun=\"woman\">slender aelotoi woman</a><popBold/> and <pushBold/>a <a exist=\"71\" noun=\"scoundrel\">grimy halfling scoundrel</a><popBold/>.</component>\n<prompt time=\"2\">&gt;</prompt>\n";

/// An iron coffer in the backpack.
fn a_coffer(state: &mut GameState) {
    inside(state, "902", &[("5", "coffer", "iron coffer")]);
}

/// `loot pool` (`;eloot pool`, `pool`, `eloot.lic:7612-7652`): what is
/// carried is asked first, the coffer given to the worker the map names and
/// not to the woman, the returns asked for, and then the bank, where what was
/// carried is kept and the rest, the tip's difference, evened out.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn loot_pool_gives_to_the_maps_worker_and_banks_after() {
    let errand = Errand::Pool {
        drop: true,
        collect: true,
    };
    let (transcript, task, _) = set_out_knowing(
        errand,
        Vec::new(),
        (TOWN, POOLS),
        &Arc::default(),
        &FullBags::default(),
        a_coffer,
    );
    transcript.answer("wealth quiet", &reply("You have 1,234 silver with you."));
    transcript.answer("east", AT_THE_POOL);
    transcript.answer(
        "get #5",
        &reply("You remove an iron coffer from in your backpack.\n<right exist=\"5\" noun=\"coffer\">iron coffer</right>"),
    );
    transcript.answer(
        "give #71 300",
        &reply("You want a locksmith to open an <a exist=\"5\" noun=\"coffer\">iron coffer</a> for a tip of 300 silvers, or 3 percent of the box value.  There is also a fee of 50 silvers due up front."),
    );
    transcript.answer(
        "give #71 300 confirm",
        &reply("<pushBold/>The <a exist=\"71\" noun=\"scoundrel\">grimy halfling scoundrel</a><popBold/> takes your coffer and says, \"Your tip of 300 silvers has been recorded, and the 50 silver fee has been collected.  We'll get someone on that right away.\"\n<right>Empty</right>"),
    );
    transcript.answer(
        "ask #71 for return",
        &reply("<pushBold/>The <a exist=\"71\" noun=\"scoundrel\">grimy halfling scoundrel</a><popBold/> says, \"We don't have any boxes ready for you.\""),
    );
    transcript.answer("west", &arrival(1001));
    transcript.answer("west", &arrival(1003));
    transcript.answer(
        "deposit all",
        &reply("You deposit 884 silvers into your account."),
    );
    transcript.answer("east", &arrival(1001));
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(errand))),
        "{end:?} {lines:?}"
    );
    assert!(
        in_order(
            &lines,
            &[
                "wealth quiet",
                "east",
                "get #5",
                "give #71 300",
                "give #71 300 confirm",
                "ask #71 for return",
                "west",
                "west",
                "deposit all",
                "withdraw 1234 silver",
                "east",
            ]
        ),
        "{lines:?}"
    );
    assert_eq!(at(&lines, "give #70 300"), None, "not the woman: {lines:?}");
}

/// A profile that takes gems from its boxes and pools them.
const RETURNS: &str = "take = [\"gem\"]\n\n[town]\nsell_locksmith_pool = true\n";

/// The worker hands back the iron chest `6`.
const CHEST_BACK: &[u8] = b"<pushBold/>The <a exist=\"71\" noun=\"scoundrel\">grimy halfling scoundrel</a><popBold/> says, \"Alright, here's your <a exist=\"6\" noun=\"chest\">iron chest</a> back.\"\n<right exist=\"6\" noun=\"chest\">iron chest</right>\n<prompt time=\"1002\">&gt;</prompt>\n";

/// The backpack, listed and empty.
fn an_empty_pack(state: &mut GameState) {
    inside(state, "902", &[]);
}

/// A chest back from the pool whose contents were never listed is kept,
/// not thrown out with whatever it holds (`box_loot`, `eloot.lic:5096`; the
/// crate review of 2026-10-01, BE-E-5).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_returned_chest_never_looked_into_goes_back_in_the_pack() {
    let errand = Errand::Pool {
        drop: false,
        collect: true,
    };
    let (transcript, task, _) = set_out_knowing(
        errand,
        Vec::new(),
        (TOWN, RETURNS),
        &Arc::default(),
        &FullBags::default(),
        an_empty_pack,
    );
    transcript.answer("east", AT_THE_POOL);
    transcript.answer("ask #71 for return", CHEST_BACK);
    transcript.answer(
        "_drag #6 #902",
        &reply("You put an iron chest in your backpack.\n<right>Empty</right>"),
    );
    transcript.answer(
        "ask #71 for return",
        &reply("The grimy halfling scoundrel says, \"We don't have any boxes ready for you.\""),
    );
    transcript.answer("west", &arrival(1001));
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(errand))),
        "{lines:?}"
    );
    assert!(
        in_order(
            &lines,
            &[
                "east",
                "ask #71 for return",
                "open #6",
                "look in #6",
                "_drag #6 #902",
                "ask #71 for return",
                "west",
            ]
        ),
        "{lines:?}"
    );
    assert_eq!(at(&lines, "trash #6"), None, "{lines:?}");
}

/// A chest holding an emerald no bag will take is not thrown out: `loot pool`
/// sells nothing, so it stays in hand, and the round says so.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_returned_chest_no_bag_will_empty_stays_in_hand_and_is_said() {
    let errand = Errand::Pool {
        drop: false,
        collect: true,
    };
    let (transcript, task, mut told) = set_out_knowing(
        errand,
        Vec::new(),
        (TOWN, RETURNS),
        &Arc::default(),
        &FullBags::default(),
        an_empty_pack,
    );
    transcript.answer("east", AT_THE_POOL);
    transcript.answer("ask #71 for return", CHEST_BACK);
    transcript.answer(
        "look in #6",
        &reply("<inv id='6'>In the <a exist=\"6\" noun=\"chest\">iron chest</a>:</inv>\n<inv id='6'> an <a exist=\"61\" noun=\"emerald\">uncut emerald</a></inv>"),
    );
    transcript.answer(
        "loot #61",
        &reply("The uncut emerald won't fit in the backpack."),
    );
    transcript.answer("west", &arrival(1001));
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(errand))),
        "{lines:?}"
    );
    assert!(in_order(&lines, &["east", "loot #61", "west"]), "{lines:?}");
    assert_eq!(at(&lines, "trash #6"), None, "{lines:?}");
    let said = drive_support::told_so_far(&mut told);
    assert!(
        said.iter()
            .any(|(_, text)| text.contains("a box from the pool is still in hand")),
        "{said:?}"
    );
}
