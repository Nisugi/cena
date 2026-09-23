//! Regression tests for the review of 2026-09-23.
//!
//! Each test names the defect it pins and was confirmed RED against the code
//! with only that fix reverted -- a test that stays green on the mutation is
//! decoration (`plan/05` §0), and this crate has been bitten by exactly that
//! more than once.
//!
//! `cena-protocol`'s tests ban `panic!`, `expect()` and `unwrap()` outside a
//! `#[test]` body, which is why the helpers below filter rather than unwrap.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// Every frame a sequence of lines produces, through one parser.
fn frames(lines: &[&str]) -> Vec<Frame> {
    let mut parser = Parser::new();
    lines.iter().flat_map(|l| parser.parse_line(l)).collect()
}

/// Does any string in the frames carry a C0/C1 control other than `\n`/`\t`?
fn has_escape(s: &str) -> bool {
    s.chars()
        .any(|c| (c.is_control() || c == '\u{7f}') && c != '\n' && c != '\t')
}

// --- 1. attribute values are control-stripped -------------------------------

#[test]
fn an_attribute_value_cannot_smuggle_a_terminal_escape() {
    // `&#27;` and `&#7;` decode to ESC and BEL. Decoded without stripping,
    // this is an OSC-52 clipboard write riding in `Text.stream`, and a
    // clear-screen riding in `Component.id` -- both strings a frontend prints.
    let out = frames(&[
        "<pushStream id='a&#27;]52;c;Zm9v&#7;'/>hello",
        "<component id='room&#27;[2J objs'>a rock</component>",
    ]);
    let streams: Vec<&str> = out
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some(t.stream.as_str()),
            Frame::StreamPush { id } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert!(!streams.is_empty(), "the push must still route: {out:#?}");
    assert!(
        streams.iter().all(|s| !has_escape(s)),
        "a stream id carried a control character: {streams:?}"
    );
    assert!(
        streams.contains(&"a]52;c;Zm9v"),
        "the id is kept, minus its controls: {streams:?}"
    );
    let ids: Vec<&str> = out
        .iter()
        .filter_map(|f| match f {
            Frame::Component { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["room[2J objs"]);
}

#[test]
fn an_attribute_bag_is_stripped_too_but_keeps_its_newlines() {
    // `attributes()` is the other reader, and the bag reaches consumers whole
    // (`crtrStatus`, `streamWindow`, dialog widgets).
    let out = frames(&["<streamWindow id='x' title='T&#27;[31m' location='a&#10;b'/>"]);
    let attrs: Vec<(String, String)> = out
        .iter()
        .filter_map(|f| match f {
            Frame::StreamWindow { attrs, .. } => Some(attrs.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(
        attrs.iter().all(|(_, v)| !has_escape(v)),
        "a bag value carried a control character: {attrs:?}"
    );
    // A newline is the one control an attribute legitimately carries:
    // `<objective description=>` sends `&#10;`.
    assert!(
        attrs.contains(&("location".to_owned(), "a\nb".to_owned())),
        "`&#10;` must survive as a newline: {attrs:?}"
    );
}

// --- 5. a component body saves and restores ALL markup state ----------------

#[test]
fn mono_opened_inside_a_component_does_not_leak_past_it() {
    // The review's exact reproduction: "after" came out `mono: true`, and so
    // did every line after it until something reset the flag.
    let out = frames(&[r#"<component id='x'><output class="mono"/>a</component>after"#]);
    let after: Vec<bool> = out
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) if t.content == "after" => Some(t.style.mono),
            _ => None,
        })
        .collect();
    assert_eq!(after, [false], "{out:#?}");
}

// --- 6. a progress amount's numbers abut the slash --------------------------

#[test]
fn a_stray_number_is_not_paired_with_a_denominator() {
    let out = frames(&["<progressBar id='stamina' value='50' text='stamina 100 foo/200'/>"]);
    let amounts: Vec<_> = out
        .iter()
        .filter_map(|f| match f {
            Frame::ProgressBar(b) => Some(b.amount),
            _ => None,
        })
        .collect();
    assert_eq!(amounts, [None], "{out:#?}");
}

// --- 4. an envelope's attributes come from its open tag ---------------------

#[test]
fn a_child_cannot_supply_the_envelopes_attributes() {
    // The envelope sends no `root=`; its `<continuation>` child does. Read
    // over the whole paired string, the child's value was reported as the
    // envelope's -- a fabricated cursor on a snapshot that had none.
    let out = frames(&[
        "<inventoryManager id='im1' room='7'><continuation root='777' last='5'/></inventoryManager>",
    ]);
    let snaps: Vec<_> = out
        .iter()
        .filter_map(|f| match f {
            Frame::InventoryManager(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(snaps.len(), 1, "{out:#?}");
    for snap in snaps {
        assert_eq!(snap.root, None, "root was read out of the child");
        assert_eq!(
            snap.continuations.first().map(|c| c.root.as_str()),
            Some("777"),
            "the child keeps its own root"
        );
    }
}

// --- 8. a self-closing <stream/> declares no window -------------------------

#[test]
fn a_self_closing_stream_is_not_a_window_declaration() {
    // PR-4 made `<dynaStream/>` structural and left `<stream/>` falling
    // through to `thin.rs`, which declared a window nobody sent.
    let out = frames(&["<stream id='Spells'/>"]);
    assert!(
        matches!(out.as_slice(), [Frame::Structural { name, .. }] if name == "stream"),
        "expected one Structural, got {out:#?}"
    );
    // And it routes nothing: a following line is main-window text.
    let mut parser = Parser::new();
    let _ = parser.parse_line("<stream id='Spells'/>");
    let next = parser.parse_line("plain");
    assert!(
        next.iter()
            .any(|f| matches!(f, Frame::Text(t) if t.stream.is_empty())),
        "{next:#?}"
    );
}

// --- 10. child rows are matched by exact name -------------------------------

#[test]
fn a_child_row_is_matched_by_its_exact_name() {
    // The four hand-rolled scanners had drifted: the continuation scanner
    // tested `starts_with("continuation")` with no delimiter, so a
    // `<continuationCursor/>` was read as a continuation row.
    let out = frames(&[
        "<inventoryManager id='im1' room='7'><continuationCursor root='1' last='2'/></inventoryManager>",
    ]);
    let counts: Vec<usize> = out
        .iter()
        .filter_map(|f| match f {
            Frame::InventoryManager(s) => Some(s.continuations.len()),
            _ => None,
        })
        .collect();
    assert_eq!(counts, [0], "{out:#?}");
}

// --- 2. non-styling tags inside an <inventoryViewItem> capture still act ----

#[test]
fn a_stream_push_inside_a_capture_reaches_the_stream_stack() {
    // The capture flattened every tag it did not name, so the push vanished:
    // no frame, and no entry on the stack.
    let out = frames(&[
        "<inventoryViewItem id='iv1' exist='9'><result command='look'>A rock.",
        "<pushStream id='inv'/><progressBar id='health' value='95' text='health 213/223'/>",
        "</result></inventoryViewItem>after",
    ]);
    assert!(
        out.iter()
            .any(|f| matches!(f, Frame::StreamPush { id } if id == "inv")),
        "the push produced no frame: {out:#?}"
    );
    assert!(
        out.iter()
            .any(|f| matches!(f, Frame::ProgressBar(b) if b.id == "health")),
        "the progress bar produced no frame: {out:#?}"
    );
    // The push is on the parser's stack, so text after the capture routes.
    assert!(
        out.iter()
            .any(|f| matches!(f, Frame::Text(t) if t.content == "after" && t.stream == "inv")),
        "the stream stack never saw the push: {out:#?}"
    );
    // And the capture's own text is unchanged: the tags are not prose.
    let texts: Vec<String> = out
        .iter()
        .filter_map(|f| match f {
            Frame::InventoryViewItem(v) => Some(v.results.iter().map(|r| r.text.clone()).collect()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["A rock."], "{out:#?}");
}

#[test]
fn a_paired_tag_inside_a_capture_is_dispatched_whole() {
    let out = frames(&[
        "<inventoryViewItem id='iv1' exist='9'><result command='look'>A rock.",
        "<right exist='5' noun='sword'>sword</right></result></inventoryViewItem>",
    ]);
    assert!(
        out.iter()
            .any(|f| matches!(f, Frame::RightHand { item, .. } if item == "sword")),
        "{out:#?}"
    );
}

// --- 7. a bare <d> inside a capture keeps its link --------------------------

#[test]
fn a_bare_d_inside_a_capture_is_a_direct_text_link() {
    use cena_protocol::frame::LinkKind;
    let out = frames(&[
        "<inventoryViewItem id='iv1' exist='9'><result command='look'>Try <d>look closer</d>.",
        "</result></inventoryViewItem>",
    ]);
    let links: Vec<_> = out
        .iter()
        .filter_map(|f| match f {
            Frame::InventoryViewItem(v) => Some(v.results.iter().flat_map(|r| r.links.clone())),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(
        matches!(links.as_slice(), [l] if l.kind == LinkKind::DirectText && l.text == "look closer"),
        "{links:#?}"
    );
}

// --- 3. layout attributes survive on ProgressBar, Label and InjuryImage -----

/// The value of `key` in a bag, if present.
fn get<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

#[test]
fn a_progress_bar_keeps_its_layout() {
    // Verbatim from `tests/fixtures/vitals_secondary.xml`.
    let out = frames(&[
        "<progressBar id='pbarStance' value='80' text='guarded (80%)' top='51' width='130' height='16' left='0' align='n' tooltip='Percent of stance contributing to defense'/>",
    ]);
    let bags: Vec<_> = out
        .iter()
        .filter_map(|f| match f {
            Frame::ProgressBar(b) => Some(b.attrs.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(bags.len(), 1, "{out:#?}");
    for bag in &bags {
        for (key, want) in [("top", "51"), ("width", "130"), ("align", "n")] {
            assert_eq!(get(bag, key), Some(want), "{key} dropped: {bag:?}");
        }
        assert!(get(bag, "tooltip").is_some(), "tooltip dropped: {bag:?}");
    }
}

#[test]
fn a_label_and_an_image_keep_theirs() {
    let out = frames(&[
        "<label id='yourLvl' value='Level 100' top='0' left='0' align='n' width='160' height='15'/>",
        // A toolbar button from `inventory_container.xml`: `cmd` and `echo`
        // are its behaviour, not decoration.
        "<image id='unsheathe' name='SwordBtn' cmd='_ready weapon' tooltip='Unsheathe Weapon' echo='ready weapon' align='n' top='3' left='-50' height='29' width='29'/>",
    ]);
    let mut seen = 0;
    for f in &out {
        match f {
            Frame::Label { attrs, .. } => {
                seen += 1;
                assert_eq!(get(attrs, "width"), Some("160"), "{attrs:?}");
            }
            Frame::InjuryImage { attrs, .. } => {
                seen += 1;
                assert_eq!(get(attrs, "cmd"), Some("_ready weapon"), "{attrs:?}");
                assert_eq!(get(attrs, "left"), Some("-50"), "{attrs:?}");
            }
            _ => {}
        }
    }
    assert_eq!(seen, 2, "{out:#?}");
}

// --- 9. push_bytes and parse_line agree about the settings blob -------------

/// The frames `push_bytes` yields for `lines`, newline-terminated.
fn by_bytes(lines: &[&str]) -> Vec<Frame> {
    let wire: String = lines.iter().flat_map(|l| [*l, "\n"]).collect();
    Parser::new().push_bytes(wire.as_bytes())
}

#[test]
fn both_entry_points_agree_about_how_a_blob_ends() {
    // Three divergences the agreement property found, one shape each:
    let cases: &[&[&str]] = &[
        // a close that ends its line was a BLANK LINE on `push_bytes`;
        &["<settings>", "<ignores/></settings>"],
        // blob bytes before a prompt were re-parsed as game text by
        // `parse_line` -- `<h>` reaching `UnknownTag`;
        &[
            "<settings>",
            "<h id='2'/>junk<prompt time='1'>&gt;</prompt>",
        ],
        // and a prompt BEFORE the close lost to it on `parse_line`.
        &[
            "<settings>",
            "<prompt time='1'>&gt;</prompt><h id='3'/></settings>",
        ],
    ];
    for lines in cases {
        let mut parser = Parser::new();
        let whole: Vec<Frame> = lines.iter().flat_map(|l| parser.parse_line(l)).collect();
        assert_eq!(by_bytes(lines), whole, "{lines:?}");
        assert!(
            !whole
                .iter()
                .any(|f| matches!(f, Frame::Text(t) if t.content.contains("junk"))),
            "blob bytes reached the text stream: {whole:#?}"
        );
    }
}
