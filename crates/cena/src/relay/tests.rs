use super::*;
use cena_platform::AnsweringSource;
use cena_session::{Event, Session, SessionId};

/// A handle with no session behind it, for choosing among.
fn handle() -> SessionHandle {
    SessionHandle::new(
        tokio::sync::mpsc::channel(1).0,
        cena_session::GenerationCell::default(),
        tokio::sync::broadcast::channel(1).0,
    )
}

/// Characters running, each `GAME:Name`, or a name alone on ONE.
fn running(names: &[&str]) -> Vec<Running> {
    names
        .iter()
        .map(|named| {
            let (game, name) = named.split_once(':').unwrap_or(("ONE", named));
            Running {
                game: game.to_owned(),
                name: name.to_owned(),
                handle: handle(),
            }
        })
        .collect()
}

#[test]
fn the_two_words_and_what_they_carry() {
    assert_eq!(
        parse("to Baelor look at kobold"),
        Some(Ok(Relay::To {
            name: "Baelor".to_owned(),
            line: "look at kobold".to_owned()
        }))
    );
    assert_eq!(
        parse("ALL stand"),
        Some(Ok(Relay::All {
            who: Who::Everyone,
            line: "stand".to_owned()
        }))
    );
    assert_eq!(
        parse("all -Dicate, Maravel stand"),
        Some(Ok(Relay::All {
            who: Who::Except(vec!["Dicate".to_owned()]),
            line: "Maravel stand".to_owned()
        })),
        "a list is one word: a space ends it"
    );
    assert_eq!(
        parse("all +Nisugi,Dicate kneel"),
        Some(Ok(Relay::All {
            who: Who::Only(vec!["Nisugi".to_owned(), "Dicate".to_owned()]),
            line: "kneel".to_owned()
        }))
    );
    assert!(matches!(parse("all -Dicate"), Some(Err(_))), "no command");
    assert!(matches!(parse("all - stand"), Some(Err(_))), "no names");
    assert!(matches!(parse("to Baelor"), Some(Err(_))), "no command");
    assert!(matches!(parse("all"), Some(Err(_))), "no command");
    assert_eq!(parse("toast"), None, "another word");
    assert_eq!(parse("go2 bank"), None);
}

/// A whole name wins, whatever the case; otherwise the start of one name.
/// A character named All is reached by `;to`, never by `;all`.
/// `;all` leaves out, or keeps only, the characters named, each picked as
/// `;to` picks one; a name that picks nobody refuses the whole line.
#[test]
fn all_leaves_out_or_keeps_only_those_named() {
    let characters = running(&["Nisugi", "Dicate", "Maravel"]);
    let names = |who: Who| {
        chosen(&who, &characters)
            .map(|chosen| chosen.into_iter().map(|one| one.name).collect::<Vec<_>>())
    };
    assert_eq!(
        names(Who::Except(vec!["dic".to_owned()])),
        Ok(vec!["Nisugi".to_owned(), "Maravel".to_owned()])
    );
    assert_eq!(
        names(Who::Only(vec!["Maravel".to_owned(), "nisugi".to_owned()])),
        Ok(vec!["Nisugi".to_owned(), "Maravel".to_owned()])
    );
    let Err(why) = names(Who::Only(vec!["Lorwyn".to_owned()])) else {
        panic!("a name nobody has");
    };
    assert!(why.starts_with("All:"), "{why}");
}

#[test]
fn a_character_is_picked_by_its_name_or_its_start() {
    let characters = running(&["Ashryn", "Ashkar", "Baelor", "All"]);
    let picked = |name: &str| pick(name, &characters).map(|one| one.name.clone());
    assert_eq!(picked("baelor"), Ok("Baelor".to_owned()));
    assert_eq!(picked("Bae"), Ok("Baelor".to_owned()));
    assert_eq!(picked("all"), Ok("All".to_owned()));
    assert_eq!(picked("Ashk"), Ok("Ashkar".to_owned()));
    let Err(both) = picked("Ash") else {
        panic!("two fit");
    };
    assert!(both.contains("Ashryn or Ashkar"), "{both}");
    assert!(picked("Lorwyn").is_err());
    // A whole name is never ambiguous, however many names it starts.
    let nested = running(&["Ash", "Ashryn"]);
    assert_eq!(
        pick("ash", &nested).map(|one| one.name.clone()),
        Ok("Ash".to_owned())
    );
}

/// One name on two games is two characters: the name alone is refused,
/// saying how to name each, and `GAME:Name` picks one, by the start of its
/// name too (the crate review of 2026-09-28, R6: the name alone went to
/// whichever was found first).
#[test]
fn a_name_on_two_games_is_named_with_its_game() {
    let characters = running(&["ONE:Baelor", "TWO:Baelor", "ONE:Ashryn"]);
    let picked =
        |name: &str| pick(name, &characters).map(|one| format!("{}:{}", one.game, one.name));
    let Err(which) = picked("baelor") else {
        panic!("on two games");
    };
    assert!(
        which.contains("more than one game") && which.contains("ONE:Baelor or TWO:Baelor"),
        "{which}"
    );
    assert!(picked("Bae").is_err(), "nor by its start");
    assert_eq!(picked("two:baelor"), Ok("TWO:Baelor".to_owned()));
    assert_eq!(picked("ONE:Bae"), Ok("ONE:Baelor".to_owned()));
    assert_eq!(
        picked("Ash"),
        Ok("ONE:Ashryn".to_owned()),
        "one game, no need"
    );
    assert!(picked("TWO:Ashryn").is_err(), "not on that game");
}

/// What a scripted character was told, as text.
fn told(events: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
    let mut said = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::Notice(notice) = event {
            said.push(format!("{:?}", notice.body));
        }
    }
    said
}

/// Characters on scripted games: a line typed on Ashryn with `;to` goes to
/// Baelor's game and not Ashryn's; with `;all`, to every one. A Baelor on
/// another game is reached by its game, and the name alone reaches neither.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_relay_sends_on_the_character_it_names() {
    let prompt = b"<prompt time=\"1\">&gt;</prompt>\n";
    let (a_source, a_sent) = AnsweringSource::new(prompt);
    let (b_source, b_sent) = AnsweringSource::new(prompt);
    let (f_source, f_sent) = AnsweringSource::new(prompt);
    let ashryn = Session::numbered(SessionId(0), a_source);
    let baelor = Session::numbered(SessionId(1), b_source);
    let other = Session::numbered(SessionId(2), f_source);
    let (a, b, f) = (ashryn.handle(), baelor.handle(), other.handle());
    let (_, mut heard) = ashryn.subscribe();
    tokio::spawn(ashryn.into_actor().run());
    tokio::spawn(baelor.into_actor().run());
    let commands = Commands::install(&a);
    let on = |game: &str, name: &str, handle: &SessionHandle| Running {
        game: game.to_owned(),
        name: name.to_owned(),
        handle: handle.clone(),
    };
    let two = vec![on("ONE", "Ashryn", &a), on("ONE", "Baelor", &b)];
    let three = vec![
        on("ONE", "Ashryn", &a),
        on("ONE", "Baelor", &b),
        on("TWO", "Baelor", &f),
    ];
    let listed = Arc::new(std::sync::Mutex::new(two));
    let listing = Arc::clone(&listed);
    open(
        &a,
        &commands,
        Arc::new(move || {
            let listed = listing
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            Box::pin(async move { listed })
        }),
    );

    let typed = |line: &'static str| {
        let a = a.clone();
        async move {
            a.send_manual_at(a.generation(), line, DEADLINE).await;
            // The relay runs on by itself; let it send.
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };
    typed(".to baelor look").await;
    assert_eq!(b_sent.lines(), ["look"]);
    assert!(a_sent.lines().is_empty(), "not on the one it was typed on");

    typed(".all stand").await;
    assert_eq!(a_sent.lines(), ["stand"]);
    assert_eq!(b_sent.lines(), ["look", "stand"]);
    assert!(
        told(&mut heard)
            .iter()
            .any(|said| said.contains("on Ashryn, Baelor")),
        "all says who"
    );

    typed(".to Lorwyn look").await;
    assert!(
        told(&mut heard)
            .iter()
            .any(|said| said.contains("no character named Lorwyn")),
    );

    tokio::spawn(other.into_actor().run());
    *listed
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = three;
    typed(".to baelor sit").await;
    assert!(
        told(&mut heard)
            .iter()
            .any(|said| said.contains("more than one game")),
    );
    typed(".to TWO:Baelor kneel").await;
    assert_eq!(f_sent.lines(), ["kneel"]);
    assert_eq!(b_sent.lines(), ["look", "stand"], "not the other Baelor");
}
