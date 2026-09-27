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

fn running(names: &[&str]) -> Vec<(String, SessionHandle)> {
    names
        .iter()
        .map(|name| ((*name).to_owned(), handle()))
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
    assert_eq!(parse("ALL stand"), Some(Ok(Relay::All("stand".to_owned()))));
    assert!(matches!(parse("to Baelor"), Some(Err(_))), "no command");
    assert!(matches!(parse("all"), Some(Err(_))), "no command");
    assert_eq!(parse("toast"), None, "another word");
    assert_eq!(parse("go2 bank"), None);
}

/// A whole name wins, whatever the case; otherwise the start of one name.
/// A character named All is reached by `;to`, never by `;all`.
#[test]
fn a_character_is_picked_by_its_name_or_its_start() {
    let characters = running(&["Ashryn", "Ashkar", "Baelor", "All"]);
    let picked = |name: &str| pick(name, &characters).map(|(name, _)| name.clone());
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
        pick("ash", &nested).map(|(name, _)| name.clone()),
        Ok("Ash".to_owned())
    );
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

/// Two characters on scripted games: a line typed on Ashryn with `;to`
/// goes to Baelor's game and not Ashryn's; with `;all`, to both.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_relay_sends_on_the_character_it_names() {
    let prompt = b"<prompt time=\"1\">&gt;</prompt>\n";
    let (a_source, a_sent) = AnsweringSource::new(prompt);
    let (b_source, b_sent) = AnsweringSource::new(prompt);
    let ashryn = Session::numbered(SessionId(0), a_source);
    let baelor = Session::numbered(SessionId(1), b_source);
    let (a, b) = (ashryn.handle(), baelor.handle());
    let (_, mut heard) = ashryn.subscribe();
    tokio::spawn(ashryn.into_actor().run());
    tokio::spawn(baelor.into_actor().run());
    let commands = Commands::install(&a);
    let listed = vec![("Ashryn".to_owned(), a.clone()), ("Baelor".to_owned(), b)];
    open(
        &a,
        &commands,
        Arc::new(move || {
            let listed = listed.clone();
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
    typed(";to baelor look").await;
    assert_eq!(b_sent.lines(), ["look"]);
    assert!(a_sent.lines().is_empty(), "not on the one it was typed on");

    typed(";all stand").await;
    assert_eq!(a_sent.lines(), ["stand"]);
    assert_eq!(b_sent.lines(), ["look", "stand"]);
    assert!(
        told(&mut heard)
            .iter()
            .any(|said| said.contains("on Ashryn, Baelor")),
        "all says who"
    );

    typed(";to Lorwyn look").await;
    assert!(
        told(&mut heard)
            .iter()
            .any(|said| said.contains("no character named Lorwyn")),
    );
}
