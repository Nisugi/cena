/// **`</dynaStream>` closes its routing context**, which it did not.
///
/// Review finding 5. The open arm takes `"stream" | "dynaStream"` together and
/// calls `open_stream(.., true)` for both, so both push a paired entry -- but
/// the closer matched only `"stream"`. Reproduced: `<dynaStream id='bugStream'>
/// Details</dynaStream>Back in main` left "Back in main" tagged `bugStream`,
/// and every line after it for the life of the session.
///
/// The asymmetry is the tell: a tag that opens a routing context in one arm
/// and is absent from its closer leaks by construction.
#[test]
fn a_dynastream_close_restores_the_enclosing_route() {
    let mut parser = cena_protocol::Parser::new();
    let mut routed: Vec<(String, String)> = Vec::new();
    for frame in parser.push_bytes(b"<dynaStream id='bugStream'>Details</dynaStream>Back in main\n")
    {
        if let cena_protocol::frame::Frame::Text(text) = &frame {
            routed.push((text.content.clone(), text.stream.clone()));
        }
    }

    assert_eq!(
        routed,
        vec![
            ("Details".to_owned(), "bugStream".to_owned()),
            ("Back in main".to_owned(), String::new()),
        ]
    );
}

/// A `dynaStream` close with nothing open does not unroute an enclosing push.
///
/// The same guard `</stream>` has: a close that popped nothing is not a pop,
/// so a stray one must not take a `pushStream` context with it.
#[test]
fn a_stray_dynastream_close_pops_nothing() {
    let mut parser = cena_protocol::Parser::new();
    let mut last = String::new();
    for frame in parser.push_bytes(b"<pushStream id='thoughts'/></dynaStream>still thoughts\n") {
        if let cena_protocol::frame::Frame::Text(text) = &frame
            && !text.content.trim().is_empty()
        {
            last = text.stream.clone();
        }
    }
    assert_eq!(last, "thoughts", "a stray close must not unroute the push");
}
