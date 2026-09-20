//! Regression tests for defects found by attacking this crate.
//!
//! Each of these fixes had a written justification in the source and **nothing
//! asserting it**, which `plan/05` §0 calls decoration: a mutation reverting
//! the fix left the whole suite green. Every test here names the measurement
//! that made the fix worth making, and goes RED on that mutation.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;

#[test]
fn a_paired_dialog_data_with_clear_t_still_clears_the_dialog() {
    // `clear='t'` is read on the OPEN tag, and the open tag is PAIRED. Reading
    // only `ends_with("/>")` meant the form the wire actually sends produced
    // no frame at all, so a stale buff or cooldown list was never cleared.
    //
    // Measured over 40 corpus files: 26,745 `clear='t'` opens, of which
    // **zero** are self-closing. The self-closing form that used to work does
    // not occur at all.
    let mut parser = Parser::new();
    let frames = parser.parse_line("<dialogData id='Buffs' clear='t'></dialogData>");
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::ClearDialogData { id } if id == "Buffs")),
        "the paired clear='t' form is the only one the wire sends: {frames:#?}"
    );

    // The self-closing form keeps working.
    let selfclose = parser.parse_line("<dialogData id='Cooldowns'/>");
    assert!(
        selfclose
            .iter()
            .any(|f| matches!(f, Frame::ClearDialogData { id } if id == "Cooldowns")),
        "a self-closing dialogData still clears: {selfclose:#?}"
    );

    // A paired dialogData WITHOUT clear must not clear -- it opens the dialog
    // context that gives vitals their enclosing id.
    let opened = parser.parse_line(
        "<dialogData id='minivitals'><progressBar id='health' value='95' text='health 213/223'/>",
    );
    assert!(
        !opened
            .iter()
            .any(|f| matches!(f, Frame::ClearDialogData { .. })),
        "a paired dialogData with no clear= must not clear: {opened:#?}"
    );
    assert!(
        opened.iter().any(|f| matches!(
            f,
            Frame::ProgressBar(b) if b.dialog.as_deref() == Some("minivitals")
        )),
        "and it must still supply the enclosing dialog id: {opened:#?}"
    );
}

#[test]
fn a_cooldown_bar_carries_its_countdown() {
    // `time='00:00:37'` was dropped entirely: `ProgressBar` had no field for
    // it, so a Heal or Hunt behavior could not tell when a cooldown expired.
    // 154,313 progress bars carry `time=` in a 40-file sample.
    let mut parser = Parser::new();
    let frames = parser.parse_line(
        "<dialogData id='Cooldowns'><progressBar id='110572' value='100' \
         text=\"Multi-Strike\" time='00:00:37'/>",
    );
    let bar = frames
        .iter()
        .find_map(|f| match f {
            Frame::ProgressBar(b) => Some(b),
            _ => None,
        })
        .expect("the cooldown bar parses");
    assert_eq!(
        bar.time_remaining_secs,
        Some(37),
        "the countdown must reach the caller as a duration: {bar:#?}"
    );

    // A vitals bar has no countdown and must not invent one.
    let vitals = parser.parse_line("<progressBar id='health' value='95' text='health 213/223'/>");
    let bar = vitals
        .iter()
        .find_map(|f| match f {
            Frame::ProgressBar(b) => Some(b),
            _ => None,
        })
        .expect("the vitals bar parses");
    assert_eq!(bar.time_remaining_secs, None);
}

#[test]
fn a_pulse_reports_the_mana_flag_the_wire_sent() {
    // `is_some()` on the attribute made every pulse read `true`, because the
    // wire always sends the attribute. Measured over 272 corpus files the
    // split is almost exactly even -- 710 `mana="1"` against 709 `mana="0"` --
    // so half of all pulses were reported wrongly and the field carried no
    // information at all.
    let mut parser = Parser::new();
    for (tag, expected) in [
        ("<pulse mana=\"1\"/>", true),
        ("<pulse mana=\"0\"/>", false),
    ] {
        let frames = parser.parse_line(tag);
        assert!(
            frames
                .iter()
                .any(|f| matches!(f, Frame::Pulse { mana, .. } if *mana == expected)),
            "{tag} must report mana={expected}: {frames:#?}"
        );
    }

    // The wire sends `<pulse mana="1"/>` bare -- 1,419 of 1,419 occurrences in
    // that sample -- so the interval defaults are load bearing.
    let frames = parser.parse_line("<pulse mana=\"1\"/>");
    assert!(
        frames.iter().any(|f| matches!(
            f,
            Frame::Pulse {
                min: 46,
                max: 75,
                ..
            }
        )),
        "a bare pulse implies the 46/75 interval: {frames:#?}"
    );
}

#[test]
fn a_named_pop_stream_closes_the_stream_it_names() {
    // Popping the innermost entry regardless routed every later line to the
    // wrong window. Nesting is real -- 151 lines in a 272-file sample carry
    // two pushes -- and pushStream is not stack-disciplined (19,142 pushes to
    // 14,714 pops), so a mismatch is the expected case.
    let mut parser = Parser::new();
    let _ = parser.parse_line("<pushStream id='a'/>");
    let _ = parser.parse_line("<pushStream id='b'/>");
    let popped = parser.parse_line("<popStream id='a'/>");
    assert!(
        popped
            .iter()
            .any(|f| matches!(f, Frame::StreamResume { id } if id == "b")),
        "popping `a` must leave `b` current, not resume `a`: {popped:#?}"
    );
    let text = parser.parse_line("text");
    assert!(
        text.iter()
            .any(|f| matches!(f, Frame::Text(t) if t.stream == "b")),
        "and text must route to `b`: {text:#?}"
    );

    // A bare `<popStream/>` still means "the current one" -- 625 of the pops
    // in that sample.
    let bare = parser.parse_line("<popStream/>");
    assert!(
        bare.iter()
            .any(|f| matches!(f, Frame::StreamPop { id } if id.is_none())),
        "a bare pop names no stream: {bare:#?}"
    );
}

#[test]
fn prose_containing_an_angle_bracket_is_not_eaten_as_markup() {
    // Any `<` used to start a tag that ran to the next `>`, so this line
    // reached a consumer as Text("5 ") + UnknownTag + Text(" 5") and rendered
    // to the player as "5   5". Absent from live traffic -- latent rather than
    // live -- but Rule 2.2 is precisely about input the corpus has not seen.
    let mut parser = Parser::new();
    let frames = parser.parse_line("5 < 10 and 10 > 5");
    let text: String = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some(t.content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "5 < 10 and 10 > 5", "prose must survive intact");
    assert!(
        !frames.iter().any(|f| matches!(f, Frame::UnknownTag { .. })),
        "and none of it is markup: {frames:#?}"
    );

    // Real markup on the same line still parses.
    let mixed = parser.parse_line("5 < 10 <nav rm='7503251'/>done");
    assert!(
        mixed
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id.as_deref() == Some("7503251"))),
        "a real tag beside prose angle brackets must still parse: {mixed:#?}"
    );

    // A genuinely unknown tag is still news: the guard must not swallow it.
    let unknown = parser.parse_line("<someNewTag id='1'/>");
    assert!(
        unknown
            .iter()
            .any(|f| matches!(f, Frame::UnknownTag { name, .. } if name == "someNewTag")),
        "the name-start guard must not weaken Rule 2.2: {unknown:#?}"
    );
}

#[test]
fn a_malformed_attribute_does_not_drop_the_attributes_after_it() {
    // `attributes` returned early on anything it could not parse, so
    // `bleeding` and `dead` vanished with no trace. For a Heal or Hunt
    // behavior reading creature status that is a wrong decision, not a
    // cosmetic one -- the same severity class as the negative-health bug.
    let mut parser = Parser::new();
    let frames =
        parser.parse_line("<crtrStatus exist='1' stunned='1' bad=unquoted bleeding='1' dead='1'/>");
    let attrs = frames
        .iter()
        .find_map(|f| match f {
            Frame::CreatureStatus { attrs, .. } => Some(attrs),
            _ => None,
        })
        .expect("crtrStatus parses");
    let keys: Vec<&str> = attrs.iter().map(|(k, _)| k.as_str()).collect();
    assert!(
        keys.contains(&"bleeding") && keys.contains(&"dead"),
        "attributes after a malformed pair must survive: {keys:?}"
    );

    // A valueless attribute is dropped, not merged into the next key.
    // `("bonfire inside", "1")` is a name no consumer can match, and it took
    // `inside` down with it. Now that `<roommeta>` is typed, the symptom is
    // sharper: the compound key means `inside` parses as `None`.
    let meta = parser.parse_line("<roommeta weather='0' bonfire inside='1' sanctuary='1'/>");
    let room_meta = meta
        .iter()
        .find_map(|f| match f {
            Frame::RoomMeta(meta) => Some(*meta),
            _ => None,
        })
        .expect("roommeta parses");
    assert_eq!(
        (room_meta.weather, room_meta.inside, room_meta.sanctuary),
        (Some(0), Some(1), Some(1)),
        "a valueless attribute must not fabricate a compound key and take          the attribute after it down too"
    );
    assert_eq!(
        room_meta.bonfire, None,
        "the valueless attribute itself said nothing, and None is that"
    );
}

#[test]
fn hand_and_world_event_text_arrives_decoded_and_without_markup() {
    // Rule 2.1 in the thin tier: five fields handed back the raw inner slice,
    // so `<worldEvent>` reached the caller with `<b>` and `&amp;` intact --
    // the same violation this crate fixes for `Component` with `Runs`.
    let mut parser = Parser::new();
    let frames = parser.parse_line(
        "<worldEvent realm='Elanthia' expires='30'>A <b>great</b> storm &amp; flood!</worldEvent>",
    );
    let text = frames
        .iter()
        .find_map(|f| match f {
            Frame::WorldEvent { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .expect("worldEvent parses");
    assert_eq!(
        text, "A great storm & flood!",
        "nested markup is flattened and entities decoded (Rule 2.1)"
    );

    let hand = parser.parse_line("<left exist=\"2\" noun=\"pack\">a knight&apos;s pack</left>");
    let item = hand
        .iter()
        .find_map(|f| match f {
            Frame::LeftHand { item, .. } => Some(item.as_str()),
            _ => None,
        })
        .expect("left hand parses");
    assert_eq!(item, "a knight's pack");
}

#[test]
fn an_inventory_manager_token_comes_from_the_id_attribute() {
    // It read an attribute called `token`, which the wire does not send, so
    // the field was always empty and no response could be matched to its
    // request. VERIFIED against the reference at
    // `src/parser/handlers.rs:910`: `extract_attribute(tag, "id")`.
    let mut parser = Parser::new();
    let frames = parser.parse_line("<inventoryManager id='tok99' room='1'/>");
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::InventoryManager(s) if s.token == "tok99")),
        "the correlation token lives in id=: {frames:#?}"
    );
}

#[test]
fn a_reward_keeps_its_type_and_amount_instead_of_becoming_an_empty_husk() {
    // `reward` shared an arm with `celebration` that built an
    // `ActiveEffect { category, id, text, time }`. The wire sends
    // `<reward type='fame' amount='20000'/>`: self-closing, no `id`, no
    // `time`, no body text. So every field that arm read was absent and every
    // reward became
    // `ActiveEffect { category: "reward", id: "", text: "", time: None }`,
    // with `type` and `amount` -- the entire payload -- dropped. A handler
    // that looked like it was handling something was destroying it, which is
    // worse than not handling it at all: `Frame::UnknownTag` would at least
    // have carried the raw bytes.
    //
    // VERIFIED over a 1,547-file corpus sample (stride-7 of 10,824 files):
    // 624 `<reward>`, all 624 of the shape `<reward type='X' amount='X'/>`,
    // 0 with `id`, 0 with `time`. `type` is `fame` (312) or `experience`
    // (312).
    //
    // Goes RED on restoring `reward` to the ActiveEffect arm.
    let mut parser = Parser::new();
    let frames = parser.parse_line("<reward type='fame' amount='20000'/>");
    let attrs = frames
        .iter()
        .find_map(|f| match f {
            Frame::WindowHints { attrs, .. } => Some(attrs),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a reward must carry its attributes: {frames:#?}"));
    let value = |key: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(
        value("type"),
        Some("fame"),
        "the reward kind is the payload"
    );
    assert_eq!(
        value("amount"),
        Some("20000"),
        "the reward size is the payload"
    );
}

#[test]
fn a_stream_window_keeps_the_attributes_beyond_id_title_and_subtitle() {
    // The handler lifted exactly `id`, `title` and `subtitle` and discarded
    // the rest of the tag. A census of a 1,547-file corpus sample (stride-7
    // of 10,824) found 1,176,686 `<streamWindow>` carrying 15 distinct
    // attribute names, so 12 were being dropped -- including `location`
    // (1,169,402 occurrences) and `target` (1,163,813), which ride ~99% of
    // them, plus `resident` (605,219), `ifClosed` (602,513) and
    // `styleIfClosed` (1,353).
    //
    // `styleIfClosed` is documented by the protocol wiki, so this was a gap
    // against a source Cena already had rather than a Saga discovery.
    //
    // Goes RED on removing the `attrs` field from the handler.
    let mut parser = Parser::new();
    let frames = parser.parse_line(
        "<streamWindow id='percWindow' title='Spells' subtitle=' - [Active]' \
         location='center' target='drop' ifClosed='' resident='true' \
         styleIfClosed='watching'/>",
    );
    let attrs = frames
        .iter()
        .find_map(|f| match f {
            Frame::StreamWindow { attrs, .. } => Some(attrs),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a streamWindow must parse: {frames:#?}"));
    let value = |key: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(value("location"), Some("center"), "on ~99% of the tags");
    assert_eq!(value("target"), Some("drop"), "on ~99% of the tags");
    assert_eq!(value("resident"), Some("true"));
    assert_eq!(
        value("styleIfClosed"),
        Some("watching"),
        "wiki-documented, and live on the wire"
    );
}
