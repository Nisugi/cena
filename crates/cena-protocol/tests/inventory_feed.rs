//! `<inventoryManager>` and `<inventoryViewItem>` reach the model typed.
//!
//! Both were untyped attribute bags carrying only their envelope. The
//! snapshot's 149 item rows fell out separately as `Frame::Structural`,
//! because `<i>` sat in the parser's STYLING arm on the assumption that it is
//! an italic tag -- it is not, and `GemStone` has no italic tag. The detail
//! response's `<result>` sections landed in `Frame::WindowHints`, the window
//! placement bag.

use cena_protocol::{Frame, InventoryResponse, Parser};

/// A real snapshot, trimmed from the author's September logs. The shapes that
/// matter are all here: a worn item, a container with `in_max`, an item
/// nested inside that container, a `long` description with `$_` markers, and
/// a closed flag.
const SNAPSHOT: &str = concat!(
    r"<inventoryManager id='im9445b1acb' room='7503206'>",
    r#"<i id='309585698' loc='worn,player' name="a scorched,glowbark long,bow" weight='3'/>"#,
    r#"<i id='309585706' loc='worn,player' name="a nacreous,plumille,cloak" weight='4' in_max='2000'/>"#,
    r#"<i id='309585758' loc='in,309585706' name="a sapphire-set,platinum,crown" weight='2'/>"#,
    r#"<i id='309585703' loc='worn,player' name="an elesine,treasure,sack" "#,
    r#"long="an $_elesine treasure sack$_ with a blood crystal clasp" weight='2'/>"#,
    r#"<i id='309585818' loc='in,309585706' name="a,coal black,purse" weight='3' flags='closed' in_max='50'/>"#,
    r"</inventoryManager>",
);

fn frames(lines: &[&str]) -> Vec<Frame> {
    let mut parser = Parser::new();
    let mut out = Vec::new();
    for line in lines {
        out.extend(parser.parse_line(line));
    }
    out
}

/// Every snapshot frame the lines produced, so a test asserts on the count
/// rather than unwrapping. `cena-protocol`'s tests ban `panic!` and
/// `expect()`, which is why nothing here reaches for either.
fn snapshots(lines: &[&str]) -> Vec<InventoryResponse> {
    frames(lines)
        .into_iter()
        .filter_map(|f| match f {
            Frame::InventoryManager(snap) => Some(snap),
            _ => None,
        })
        .collect()
}

/// The LAST snapshot, or an empty one when none arrived.
fn snapshot(lines: &[&str]) -> InventoryResponse {
    snapshots(lines).pop().unwrap_or_default()
}

/// Just the item rows of the last snapshot.
fn snapshot_items(lines: &[&str]) -> Vec<cena_protocol::InventoryItem> {
    snapshot(lines).items
}

mod manager {
    use super::*;

    #[test]
    fn every_row_reaches_the_frame() {
        let snap = snapshot(&[SNAPSHOT]);
        assert_eq!(snap.items.len(), 5, "one frame carries every row");
        assert_eq!(snap.room, "7503206");
    }

    #[test]
    fn an_item_row_is_not_a_structural_frame() {
        // The regression this whole file exists for. `<i>` was in
        // `is_markup`, so each row degraded to `Frame::Structural` with
        // every attribute trapped in an unparsed string.
        let strays = frames(&[SNAPSHOT])
            .iter()
            .filter(|f| matches!(f, Frame::Structural { name, .. } if name == "i"))
            .count();
        assert_eq!(strays, 0, "every row is typed, none fall through");
    }

    #[test]
    fn loc_splits_into_a_relation_and_a_parent() {
        let items = snapshot_items(&[SNAPSHOT]);
        let worn = &items[0];
        assert_eq!(
            (worn.relation.as_str(), worn.parent.as_str()),
            ("worn", "player")
        );
        let nested = items
            .iter()
            .find(|i| i.id == "309585758")
            .expect("the crown");
        assert_eq!(
            (nested.relation.as_str(), nested.parent.as_str()),
            ("in", "309585706"),
            "a nested item names the container holding it"
        );
    }

    #[test]
    fn a_bare_room_loc_has_no_parent_to_split() {
        let items = snapshot_items(&[concat!(
            r"<inventoryManager id='im1' room='1'>",
            r#"<i id='5' loc='room' name="a,stone,bench" weight='-1'/>"#,
            r"</inventoryManager>",
        )]);
        assert_eq!(
            (items[0].relation.as_str(), items[0].parent.as_str()),
            ("room", "room")
        );
    }

    #[test]
    fn the_name_is_three_comma_fields_and_rejoins_for_display() {
        // MEASURED: all 149 names in the sampled snapshot carry exactly two
        // commas, so the three-field split is the wire's own convention --
        // not a guess. The first field absorbs leading adjectives.
        let items = snapshot_items(&[SNAPSHOT]);
        let bow = &items[0];
        assert_eq!(
            (
                bow.article.as_str(),
                bow.adjective.as_str(),
                bow.noun.as_str()
            ),
            ("a scorched", "glowbark long", "bow")
        );
        assert_eq!(bow.name, "a scorched glowbark long bow");
    }

    #[test]
    fn a_name_that_does_not_split_keeps_the_whole_string_as_the_noun() {
        // Losing the item would be worse than an odd noun.
        let items = snapshot_items(&[concat!(
            r"<inventoryManager id='im1' room='1'>",
            r#"<i id='5' loc='worn,player' name="a plain sack" weight='1'/>"#,
            r"</inventoryManager>",
        )]);
        assert_eq!(items[0].noun, "a plain sack");
        assert_eq!(items[0].name, "a plain sack");
        assert!(items[0].article.is_empty());
    }

    #[test]
    fn long_descriptions_lose_their_emphasis_markers() {
        let items = snapshot_items(&[SNAPSHOT]);
        let sack = items
            .iter()
            .find(|i| i.id == "309585703")
            .expect("the sack");
        assert_eq!(
            sack.long.as_deref(),
            Some("an elesine treasure sack with a blood crystal clasp"),
            "`$_` brackets the emphasised span; it is display syntax, not text"
        );
    }

    #[test]
    fn an_unstated_field_is_none_rather_than_zero() {
        // §5.2: absent is not a fabricated zero. A weight of 0 is real --
        // rings and coins carry it -- so a missing weight must be
        // distinguishable from a stated one.
        let items = snapshot_items(&[SNAPSHOT]);
        assert_eq!(items[0].weight, Some(3));
        assert_eq!(items[0].in_max, None, "a bow is not a container");
        assert_eq!(items[0].encum, None);
    }

    #[test]
    fn capacity_decodes_from_the_packed_value() {
        // `v / 10` pounds, `v % 10` item count with 0 meaning unlimited.
        let items = snapshot_items(&[SNAPSHOT]);
        let cloak = items
            .iter()
            .find(|i| i.id == "309585706")
            .expect("the cloak");
        let cap = cloak.in_capacity().expect("it holds things");
        assert_eq!((cap.pounds, cap.max_items), (200, None));
        assert!(cloak.is_container());
        assert!(!items[0].is_container(), "a bow holds nothing");
    }

    #[test]
    fn flags_are_split_and_answerable() {
        // TWO flags, comma-separated. A mutation survived a single-flag
        // fixture: with only `closed` present, splitting on `,` and splitting
        // on `;` are indistinguishable, so the test proved nothing about the
        // separator. The wire sends `closed` alone in this corpus, but Lich
        // documents the attribute as `["closed", "locked"]`
        // (`reference/lich-5/lib/common/inventory.rb:188`), so the list form
        // is the shape to pin.
        let items = snapshot_items(&[concat!(
            r"<inventoryManager id='im1' room='1'>",
            r#"<i id='1' loc='worn,player' name="a,locked,chest" weight='9' flags='closed,locked'/>"#,
            r#"<i id='2' loc='worn,player' name="a,plain,sack" weight='1'/>"#,
            r"</inventoryManager>",
        )]);
        let chest = &items[0];
        assert_eq!(
            chest.flags,
            ["closed", "locked"],
            "two flags, split on the comma"
        );
        assert!(chest.is_closed() && chest.is_locked());
        assert!(
            !items[1].is_closed() && !items[1].is_locked(),
            "an unflagged item is neither"
        );
    }

    #[test]
    fn a_capacity_states_both_pounds_and_a_count() {
        // The packed value is `pounds * 10 + max_items`, and a mutant that
        // ignored the count would pass against `in_max='2000'` alone, whose
        // count is the unlimited 0.
        let items = snapshot_items(&[concat!(
            r"<inventoryManager id='im1' room='1'>",
            r#"<i id='1' loc='worn,player' name="a,small,pouch" weight='1' in_max='53'/>"#,
            r#"<i id='2' loc='worn,player' name="a,big,sack" weight='1' on_max='2000'/>"#,
            r"</inventoryManager>",
        )]);
        let pouch = items[0].in_capacity().expect("a pouch holds things");
        assert_eq!(
            (pouch.pounds, pouch.max_items),
            (5, Some(3)),
            "5 pounds and at most 3 items"
        );
        let surface = items[1].on_capacity().expect("things rest on it");
        assert_eq!(
            (surface.pounds, surface.max_items),
            (200, None),
            "a count of 0 means unlimited, not zero items"
        );
        assert_eq!(items[1].in_capacity(), None, "nothing goes INSIDE it");
    }

    #[test]
    fn a_fixture_cannot_be_picked_up() {
        // `-1` is the wire's sentinel, which is why `weight` is SIGNED.
        let items = snapshot_items(&[concat!(
            r"<inventoryManager id='im1' room='1'>",
            r#"<i id='5' loc='room' name="a,stone,bench" weight='-1'/>"#,
            r#"<i id='6' loc='room' name="a,loose,rock" weight='1'/>"#,
            r"</inventoryManager>",
        )]);
        assert!(!items[0].can_pick_up(), "furniture");
        assert!(items[1].can_pick_up());
    }

    #[test]
    fn a_row_that_cannot_be_anchored_is_skipped_not_guessed() {
        // No id or no loc means no place in the tree. Every other malformed
        // field degrades to a default instead.
        let items = snapshot_items(&[concat!(
            r"<inventoryManager id='im1' room='1'>",
            r#"<i loc='worn,player' name="a,lost,thing"/>"#,
            r#"<i id='7' name="a,placeless,thing"/>"#,
            r#"<i id='8' loc='worn,player' name="a,good,thing" weight='oops'/>"#,
            r"</inventoryManager>",
        )]);
        assert_eq!(items.len(), 1, "only the anchorable row");
        assert_eq!(items[0].id, "8");
        assert_eq!(
            items[0].weight, None,
            "unparsable degrades, it does not drop"
        );
    }

    #[test]
    fn a_continuation_is_a_cursor_not_an_item() {
        // Absent from the author's logs, and typed anyway: treating one as an
        // item would put a cursor in the inventory tree, and dropping it
        // would silently truncate the snapshot at a page boundary.
        let snap = snapshot(&[concat!(
            r"<inventoryManager id='im1' room='1' root='99' after='5' state='stale'>",
            r#"<i id='5' loc='worn,player' name="a,real,item" weight='1'/>"#,
            r"<continuation root='99' last='5'/>",
            r"</inventoryManager>",
        )]);
        assert_eq!(snap.items.len(), 1, "the cursor is not an item");
        assert_eq!(snap.continuations.len(), 1);
        assert_eq!(snap.continuations[0].root, "99");
        assert_eq!(snap.continuations[0].last, "5");
        assert_eq!(
            (
                snap.root.as_deref(),
                snap.after.as_deref(),
                snap.state.as_deref()
            ),
            (Some("99"), Some("5"), Some("stale")),
            "the envelope's own cursor echo and error marker survive"
        );
    }

    #[test]
    fn an_empty_snapshot_is_still_a_snapshot() {
        let items = snapshot_items(&[r"<inventoryManager id='im1' room='1'></inventoryManager>"]);
        assert!(items.is_empty());
    }
}

mod view_item {
    use super::*;
    use cena_protocol::{ItemDetail, ItemView};

    /// A real detail response, trimmed to two sections. The shape that
    /// matters is that it SPANS LINES -- the envelope and the first section
    /// open on one line, and the close arrives on another.
    const DETAIL: &[&str] = &[
        r#"<inventoryViewItem id='iv1' exist='309585704'><result command='look'>Silvery fur, and a <a exist="309585704" noun="satchel">grey wolf fur satchel</a>."#,
        r"</result><result command='inspect'>It is estimated.",
        r"</result></inventoryViewItem>",
    ];

    /// Every detail frame the lines produced, so a test asserts on the count
    /// rather than unwrapping. `cena-protocol`'s tests ban `panic!` and
    /// `expect()`, which is why nothing here reaches for either.
    fn details(lines: &[&str]) -> Vec<ItemView> {
        frames(lines)
            .into_iter()
            .filter_map(|f| match f {
                Frame::InventoryViewItem(view) => Some(view),
                _ => None,
            })
            .collect()
    }

    /// The LAST detail frame, or an empty one when none arrived.
    fn detail(lines: &[&str]) -> ItemView {
        details(lines).pop().unwrap_or_default()
    }

    /// One section's command and text, for terse assertions.
    fn sections(results: &[ItemDetail]) -> Vec<(&str, &str)> {
        results
            .iter()
            .map(|r| (r.command.as_str(), r.text.as_str()))
            .collect()
    }

    #[test]
    fn the_whole_response_is_one_frame() {
        let view = detail(DETAIL);
        assert_eq!(details(DETAIL).len(), 1, "three lines, one frame");
        assert_eq!(
            (view.token.as_str(), view.exist.as_str()),
            ("iv1", "309585704")
        );
        assert!(!view.closed);
        assert_eq!(view.state, None);
        assert_eq!(
            sections(&view.results),
            [
                ("look", "Silvery fur, and a grey wolf fur satchel."),
                ("inspect", "It is estimated."),
            ]
        );
    }

    #[test]
    fn the_first_section_is_not_lost_to_the_window_bag() {
        // The defect this test was written from. The envelope opened the
        // capture, but the REST of that same line kept parsing normally, so
        // `<result command='look'>` routed to `Frame::WindowHints` and its
        // section vanished -- MEASURED against a real block, 39 sections
        // captured where the wire sent 52: exactly one lost per response.
        let strays = frames(DETAIL)
            .iter()
            .filter(|f| matches!(f, Frame::WindowHints { id, .. } if id == "result"))
            .count();
        assert_eq!(strays, 0, "no section reaches the placement bag");
        assert_eq!(
            detail(DETAIL).results.len(),
            2,
            "both sections, including the first"
        );
    }

    #[test]
    fn a_section_keeps_its_links_rather_than_flattening_them() {
        // `VellumFE` flattens these to plain text. This parser already types
        // links, so a frontend can make the nouns in a description
        // clickable, exactly as the game intends.
        let results = detail(DETAIL).results;
        let links: Vec<&str> = results
            .first()
            .map(|r| r.links.iter().map(|l| l.text.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(links, ["grey wolf fur satchel"]);
        assert!(
            results
                .first()
                .is_some_and(|r| r.text.contains("grey wolf fur satchel")),
            "the link text is still part of the prose"
        );
    }

    #[test]
    fn a_line_boundary_inside_a_section_is_a_newline() {
        // `analyze` and `inspect` arrive formatted with indented tables and
        // blank separators; flattening them runs the paragraphs together.
        let results = detail(&[
            r"<inventoryViewItem id='iv1' exist='9'><result command='analyze'>First line.",
            r"Second line.",
            r"</result></inventoryViewItem>",
        ])
        .results;
        assert_eq!(
            sections(&results),
            [("analyze", "First line.\nSecond line.")]
        );
    }

    #[test]
    fn a_closed_container_is_flagged_by_presence() {
        // The value is irrelevant; the wire sends `closed='1'` and Saga's own
        // client checks only whether the attribute is there.
        assert!(
            detail(&[
                r"<inventoryViewItem id='iv1' exist='9' closed='1'><result command='look'>Nothing unusual.",
                r"</result></inventoryViewItem>",
            ])
            .closed
        );
        assert!(
            !detail(&[
                r"<inventoryViewItem id='iv1' exist='9'><result command='look'>Nothing unusual.",
                r"</result></inventoryViewItem>",
            ])
            .closed
        );
    }

    #[test]
    fn a_self_closing_section_is_empty_rather_than_absent() {
        let results = detail(&[
            r"<inventoryViewItem id='iv1' exist='9'><result command='look'/><result command='recall'>Enhancive.",
            r"</result></inventoryViewItem>",
        ])
        .results;
        assert_eq!(
            sections(&results),
            [("look", ""), ("recall", "Enhancive.")],
            "the empty section still happened, and is not the same as unasked"
        );
    }

    #[test]
    fn a_self_closing_envelope_is_a_complete_empty_response() {
        let view = detail(&[r"<inventoryViewItem id='iv1' exist='9'/>"]);
        assert_eq!(view.exist, "9");
        assert_eq!(view.state, None, "empty is not an error");
        assert!(view.results.is_empty());
    }

    #[test]
    fn a_prompt_tears_the_block_and_the_stream_resumes() {
        // The prompt is the resync barrier. Swallowing it would consume the
        // rest of the session into a capture that never closes.
        let lines = [
            r"<inventoryViewItem id='iv1' exist='9'><result command='look'>Half a des",
            r"<prompt time='1'>&gt;</prompt>",
            r"Ordinary game text.",
        ];
        let view = detail(&lines);
        assert_eq!(view.state.as_deref(), Some("malformed"));
        assert_eq!(
            view.results.len(),
            1,
            "what was captured is kept, not discarded"
        );

        let out = frames(&lines);
        assert!(
            out.iter().any(|f| matches!(f, Frame::Prompt { .. })),
            "the prompt itself parses normally"
        );
        assert!(
            out.iter()
                .any(|f| matches!(f, Frame::Text(t) if t.content.contains("Ordinary"))),
            "the stream resumed"
        );
    }

    #[test]
    fn a_second_envelope_surfaces_the_first_rather_than_merging() {
        // An open capture owns the line before dispatch sees it, so without
        // an explicit arm the second envelope was flattened into the first
        // block's prose and lost entirely.
        let seen: Vec<(String, Option<String>)> = details(&[
            r"<inventoryViewItem id='iv1' exist='1'><result command='look'>First.",
            r"<inventoryViewItem id='iv2' exist='2'><result command='look'>Second.",
            r"</result></inventoryViewItem>",
        ])
        .into_iter()
        .map(|v| (v.exist, v.state))
        .collect();
        assert_eq!(
            seen,
            [
                ("1".to_owned(), Some("malformed".to_owned())),
                ("2".to_owned(), None),
            ]
        );
    }

    #[test]
    fn text_after_the_close_is_ordinary_feed_again() {
        let out = frames(&[
            r"<inventoryViewItem id='iv1' exist='9'><result command='look'>A thing.",
            r"</result></inventoryViewItem>You also see a rock.",
        ]);
        assert!(
            out.iter()
                .any(|f| matches!(f, Frame::Text(t) if t.content.contains("You also see a rock"))),
            "got {out:?}"
        );
    }

    #[test]
    fn an_unterminated_block_is_bounded_by_lines() {
        // The MULTI-LINE CAPTURES note requires this: the removed capture
        // path was bounded by BYTES and could swallow 4,297 consecutive
        // lines of game text before the cap fired. MEASURED span of a real
        // block is 7 to 50 lines.
        let mut lines =
            vec![r"<inventoryViewItem id='iv1' exist='9'><result command='look'>Start."];
        lines.extend(std::iter::repeat_n("more text", 260));
        let view = detail(&lines);
        assert_eq!(
            view.state.as_deref(),
            Some("truncated"),
            "the capture gives up and says so rather than consuming the stream"
        );
        assert_eq!(view.results.len(), 1, "what was captured is still surfaced");
    }

    #[test]
    fn many_captures_on_one_line_do_not_grow_the_stack() {
        // Each open/close used to be a nested call -- `parse_line` into the
        // capture, and the close back into `parse_line`. MEASURED: 2,000
        // pairs on one 92 KB line (under the 256 KB line cap) overflowed a
        // 2 MiB stack in release, and 500 did in debug. An overflow aborts
        // the process rather than panicking, so every session died with it.
        //
        // Run on a deliberately SMALL stack, so a regression aborts this test
        // binary at any build profile rather than depending on the default.
        const PAIRS: usize = 2_000;
        let line = format!(
            "{}And then some prose.",
            "<inventoryViewItem id='t'></inventoryViewItem>".repeat(PAIRS)
        );
        let spawned = std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(move || Parser::new().parse_line(&line));
        let Ok(handle) = spawned else {
            unreachable!("the test thread could not be spawned");
        };
        let out = handle.join().unwrap_or_default();

        let views = out
            .iter()
            .filter(|f| matches!(f, Frame::InventoryViewItem(_)))
            .count();
        assert_eq!(views, PAIRS, "every pair is one detail frame");
        assert!(
            out.iter()
                .any(|f| matches!(f, Frame::Text(t) if t.content.contains("And then some prose"))),
            "the prose after the last close is ordinary feed"
        );
    }

    #[test]
    fn text_after_a_self_closing_second_envelope_is_not_dropped() {
        // A torn block followed by a complete, empty response on the same
        // line. The rest of the line was fed onward only if the new capture
        // stayed open, so after a self-closing envelope it vanished.
        let out = frames(&[
            r"<inventoryViewItem id='1' exist='9'><result command='look'>Torn.",
            r"<inventoryViewItem id='2' exist='9'/>You also see a rock.",
        ]);
        assert!(
            out.iter()
                .any(|f| matches!(f, Frame::Text(t) if t.content.contains("You also see a rock"))),
            "got {out:?}"
        );
    }
}
