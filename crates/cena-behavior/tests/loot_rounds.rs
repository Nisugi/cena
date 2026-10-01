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
use cena_session::{Frame, GameState};
use drive_support::arrival;
use errand_support::{at, ended, in_order, inside, link, reply, set_out_choosing, set_out_knowing};

/// Home (1), the pool east of it (2), whose worker the map names, the bank
/// west of it (3), the gem shop north (4) and the pawnshop south (5).
const TOWN: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
    {"to":2,"kind":"cardinal","cmd":"east","cost":1},
    {"to":3,"kind":"cardinal","cmd":"west","cost":1},
    {"to":4,"kind":"cardinal","cmd":"north","cost":1},
    {"to":5,"kind":"cardinal","cmd":"south","cost":1}]},
  {"id":2,"uid":[1002],"tags":["locksmith pool"],
   "meta":["boxpool:npc:grimy halfling scoundrel"],"exits":[
    {"to":1,"kind":"cardinal","cmd":"west","cost":1}]},
  {"id":3,"uid":[1003],"tags":["bank"],"exits":[
    {"to":1,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":4,"uid":[1004],"tags":["gemshop"],"exits":[
    {"to":1,"kind":"cardinal","cmd":"south","cost":1}]},
  {"id":5,"uid":[1005],"tags":["pawnshop"],"exits":[
    {"to":1,"kind":"cardinal","cmd":"north","cost":1}]}
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

/// A profile that sells gems, but never a diamond.
const SELLS_GEMS: &str =
    "take = [\"gem\"]\n\n[town]\nsell_loot_types = [\"gem\"]\nsell_exclude = [\"diamond\"]\n";

/// `loot sell` (`;eloot sell`, `Sell.sell`, `eloot.lic:7812-7847`): the
/// emerald sold at the gem shop item by item -- the backpack holds a
/// diamond the profile keeps, so it is not sold whole -- then the bank, then
/// home, and what it came to said.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn loot_sell_sells_at_the_gem_shop_banks_and_comes_home() {
    let (transcript, task, mut told) = set_out_knowing(
        Errand::Sell,
        Vec::new(),
        (TOWN, SELLS_GEMS),
        &Arc::default(),
        &FullBags::default(),
        |state| {
            inside(
                state,
                "902",
                &[
                    ("61", "emerald", "uncut emerald"),
                    ("62", "diamond", "uncut diamond"),
                ],
            );
        },
    );
    transcript.answer("north", &arrival(1004));
    transcript.answer(
        "get #61",
        &reply("You remove an uncut emerald from in your backpack.\n<right exist=\"61\" noun=\"emerald\">uncut emerald</right>"),
    );
    transcript.answer(
        "sell #61",
        &reply("Arnalto takes the uncut emerald, gives it a careful examination and hands you 900 silver for it.\n<right>Empty</right>"),
    );
    transcript.answer("south", &arrival(1001));
    transcript.answer("west", &arrival(1003));
    transcript.answer(
        "deposit all",
        &reply("You deposit 900 silvers into your account."),
    );
    transcript.answer("east", &arrival(1001));
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Sell))),
        "{lines:?}"
    );
    assert!(
        in_order(
            &lines,
            &[
                "north",
                "get #61",
                "sell #61",
                "south",
                "west",
                "deposit all",
                "east",
            ]
        ),
        "{lines:?}"
    );
    assert_eq!(
        at(&lines, "get #62"),
        None,
        "the diamond is kept: {lines:?}"
    );
    assert_eq!(at(&lines, "sell #902"), None, "not sold whole: {lines:?}");
    let said = drive_support::told_so_far(&mut told);
    assert!(
        said.iter()
            .any(|(_, text)| text.contains("the round came to") && text.contains("900")),
        "{said:?}"
    );
}

/// The chest `6` in the right hand, holding `inside`.
fn a_chest_in_hand(state: &mut GameState) {
    state.apply(&Frame::RightHand {
        item: "iron chest".to_owned(),
        link: Some(link("6", "chest", "iron chest")),
    });
    inside(state, "902", &[]);
}

/// The chest's `look in`: these things in it.
fn chest_holds(line: &str) -> Vec<u8> {
    reply(&format!(
        "<inv id='6'>In the <a exist=\"6\" noun=\"chest\">iron chest</a>:</inv>\n<inv id='6'> {line}</inv>"
    ))
}

/// The chest listed empty, as the game lists it.
const CHEST_EMPTY: &str = "<clearContainer id=\"6\"/>\n<inv id='6'>In the <a exist=\"6\" noun=\"chest\">iron chest</a>:</inv>\n<inv id='6'> nothing</inv>";

/// `loot box` with no receptacle here takes the emptied chest to the one in
/// the pool's room and comes back (`save_trash_box`, `eloot.lic:7786-7791`).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn loot_box_takes_the_box_to_a_bin_and_comes_back() {
    let (transcript, task, _) = set_out_knowing(
        Errand::Box,
        Vec::new(),
        (TOWN, RETURNS),
        &Arc::default(),
        &FullBags::default(),
        a_chest_in_hand,
    );
    transcript.answer(
        "look in #6",
        &chest_holds("an <a exist=\"61\" noun=\"emerald\">uncut emerald</a>"),
    );
    transcript.answer(
        "loot #61",
        &reply(&format!(
            "You put an uncut emerald in your backpack.\n{CHEST_EMPTY}"
        )),
    );
    transcript.answer(
        "trash #6",
        &reply("You do not notice a trash receptacle here."),
    );
    transcript.answer("east", &arrival(1002));
    transcript.answer(
        "trash #6",
        &reply("You drop an iron chest in the barrel.\n<right>Empty</right>"),
    );
    transcript.answer("west", &arrival(1001));
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Box))),
        "{lines:?}"
    );
    assert!(
        in_order(
            &lines,
            &[
                "open #6",
                "look in #6",
                "loot #61",
                "trash #6",
                "east",
                "trash #6",
                "west",
            ]
        ),
        "{lines:?}"
    );
    assert_eq!(at(&lines, "drop #6"), None, "{lines:?}");
}

/// `loot box` whose coins will not all fit goes to the bank and back, and
/// gathers the rest (`box_loot`, `eloot.lic:5109-5115`).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn loot_box_banks_for_coins_that_will_not_fit_and_gathers_the_rest() {
    let (transcript, task, _) = set_out_knowing(
        Errand::Box,
        Vec::new(),
        (TOWN, RETURNS),
        &Arc::default(),
        &FullBags::default(),
        a_chest_in_hand,
    );
    let coins = "some <a exist=\"63\" noun=\"coins\">900 silver coins</a>";
    transcript.answer("look in #6", &chest_holds(coins));
    transcript.answer(
        "get coins from #6",
        &reply("You cannot hold any more silvers."),
    );
    transcript.answer("west", &arrival(1003));
    transcript.answer(
        "deposit all",
        &reply("You deposit 5,000 silvers into your account."),
    );
    transcript.answer("east", &arrival(1001));
    transcript.answer("look in #6", &chest_holds(coins));
    transcript.answer(
        "get coins from #6",
        &reply(&format!(
            "You gather the remaining 900 coins from inside your iron chest.\n{CHEST_EMPTY}"
        )),
    );
    transcript.answer(
        "trash #6",
        &reply("You drop an iron chest in the barrel.\n<right>Empty</right>"),
    );
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Box))),
        "{lines:?}"
    );
    assert!(
        in_order(
            &lines,
            &[
                "look in #6",
                "get coins from #6",
                "west",
                "deposit all",
                "east",
                "look in #6",
                "get coins from #6",
                "trash #6",
            ]
        ),
        "{lines:?}"
    );
}

/// A profile that sells gems and weapons, but never a diamond.
const GEMS_AND_WEAPONS: &str = "take = [\"gem\"]\n\n[town]\nsell_loot_types = [\"gem\", \"weapon\"]\nsell_exclude = [\"diamond\"]\n";

/// `loot sell type gem` (eloot's `--type`, `custom_type`,
/// `eloot.lic:6744-6813`): the emerald sold, the bank, home; the poignard the
/// profile sells too left for another round, the pawnshop not walked to.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn loot_sell_type_gem_sells_the_gem_and_leaves_the_poignard() {
    let (transcript, task, _) = set_out_choosing(
        cena_behavior::town::Choice::Kinds(vec!["gem".to_owned()]),
        (TOWN, GEMS_AND_WEAPONS),
        |state| {
            inside(
                state,
                "902",
                &[
                    ("61", "emerald", "uncut emerald"),
                    ("62", "diamond", "uncut diamond"),
                    ("63", "poignard", "steel poignard"),
                ],
            );
        },
    );
    transcript.answer("north", &arrival(1004));
    transcript.answer(
        "get #61",
        &reply("You remove an uncut emerald from in your backpack.\n<right exist=\"61\" noun=\"emerald\">uncut emerald</right>"),
    );
    transcript.answer(
        "sell #61",
        &reply("Arnalto takes the uncut emerald, gives it a careful examination and hands you 900 silver for it.\n<right>Empty</right>"),
    );
    // From the gem shop home, and -- were the poignard sold -- on to the
    // pawnshop.
    transcript.answer("south", &arrival(1001));
    transcript.answer("south", &arrival(1005));
    transcript.answer("west", &arrival(1003));
    transcript.answer(
        "deposit all",
        &reply("You deposit 900 silvers into your account."),
    );
    transcript.answer("east", &arrival(1001));
    let end = ended(task).await;
    let lines = transcript.lines();
    assert_eq!(
        end,
        Some(HuntEnd::Finished(Ending::Looted(Errand::Sell))),
        "{lines:?}"
    );
    assert!(
        in_order(
            &lines,
            &[
                "north",
                "get #61",
                "sell #61",
                "south",
                "west",
                "deposit all",
                "east"
            ]
        ),
        "{lines:?}"
    );
    assert_eq!(at(&lines, "get #63"), None, "the poignard stays: {lines:?}");
}
