//! The loot classifier against the lines `loottracker.lic` cites as `# Real:`,
//! and against the searches in the hunt's replay fixtures.
//!
//! Every wire line here is one the script's comments quote from a real
//! session (`reference/scripts/scripts/loottracker.lic:541-750`), with the
//! script's own elisions (`<box>`, `<locksmith>`) filled in with the shape the
//! wire uses. The point of parsing them rather than building `ChunkLine`s by
//! hand is that the classifier's whole premise -- creature is the bolded link,
//! item is the unbolded one -- is a claim about what the parser leaves on the
//! line.

use cena_model::GameState;
use cena_model::state::chunks::Chunk;
use cena_model::state::ledger::{Appraiser, Buyer, Find, LootFact, classify};
use cena_protocol::Frame;
use cena_protocol::Parser;

/// Fold wire with no prompt, so the chunk stays open, and take it.
fn chunk(wire: &str) -> Chunk {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state.open_chunk().clone()
}

fn facts(wire: &str) -> Vec<LootFact> {
    classify(&chunk(wire))
}

fn one(wire: &str) -> Option<LootFact> {
    let mut all = facts(wire);
    (all.len() == 1).then(|| all.remove(0))
}

/// `loottracker.lic:541-574`'s search lines, one search's worth, as wire.
const BERSERKER: &str = include_str!("fixtures/loot/berserker_search.xml");

#[test]
fn a_search_gathers_its_silver_items_and_finds() {
    let Some(LootFact::Searched {
        creature,
        silvers,
        items,
        finds,
    }) = one(BERSERKER)
    else {
        panic!("one search: {:?}", facts(BERSERKER));
    };
    assert_eq!(creature.id, "393774588");
    assert_eq!(creature.noun, "berserker");
    assert_eq!(silvers, 344);
    let nouns: Vec<&str> = items.iter().map(|i| i.noun.as_str()).collect();
    assert_eq!(
        nouns,
        ["deathstone", "sword", "strongbox", "leaf", "crystal"],
        "had, had (simple), carried, Interesting/carried, left behind"
    );
    assert_eq!(finds.len(), 4, "{finds:?}");
    assert!(matches!(&finds[0], Find::Klock(k) if k.noun == "key"));
    assert_eq!(finds[1], Find::GemDust);
    assert!(matches!(&finds[2], Find::Jewel(j) if j.id == "309578537"));
    assert_eq!(finds[3], Find::Boost(1));
}

#[test]
fn the_search_result_lines_mean_nothing_without_a_search() {
    let orphan = "<pushBold/><a exist=\"1\" noun=\"berserker\">He</a><popBold/> had 344 silvers on <pushBold/><a exist=\"1\" noun=\"berserker\">him</a><popBold/>.\n";
    assert_eq!(facts(orphan), []);
}

#[test]
fn rifling_through_belongings_is_a_search_item() {
    let wire = concat!(
        "You search the <pushBold/><a exist=\"420754492\" noun=\"mutant\">mutant</a><popBold/>.\n",
        "While rifling through <pushBold/>the <a exist=\"420754492\" noun=\"mutant\">mutant's</a><popBold/> belongings, you find a <a exist=\"420755239\" noun=\"idol\">silver-veined black draconic idol</a>!\n",
    );
    let Some(LootFact::Searched { items, .. }) = one(wire) else {
        panic!("{:?}", facts(wire));
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].noun, "idol");
}

#[test]
fn skinning_names_the_corpse_and_the_skin() {
    let wire = "You skinned the <pushBold/><a exist=\"7\" noun=\"brawler\">triton brawler</a><popBold/>, yielding a <a exist=\"8\" noun=\"hide\">darkened triton hide</a>.\n";
    let Some(LootFact::Skinned { creature, skin }) = one(wire) else {
        panic!("{:?}", facts(wire));
    };
    assert_eq!(creature.id, "7");
    assert_eq!(skin.id, "8");
}

#[test]
fn the_three_bundle_lines() {
    let create = "As you place your <a exist=\"1\" noun=\"hide\">triton hide</a> inside your <a exist=\"2\" noun=\"shroud\">shroud</a>, you notice another <a exist=\"3\" noun=\"hide\">triton hide</a> inside the <a exist=\"2\" noun=\"shroud\">shroud</a> and carefully arrange the two <a exist=\"4\" noun=\"hides\">triton hides</a> into a neat bundle.\n";
    assert!(matches!(
        one(create),
        Some(LootFact::Bundled { skin: Some(s), bundle, container: Some(c), created: true })
            if s.id == "1" && bundle.id == "4" && c.id == "2"
    ));
    let add = "You carefully add your <a exist=\"5\" noun=\"hide\">triton hide</a> to your bundle of <a exist=\"4\" noun=\"hides\">triton hides</a> inside your <a exist=\"2\" noun=\"shroud\">shroud</a>.\n";
    assert!(matches!(
        one(add),
        Some(LootFact::Bundled { skin: Some(s), bundle, container: Some(c), created: false })
            if s.id == "5" && bundle.id == "4" && c.id == "2"
    ));
    let manual = "You carefully arrange your two <a exist=\"4\" noun=\"hides\">triton hides</a> into a neat bundle.\n";
    assert!(matches!(
        one(manual),
        Some(LootFact::Bundled { skin: None, bundle, container: None, created: true }) if bundle.id == "4"
    ));
}

#[test]
fn the_bounty_reward_and_the_bank() {
    assert_eq!(
        one("[You have earned 925 bounty points, 800 experience points, and 9250 silver.]\n"),
        Some(LootFact::Bounty {
            points: 925,
            experience: 800,
            silvers: 9250
        })
    );
    assert_eq!(
        one(
            "You deposit 9,250 silvers into your account.  The teller carefully records the transaction.\n"
        ),
        Some(LootFact::Deposited(9250))
    );
    assert_eq!(
        one(
            "The teller carefully records the transaction, hands you 8,000 silvers, and says, \"This brings your total to 455,394 silvers.\"\n"
        ),
        Some(LootFact::Withdrew(8000))
    );
    assert_eq!(
        one(
            "She says, \"That's a total of 50,683 silvers, bringing your balance to 234,039 silvers.\"\n"
        ),
        Some(LootFact::NoteDeposited(50_683))
    );
}

#[test]
fn a_wand_duplicated_carries_its_donor_when_the_gesture_was_in_the_chunk() {
    let wire = concat!(
        "You gesture at a <a exist=\"9\" noun=\"wand\">bloodwood wand</a>.\n",
        "The glow fades from both objects and they solidify, looking almost identical to the original, and feeling a little lighter.\n",
    );
    assert!(matches!(
        one(wire),
        Some(LootFact::WandDuplicated { donor: Some(w) }) if w.id == "9"
    ));
    let alone = "The glow fades from both objects and they solidify, looking almost identical to the original, and feeling a little lighter.\n";
    assert_eq!(one(alone), Some(LootFact::WandDuplicated { donor: None }));
}

#[test]
fn boxes_opened_returned_quoted_and_dropped() {
    let gathered = "You gather the remaining 1,576 <a exist=\"517651953\" noun=\"coins\">coins</a> from inside your <a exist=\"517651949\" noun=\"strongbox\">maoral strongbox</a>.\n";
    assert!(matches!(
        one(gathered),
        Some(LootFact::BoxOpened { item, silvers: 1576 }) if item.id == "517651949"
    ));
    let charm = "You summon a swarm of corpse flies from your <a exist=\"1\" noun=\"charm\">bone charm</a>.  They swarm inside an <a exist=\"2\" noun=\"coffer\">enruned steel coffer</a> and locate a pile of 2,090 <a exist=\"3\" noun=\"coins\">coins</a>, reclaiming them for you.\n";
    assert!(matches!(
        one(charm),
        Some(LootFact::BoxOpened { item, silvers: 2090 }) if item.id == "2"
    ));
    let returned = "<pushBold/>The <a exist=\"-100\" noun=\"Wehnimer\">locksmith Wehnimer</a><popBold/> says, \"Alright, here's your <a exist=\"12345\" noun=\"strongbox\">simple haon strongbox</a> back.\"\n";
    assert!(matches!(
        one(returned),
        Some(LootFact::BoxReturned { item }) if item.id == "12345"
    ));
    let quote = "You want a locksmith to open a <a exist=\"12345\" noun=\"strongbox\">simple haon strongbox</a> for a tip of 219 silvers, or 3 percent of the box value.  There is also a fee of 2,716 silvers due up front.\n";
    assert!(matches!(
        one(quote),
        Some(LootFact::PoolQuoted { item, tip: 219, fee: 2716 }) if item.id == "12345"
    ));
    let dropped = "<pushBold/>The <a exist=\"-100\" noun=\"Wehnimer\">locksmith Wehnimer</a><popBold/> takes your chest and says, \"Your tip of 169 silvers has been recorded, and the 2001 silver fee has been collected.  We'll get someone on that right away.\"\n";
    assert_eq!(
        one(dropped),
        Some(LootFact::PoolDropped {
            noun: "chest".to_owned(),
            tip: 169,
            fee: 2001
        })
    );
}

#[test]
fn appraisals_by_hand_and_by_song() {
    let gem = include_str!("fixtures/loot/gem_appraise.xml");
    assert!(matches!(
        one(gem),
        Some(LootFact::Appraised { item: Some(i), value: Some(4000), by: Appraiser::Gem }) if i.id == "321408923"
    ));
    let skin = "You turn the <a exist=\"691558\" noun=\"hide\">darkened triton hide</a> over in your hands, meticulously inspecting for flaws.  You estimate that the <a exist=\"691558\" noun=\"hide\">darkened triton hide</a> is of magnificent quality and worth approximately 25 silvers!\n";
    assert!(matches!(
        one(skin),
        Some(LootFact::Appraised { item: Some(i), value: Some(25), by: Appraiser::Skin }) if i.id == "691558"
    ));
    let bundle = "You estimate that the total value of your <a exist=\"4\" noun=\"hides\">triton hides</a> is approximately 75 silvers.\n";
    assert!(matches!(
        one(bundle),
        Some(LootFact::Appraised { item: Some(i), value: Some(75), by: Appraiser::Skin }) if i.id == "4"
    ));

    // The loresong's two lines arrive in separate chunks: each half is
    // reported with its other side missing, for the ledger to pair.
    let sung = "As you sing, you feel a faint resonating vibration from the <a exist=\"554177549\" noun=\"ivory\">age-darkened ivory</a> in your hand, and you learn something about it...\n";
    assert!(matches!(
        one(sung),
        Some(LootFact::Appraised { item: Some(i), value: None, by: Appraiser::Loresong }) if i.id == "554177549"
    ));
    let valued = "This is a small item, under a pound.  In your best estimation, it's worth about 1,400 silvers, and is of outstanding quality.\n";
    assert_eq!(
        one(valued),
        Some(LootFact::Appraised {
            item: None,
            value: Some(1400),
            by: Appraiser::Loresong
        })
    );
    // Both in one chunk: paired here.
    let both = format!("{sung}{valued}");
    assert!(matches!(
        one(&both),
        Some(LootFact::Appraised { item: Some(i), value: Some(1400), by: Appraiser::Loresong }) if i.id == "554177549"
    ));

    let shatter = "Your focused voice causes the <a exist=\"143218758\" noun=\"diamond\">uncut diamond</a> to shatter into thousands of fragments!\n";
    assert!(matches!(one(shatter), Some(LootFact::Shattered { item }) if item.id == "143218758"));
}

#[test]
fn shop_appraisals_in_their_three_shapes() {
    let pawn = concat!(
        "<pushBold/><a exist=\"-200\" noun=\"Gryhm\">Walsor Gryhm</a><popBold/> turns the <a exist=\"568226735\" noun=\"aventail\">aventail</a> over in his hands a few times.\n",
        "<pushBold/><a exist=\"-200\" noun=\"Gryhm\">Walsor Gryhm</a><popBold/> says, \"Hmm, a most impressive bit of protection of note.  I'll give you 9,472 silver coins for it.\"\n",
    );
    assert!(matches!(
        one(pawn),
        Some(LootFact::Appraised { item: Some(i), value: Some(9472), by: Appraiser::Shop }) if i.id == "568226735"
    ));
    let jeweler = "The <pushBold/><a exist=\"-300\" noun=\"jeweler\">jeweler</a><popBold/> takes the <a exist=\"568262930\" noun=\"ingot\">gold ingot</a> and inspects it carefully before saying, \"I'll give you 7,825 silvers for it, if you're interested.\"\n";
    assert!(matches!(
        one(jeweler),
        Some(LootFact::Appraised { item: Some(i), value: Some(7825), by: Appraiser::Shop }) if i.id == "568262930"
    ));
    let worth = "Sniffberry shrugs before saying, \"That <a exist=\"77\" noun=\"stickpin\">enruned gold stickpin</a> looks decent, probably worth about 83,058 silvers.\"\n";
    assert!(matches!(
        one(worth),
        Some(LootFact::Appraised { item: Some(i), value: Some(83_058), by: Appraiser::Shop }) if i.id == "77"
    ));
}

#[test]
fn the_pawnbroker_pays_writes_a_chit_or_refuses() {
    let paid = concat!(
        "You offer to sell your <a exist=\"339188773\" noun=\"wand\">bloodwood wand</a> to Bushybrow.\n",
        "Bushybrow takes your <a exist=\"339188773\" noun=\"wand\">bloodwood wand</a>, glances at it briefly, then hands you 582 silver coins.\n",
    );
    assert!(matches!(
        one(paid),
        Some(LootFact::Sold { item: Some(i), silvers: 582, to: Buyer::Pawn, note: None }) if i.id == "339188773"
    ));
    let chit = concat!(
        "You offer to sell your <a exist=\"50\" noun=\"aventail\">aventail</a> to Bushybrow.\n",
        "He scribbles out a <a exist=\"339455621\" noun=\"chit\">salt-stained kraken chit</a> for 25,000 silvers and hands it to you.\n",
    );
    assert!(matches!(
        one(chit),
        Some(LootFact::Sold { item: Some(i), silvers: 25_000, to: Buyer::Pawn, note: Some(n) })
            if i.id == "50" && n.id == "339455621"
    ));
    let worthless = concat!(
        "You offer to sell your <a exist=\"51\" noun=\"rock\">grey rock</a> to Bushybrow.\n",
        "Bushybrow says, \"That's basically worthless here, Lorwyn.  Maybe you can find a buyer somewhere in town, but I doubt it.\"\n",
    );
    assert!(matches!(
        one(worthless),
        Some(LootFact::Worthless { item: Some(i) }) if i.id == "51"
    ));
}

#[test]
fn the_gem_shop_furrier_and_chronomage() {
    let single = "The <pushBold/><a exist=\"-480255\" noun=\"Krosane\">jeweler Krosane</a><popBold/> takes the <a exist=\"60\" noun=\"sapphire\">star sapphire</a>, gives it a careful examination and hands you 1,250 silver for it.\n";
    assert!(matches!(
        one(single),
        Some(LootFact::Sold { item: Some(i), silvers: 1250, to: Buyer::Gemshop, note: None }) if i.id == "60"
    ));
    let bulk = "The <pushBold/><a exist=\"-480255\" noun=\"Krosane\">jeweler Krosane</a><popBold/> takes the <a exist=\"61\" noun=\"shroud\">shroud</a>, inspects the contents carefully, and removes the gems he is interested in.  He hands it back to you, along with 8,383 silver.\n";
    assert!(matches!(
        one(bulk),
        Some(LootFact::Sold { item: Some(i), silvers: 8383, to: Buyer::Gemshop, note: None }) if i.id == "61"
    ));
    let bulk_chit = "The <pushBold/><a exist=\"-480255\" noun=\"Krosane\">jeweler Krosane</a><popBold/> removes the gems and hands you a <a exist=\"62\" noun=\"chit\">gem chit</a> for 533,549 silvers.\n";
    assert!(matches!(
        one(bulk_chit),
        Some(LootFact::Sold { item: None, silvers: 533_549, to: Buyer::Gemshop, note: Some(n) }) if n.id == "62"
    ));
    let note = "The gemcutter takes the <a exist=\"63\" noun=\"nugget\">platinum nugget</a>, gives it a careful examination and hands you a <a exist=\"25965764\" noun=\"note\">Vornavis promissory note</a> for 33,250 silvers.\n";
    assert!(matches!(
        one(note),
        Some(LootFact::Sold { item: Some(i), silvers: 33_250, to: Buyer::Gemshop, note: Some(n) })
            if i.id == "63" && n.id == "25965764"
    ));
    let rejected = concat!(
        "You ask Kahlyr if she would like to buy an <a exist=\"20230323\" noun=\"stickpin\">enruned gold stickpin</a>.\n",
        "The <pushBold/><a exist=\"92482\" noun=\"Kahlyr\">jeweler Kahlyr</a><popBold/> says, \"Sorry, Lorwyn, I'm not buying anything this valuable today.  Maybe tomorrow.\"\n",
    );
    assert!(matches!(
        one(rejected),
        Some(LootFact::TooValuable { item: Some(item) }) if item.id == "20230323"
    ));
    // The answer is usually a prompt later: the offer alone is reported.
    let offer = "You offer to sell your <a exist=\"339188773\" noun=\"wand\">bloodwood wand</a> to Bushybrow.
";
    assert!(matches!(
        one(offer),
        Some(LootFact::Offered { item }) if item.id == "339188773"
    ));
    let furrier = "Delosa takes the <a exist=\"339211582\" noun=\"hide\">hide</a>, scrutinizes it carefully, then hands you 12 silvers.\n";
    assert!(matches!(
        one(furrier),
        Some(LootFact::Sold { item: Some(i), silvers: 12, to: Buyer::Furrier, note: None }) if i.id == "339211582"
    ));
    let furrier_bulk = "Delosa takes the <a exist=\"64\" noun=\"shroud\">shroud</a>, inspects the contents carefully and removes the item he is interested in.  Delosa hands it back to you, along with 127 silver.\n";
    assert!(matches!(
        one(furrier_bulk),
        Some(LootFact::Sold { item: Some(i), silvers: 127, to: Buyer::Furrier, note: None }) if i.id == "64"
    ));
    let ring = "<pushBold/>A <a exist=\"522933755\" noun=\"halfling\">finely-dressed halfling</a><popBold/> gleefully snatches a <a exist=\"522418497\" noun=\"ring\">braided gold ring</a> from your outreached hand and exclaims, \"Splendid!  In return I'll charge you 5,000 silvers less for your next travel ticket.\"\n";
    assert!(matches!(
        one(ring),
        Some(LootFact::Sold { item: Some(i), silvers: 5000, to: Buyer::Chronomage, note: None }) if i.id == "522418497"
    ));
}

/// Replay one of the hunt's fixtures, classifying every chunk as its prompt
/// closes it, and return the searches in order.
fn searches_in(fixture: &str) -> Vec<(String, u64, usize)> {
    let path = format!(
        "{}/../cena-behavior/tests/fixtures/{fixture}",
        env!("CARGO_MANIFEST_DIR")
    );
    let wire = std::fs::read(path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut state = GameState::default();
    let mut found = Vec::new();
    for frame in parser.push_bytes(&wire) {
        if matches!(frame, Frame::Prompt { .. }) {
            for fact in classify(state.open_chunk()) {
                if let LootFact::Searched {
                    creature,
                    silvers,
                    items,
                    ..
                } = fact
                {
                    found.push((creature.noun, silvers, items.len()));
                }
            }
        }
        state.apply(&frame);
    }
    found
}

#[test]
fn the_replay_fixtures_searches_are_read() {
    assert_eq!(
        searches_in("arch_kill.xml"),
        [
            ("mastodon".to_owned(), 0, 0),
            ("shield-maiden".to_owned(), 596, 0)
        ],
        "the mastodon carried no silver; the shield-maiden had 596 on her"
    );
    assert_eq!(
        searches_in("smithy_kill.xml"),
        [("pegasus".to_owned(), 0, 0)]
    );
    assert_eq!(searches_in("smithy_engage.xml"), []);
}
