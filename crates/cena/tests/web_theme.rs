//! `plan/57` step 7: a page is sent the theme Hydra wears first, the hub's
//! page and a character's alike, read from the data folder as the page
//! opens; a server given no data folder sends none.

mod web_support;

use cena_platform::AnsweringSource;
use cena_session::{Generation, Session, SessionId};
use cena_ui::ServerMessage;
use cena_web::WebServer;
use tokio_util::sync::CancellationToken;
use web_support::*;

#[tokio::test]
async fn every_page_is_sent_the_theme_first_from_the_data_folder() {
    let data = std::env::temp_dir().join(format!("cena-web-theme-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(data.join("themes")).unwrap();
    std::fs::write(data.join("window.toml"), "theme = \"Paper\"\n").unwrap();
    std::fs::write(
        data.join("themes").join("paper.toml"),
        "name = \"Paper\"\nbase = \"Light\"\n[pins]\nhealth = \"#112233\"\n",
    )
    .unwrap();

    let (source, _transcript) = AnsweringSource::logged_in(ROOM);
    let session = Session::numbered(SessionId(0), source);
    let (handle, observer, stop) = (session.handle(), session.observer(), session.cancel_token());
    let actor = tokio::spawn(session.into_actor().run());
    await_ready(&observer, Generation::FIRST).await.unwrap();

    let server = WebServer::open()
        .await
        .unwrap()
        .with_data(data.clone())
        .unwrap();
    server.sessions().attach("Nisugi", observer, handle);
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));

    // The theme comes before the page's first message.
    let mut page = browser(&pairing).await.unwrap();
    let ServerMessage::Theme { name, colors, .. } = receive(&mut page).await.unwrap() else {
        panic!("the theme comes first");
    };
    assert_eq!(name, "Paper");
    assert_eq!(colors.get("health").map(String::as_str), Some("#112233"));
    let canvas = colors.get("canvas").expect("every token");
    let light = cena_ui::theme::Themes::built_in()
        .outfit("Light")
        .unwrap()
        .palette
        .get(cena_ui::theme::Token::Canvas);
    assert_eq!(
        canvas,
        &cena_ui::theme::hex(light),
        "Paper is Light but for its pin"
    );
    // Then the page itself: the hub's list, since the page named no session.
    assert!(matches!(
        receive(&mut page).await.unwrap(),
        ServerMessage::Sessions { .. }
    ));

    // Changed on disk, a page opened after sees it.
    std::fs::write(data.join("window.toml"), "theme = \"Despana\"\n").unwrap();
    let mut again = browser(&pairing).await.unwrap();
    let ServerMessage::Theme { name, .. } = receive(&mut again).await.unwrap() else {
        panic!("the theme comes first");
    };
    assert_eq!(name, "Despana");

    stop_web.cancel();
    let _ = web.await;
    stop.cancel();
    let _ = actor.await;
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test]
async fn a_server_with_no_data_folder_sends_no_theme() {
    let (source, _transcript) = AnsweringSource::logged_in(ROOM);
    let session = Session::numbered(SessionId(0), source);
    let (handle, observer, stop) = (session.handle(), session.observer(), session.cancel_token());
    let actor = tokio::spawn(session.into_actor().run());
    await_ready(&observer, Generation::FIRST).await.unwrap();
    let server = WebServer::open().await.unwrap();
    server.sessions().attach("Nisugi", observer, handle);
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut page = browser(&pairing).await.unwrap();
    // Then the page itself: the hub's list, since the page named no session.
    assert!(matches!(
        receive(&mut page).await.unwrap(),
        ServerMessage::Sessions { .. }
    ));
    stop_web.cancel();
    let _ = web.await;
    stop.cancel();
    let _ = actor.await;
}
