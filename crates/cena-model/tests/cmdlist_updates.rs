//! `<cmdlist>`: the server keeping the client's menu dictionary current.
//!
//! The shipped table (`data/menu_commands.tsv`) is not static data a client
//! is expected to find -- it is the Wrayth client's **cache** of these
//! pushes. VERIFIED live, in a Wrayth-banner session:
//!
//! ```text
//! <cmdlist><cli coord="2524,12785" menu="sense @" command="sense #"
//!               menu_cat="5_roleplay"/>…</cmdlist>
//! <cmdtimestamp data='1788300900.1.1.1'/>
//! ```
//!
//! That timestamp is **exactly** the one in the header of
//! `%APPDATA%/Wrayth/GS4/cmdlist1.xml`, which is what identifies the file as
//! a cache rather than a shipped asset.
//!
//! Both rows arrived as separate `Frame::WindowHints` bags before this --
//! carrying every attribute, so nothing was lost, but with nothing joining
//! them and no consumer reading them.

use cena_model::GameState;
use cena_protocol::Parser;

/// The real push, verbatim from the captured stream.
const PUSH: &str = concat!(
    r#"<cmdlist><cli coord="2524,12785" menu="sense @" command="sense #" menu_cat="5_roleplay"/>"#,
    r#"<cli coord="2524,12784" menu="whisper @ about %" command="whisper # about %" menu_cat="9_questions"/>"#,
    r#"</cmdlist><cmdtimestamp data='1788300900.1.1.1'/>"#,
);

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    state
}

#[test]
fn a_push_teaches_the_session_its_rows() {
    let state = state_after(&[PUSH]);
    let learned = &state.learned_commands;
    assert_eq!(learned.len(), 2);
    let sense = learned.entry("2524,12785").expect("sense");
    assert_eq!(sense.label, "sense @");
    assert_eq!(sense.command, "sense #");
    assert_eq!(sense.category, "5_roleplay");
}

#[test]
fn the_version_is_recorded() {
    // What a client compares against to know whether it is current.
    assert_eq!(
        state_after(&[PUSH]).learned_commands.version(),
        Some("1788300900.1.1.1")
    );
}

#[test]
fn nothing_is_claimed_before_the_server_states_it() {
    // §5.2: the shipped table has its own version, and claiming it as the
    // session's would make a stale cache look current.
    let fresh = GameState::default();
    assert_eq!(fresh.learned_commands.version(), None);
    assert!(fresh.learned_commands.is_empty());
}

#[test]
fn a_timestamp_alone_is_the_server_saying_we_are_current() {
    // A push with no rows is meaningful: it states a version and teaches
    // nothing, which is what "you are up to date" looks like.
    let state = state_after(&[r"<cmdtimestamp data='1788300900.1.1.1'/>"]);
    assert_eq!(state.learned_commands.version(), Some("1788300900.1.1.1"));
    assert!(state.learned_commands.is_empty());
}

#[test]
fn the_push_matches_what_the_shipped_table_already_holds() {
    // Independent confirmation that taking the live client's `cmdlist1.xml`
    // was right: the rows the server is actively pushing are byte-identical
    // to the ones shipped, so `novel()` reports nothing to fold back.
    let state = state_after(&[PUSH]);
    assert_eq!(state.learned_commands.len(), 2, "guard: rows were learned");
    assert_eq!(
        state.learned_commands.novel().count(),
        0,
        "nothing differs from the baseline"
    );
}

mod merging {
    use super::*;

    /// A row for a coordinate the shipped table does not have.
    pub(super) const NOVEL: &str = concat!(
        r#"<cmdlist><cli coord="9999,1" menu="frobnicate @" command="frobnicate #" "#,
        r#"menu_cat="6"/></cmdlist>"#,
    );

    #[test]
    fn a_new_coordinate_is_learned_and_reported_as_novel() {
        let state = state_after(&[NOVEL]);
        let novel: Vec<&str> = state
            .learned_commands
            .novel()
            .map(|r| r.coord.as_str())
            .collect();
        assert_eq!(novel, ["9999,1"], "what a supplemental file must carry");
    }

    #[test]
    fn a_repeated_coordinate_replaces_rather_than_duplicating() {
        // The server is the authority on what a coordinate means, INCLUDING
        // when it changes meaning. Keeping the first would ignore the change.
        let state = state_after(&[
            NOVEL,
            concat!(
                r#"<cmdlist><cli coord="9999,1" menu="defrobnicate @" "#,
                r#"command="defrobnicate #" menu_cat="7"/></cmdlist>"#,
            ),
        ]);
        assert_eq!(state.learned_commands.len(), 1, "one row, not two");
        let row = state.learned_commands.entry("9999,1").expect("the row");
        assert_eq!(row.label, "defrobnicate @");
        assert_eq!(row.category, "7", "the category changed too");
    }

    #[test]
    fn pushes_accumulate_across_the_session() {
        // Additive: a delta teaches what changed, and earlier rows stay.
        let state = state_after(&[PUSH, NOVEL]);
        assert_eq!(state.learned_commands.len(), 3);
        assert!(state.learned_commands.entry("2524,12785").is_some());
        assert!(state.learned_commands.entry("9999,1").is_some());
    }

    #[test]
    fn a_row_with_no_coordinate_is_skipped() {
        // The coordinate is the key; a row without one cannot be stored or
        // ever looked up.
        let state = state_after(&[r#"<cmdlist><cli menu="ghost @" command="ghost #"/></cmdlist>"#]);
        assert!(state.learned_commands.is_empty());
    }
}

mod resolution {
    use super::*;
    use cena_model::MenuCommands;
    use cena_protocol::Frame;

    fn menu(line: &str) -> cena_protocol::Menu {
        Parser::new()
            .parse_line(line)
            .into_iter()
            .find_map(|f| match f {
                Frame::MenuResponse(m) => Some(m),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_learned_row_resolves_a_coordinate_the_table_lacks() {
        // The point of the whole mechanism: the client's copy lags the
        // server's, and this is how it catches up mid-session.
        let state = state_after(&[super::merging::NOVEL]);
        let m = menu(r#"<menu id="1" cat_list="6"><mi coord="9999,1"/></menu>"#);

        let without = MenuCommands::get().resolve(&m, "123", None, None);
        assert_eq!(
            without[0].label, None,
            "guard: unknown to the shipped table"
        );

        let with = MenuCommands::get().resolve(&m, "123", None, Some(&state.learned_commands));
        assert_eq!(with[0].label.as_deref(), Some("frobnicate"));
        assert_eq!(with[0].command.as_deref(), Some("frobnicate #123"));
        assert_eq!(with[0].category.as_deref(), Some("6"));
    }

    #[test]
    fn a_learned_row_outranks_the_shipped_one() {
        // A push is the game stating what a coordinate means NOW; the
        // shipped table is a cache of older pushes and can only be staler.
        let state = state_after(&[concat!(
            r#"<cmdlist><cli coord="2524,1543" menu="assault @" command="assault #" "#,
            r#"menu_cat="6"/></cmdlist>"#,
        )]);
        let m = menu(r#"<menu id="1" cat_list="6"><mi coord="2524,1543"/></menu>"#);

        assert_eq!(
            MenuCommands::get()
                .entry("2524,1543")
                .map(|e| e.label.as_str()),
            Some("attack @"),
            "guard: the shipped table says attack"
        );
        let with = MenuCommands::get().resolve(&m, "123", None, Some(&state.learned_commands));
        assert_eq!(with[0].label.as_deref(), Some("assault"));
    }
}

#[test]
fn what_the_server_taught_survives_a_reconnect() {
    // What a coordinate MEANS is a fact about the game, not about the
    // connection -- and the push is not repeated on reconnect, it arrives
    // when the server decides the client is behind. Clearing it would
    // silently fall back to the shipped table's older copy of a row the
    // server has since changed.
    let mut state = state_after(&[PUSH]);
    assert_eq!(state.learned_commands.len(), 2, "guard: known first");
    state.invalidate_for_reconnect();
    assert_eq!(state.learned_commands.len(), 2);
    assert_eq!(
        state.learned_commands.version(),
        Some("1788300900.1.1.1"),
        "including the version, or we would re-request needlessly"
    );
}
