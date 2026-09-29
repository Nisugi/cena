//! Which character an agent's name reaches: a name on two games is neither
//! until the game is named with it (the integrated crate review of
//! 2026-09-28, I3).

use cena_agent::Characters;
use cena_platform::AnsweringSource;
use cena_session::agent::Level;
use cena_session::{Session, SessionHandle, SessionId};

/// A scripted character's session, running.
fn running(characters: &Characters, id: u32, (game, name): (&str, &str)) -> SessionHandle {
    let (source, _transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    let session = Session::new(source);
    let (handle, observer) = (session.handle(), session.observer());
    tokio::spawn(session.into_actor().run());
    characters.seat(
        SessionId(id),
        (game, name),
        observer,
        handle.agent_door(),
        None,
        None,
    );
    handle
}

/// Two characters called Shared on two games, and one called Solo: the bare
/// name `Shared` is refused with both ways to name them, each `GAME:Name`
/// reaches its own (told apart by the level each was given), a name on one
/// game is found bare, and a name nobody has finds nothing.
#[tokio::test]
async fn a_name_on_two_games_is_neither_until_the_game_is_named() {
    let characters = Characters::default();
    let prime = running(&characters, 1, ("GS3", "Shared"));
    let _shattered = running(&characters, 2, ("GSF", "Shared"));
    let _solo = running(&characters, 3, ("GS3", "Solo"));
    prime.set_agent_level(Level::Observe);

    let mut could = characters.named("shared").unwrap_err();
    could.sort();
    assert_eq!(could, ["GS3:Shared", "GSF:Shared"]);

    let on_prime = characters.named("gs3:Shared").unwrap();
    assert_eq!(
        (on_prime.game.as_str(), on_prime.door.level()),
        ("GS3", Level::Observe)
    );
    let on_shattered = characters.named(" GSF : shared ").unwrap();
    assert_eq!(
        (on_shattered.game.as_str(), on_shattered.door.level()),
        ("GSF", Level::Off)
    );

    let solo = characters.named("Solo").unwrap();
    assert_eq!(
        solo.label(&characters.all()),
        "Solo",
        "one game: its name alone"
    );
    assert_eq!(
        on_prime.label(&characters.all()),
        "GS3:Shared",
        "two: with its game"
    );
    assert!(
        characters.named("GSF:Solo").unwrap_err().is_empty(),
        "not on that game"
    );
    assert!(characters.named("Nobody").unwrap_err().is_empty());
}
