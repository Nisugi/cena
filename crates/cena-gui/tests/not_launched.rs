//! `plan/49` Stage C, as the author revised it on 2026-09-27: the hub's Not
//! launched tab. It holds the roster's characters that are off the table as
//! cards, a login by account that lists the account's characters, and the
//! kept passwords. Driven through `egui_kittest`, as `hub.rs` is.

mod common;

use cena_ui::{Account, HubRequest, Listing, Login, Password, Saved};
use common::{Board, ask, board, not_launched, roster_card, type_into};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// Whether the card named `first` comes before the one named `then`, as the
/// grid is read: row by row, left to right.
fn before(harness: &Harness<'_, Board>, first: &str, then: &str) -> bool {
    let at = |name: &str| {
        let at = harness.get_by_role_and_label(Role::Label, name).rect().min;
        (at.y.round(), at.x.round())
    };
    let (first, then) = (at(first), at(then));
    first.0 < then.0 || (first.0 - then.0).abs() < 1.0 && first.1 < then.1
}

/// Log in as `account` with `password`, as the player does.
fn log_in(harness: &mut Harness<'_, Board>, account: &str, password: &str) {
    type_into(harness, Role::TextInput, "Account", account);
    type_into(harness, Role::PasswordInput, "Password", password);
    harness.get_by_label("Log in").click();
    harness.run();
}

/// Only the roster's characters off the table are cards here. They are
/// starred first, then by name, and counted on the tab. One whose password
/// is kept starts with a click; one whose is not asks for it. A playing one
/// and a closed one are both on the table, so neither is here. Only the
/// accounts with kept passwords are listed to forget.
#[test]
fn the_cards_are_the_roster_off_the_table_starred_first() {
    let mut waiting = board();
    let mut zed = roster_card("Zed", "zed01", "GS3", true);
    zed.favourite = true;
    waiting.roster.push(zed);
    waiting
        .roster
        .push(roster_card("Lorwyn", "lorwyn01", "GS3", false));
    waiting.offered.push("Zed".to_owned());
    let harness = not_launched(waiting);
    assert!(harness.query_by_label("Not launched (3)").is_some());
    assert!(before(&harness, "Zed", "Orsen"), "the star first");
    assert!(before(&harness, "Orsen", "Wyla"), "then by name");
    for on_table in ["Ashryn", "Lorwyn"] {
        assert!(
            harness
                .query_by_role_and_label(Role::Label, on_table)
                .is_none(),
            "{on_table} is on the table"
        );
    }
    assert!(harness.query_by_label("Start Orsen").is_some());
    assert!(harness.query_by_label("Start Wyla").is_none(), "none kept");
    assert!(
        harness
            .query_by_role_and_label(Role::PasswordInput, "Wyla")
            .is_some()
    );
    assert!(
        harness.query_by_label("Shattered · wyla01").is_some(),
        "the game by name"
    );
    assert_eq!(harness.query_all_by_label("★").count(), 1);
    assert!(harness.query_by_label("Forget wyla01's password").is_none());
    assert!(
        harness
            .query_by_label("Forget orsen01's password")
            .is_some()
    );
}

/// Each card asks for its own character by the name the roster reads:
/// - a start, by the name it was offered as;
/// - a login, with the password typed there and its box;
/// - a star;
/// - a forget.
///
/// A password typed goes once, and leaves the card.
#[test]
fn each_card_asks_for_its_own() {
    let mut harness = not_launched(board());
    harness.get_by_label("Start Orsen").click();
    harness.run();
    type_into(&mut harness, Role::PasswordInput, "Wyla", "hunter2");
    harness.get_by_label("Keep").click();
    harness.run();
    harness.get_by_label("Log Wyla in").click();
    harness.run();
    assert_eq!(
        harness
            .get_by_role_and_label(Role::PasswordInput, "Wyla")
            .value()
            .as_deref(),
        Some(""),
        "the password left the card"
    );
    // The cards are Orsen's, then Wyla's: Orsen's star, then Wyla's Forget.
    if let Some(star) = harness.get_all_by_label("☆").next() {
        star.click();
    }
    harness.run();
    if let Some(forget) = harness.get_all_by_label("Forget").nth(1) {
        forget.click();
    }
    harness.run();
    harness.get_by_label("Forget orsen01's password").click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [
            ask(HubRequest::Add("Orsen".to_owned())),
            ask(HubRequest::Login(Login {
                account: "wyla01".to_owned(),
                password: Password::new("hunter2".to_owned()),
                game: "GSF".to_owned(),
                character: "Wyla".to_owned(),
                remember: true,
            })),
            ask(HubRequest::Favourite("GS3:Orsen".to_owned(), true)),
            ask(HubRequest::Forget("GSF:Wyla".to_owned())),
            ask(HubRequest::ForgetPassword("orsen01".to_owned())),
        ]
    );
}

/// The login asks for no character. Its game is Prime unless the player
/// picks another, and nothing is sent until the account and password are
/// there. The password is never shown, and leaves the form once sent,
/// though it is held (never printed) for playing what the account lists.
#[test]
fn a_login_by_account_asks_for_its_characters() {
    let mut harness = not_launched(Board::default());
    assert!(
        harness
            .query_by_label("No saved character is waiting to be launched.")
            .is_some()
    );
    assert!(harness.query_by_label("No password is kept.").is_some());
    assert!(
        harness.query_by_label("Character").is_none(),
        "the listing names the characters"
    );
    harness.get_by_label("Log in").click();
    harness.run();
    assert!(harness.state().asked.is_empty(), "nothing typed yet");

    type_into(&mut harness, Role::TextInput, "Account", " newacct ");
    type_into(&mut harness, Role::PasswordInput, "Password", "hunter2");
    let shown = harness
        .get_by_role_and_label(Role::PasswordInput, "Password")
        .value()
        .unwrap_or_default();
    assert!(!shown.contains("hunter2"), "{shown}");
    harness.get_by_label("Log in").click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [ask(HubRequest::Characters(Account {
            account: "newacct".to_owned(),
            password: Password::new("hunter2".to_owned()),
            game: "GS3".to_owned(),
            remember: false,
        }))]
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::PasswordInput, "Password")
            .value()
            .as_deref(),
        Some("")
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Account")
            .value()
            .as_deref(),
        Some(" newacct "),
        "kept for a second try"
    );
    assert!(
        harness
            .query_by_label("Logged in as newacct on Prime.")
            .is_some()
    );
    assert!(!format!("{:?}", harness.state().hub).contains("hunter2"));

    // Again, keeping the password once the service proves it.
    type_into(&mut harness, Role::PasswordInput, "Password", "hunter2");
    harness.get_by_label("Keep the password").click();
    harness.run();
    harness.get_by_label("Log in").click();
    harness.run();
    assert_eq!(
        harness.state().asked.last(),
        Some(&ask(HubRequest::Characters(Account {
            account: "newacct".to_owned(),
            password: Password::new("hunter2".to_owned()),
            game: "GS3".to_owned(),
            remember: true,
        })))
    );
}

/// Once the service has listed the account's characters, each can be added
/// or forgotten, starred, and played. Playing uses the password held since
/// Log in, never typed again. One on the table says it is playing. Log out
/// lets go of the account, its list, and its password.
#[test]
fn the_accounts_characters_are_added_starred_and_played() {
    let mut listed = board();
    listed.listing = Some(Listing {
        account: "orsen01".to_owned(),
        game: "GS3".to_owned(),
        characters: ["Ashryn", "Orsen", "Newt"].map(str::to_owned).to_vec(),
    });
    let mut harness = not_launched(listed);
    assert!(
        harness.query_by_label("Play").is_none(),
        "not logged in: no list"
    );
    log_in(&mut harness, "orsen01", "hunter2");
    harness.state_mut().asked.clear();

    // Ashryn is on the roster, starred and playing; Orsen is on it; Newt
    // is not.
    assert!(harness.query_by_label("Playing").is_some(), "Ashryn");
    assert_eq!(harness.query_all_by_label("Play").count(), 2);
    harness.get_by_label("Add").click();
    harness.run();
    if let Some(newts) = harness.get_all_by_label("☆").last() {
        newts.click();
    }
    harness.run();
    harness.get_by_label("★").click();
    harness.run();
    // Orsen's and Wyla's cards, then the list's Ashryn and Orsen.
    if let Some(forget) = harness.get_all_by_label("Forget").nth(3) {
        forget.click();
    }
    harness.run();
    if let Some(play) = harness.get_all_by_label("Play").nth(1) {
        play.click();
    }
    harness.run();
    let newt = |favourite| Saved {
        character: "Newt".to_owned(),
        account: "orsen01".to_owned(),
        game: "GS3".to_owned(),
        favourite,
    };
    assert_eq!(
        harness.state().asked,
        [
            ask(HubRequest::Remember(newt(false))),
            ask(HubRequest::Remember(newt(true))),
            ask(HubRequest::Favourite("GS3:Ashryn".to_owned(), false)),
            ask(HubRequest::Forget("GS3:Orsen".to_owned())),
            ask(HubRequest::Login(Login {
                account: "orsen01".to_owned(),
                password: Password::new("hunter2".to_owned()),
                game: "GS3".to_owned(),
                character: "Newt".to_owned(),
                remember: false,
            })),
        ]
    );

    harness.get_by_label("Log out").click();
    harness.run();
    assert!(harness.query_by_label("Play").is_none(), "the list is gone");
    assert!(harness.query_by_label_contains("Logged in as").is_none());
}

/// A listing is shown only for the account and game logged in: another
/// account's, or this account's on another game, is not this login's.
#[test]
fn a_listing_is_shown_only_for_the_login_it_answers() {
    for (account, game) in [("someone", "GS3"), ("orsen01", "GSF")] {
        let mut listed = board();
        listed.listing = Some(Listing {
            account: account.to_owned(),
            game: game.to_owned(),
            characters: vec!["Newt".to_owned()],
        });
        let mut harness = not_launched(listed);
        log_in(&mut harness, "orsen01", "hunter2");
        assert!(
            harness.query_by_label("Play").is_none(),
            "{account} on {game}"
        );
    }
}

/// The tab as a player sees it, logged in with the account's characters
/// listed, rendered and compared with the committed image.
#[test]
fn the_not_launched_tab_as_drawn() {
    let mut listed = board();
    listed.listing = Some(Listing {
        account: "orsen01".to_owned(),
        game: "GS3".to_owned(),
        characters: ["Ashryn", "Orsen", "Newt"].map(str::to_owned).to_vec(),
    });
    let mut harness = Harness::builder()
        .with_size((720.0, 720.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), listed);
    harness.get_by_label_contains("Not launched").click();
    harness.run();
    log_in(&mut harness, "orsen01", "hunter2");
    harness.snapshot("hub_not_launched_listed");
}
