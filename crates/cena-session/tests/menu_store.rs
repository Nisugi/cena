//! The supplemental menu dictionary on disk.
//!
//! # What these tests can and cannot be cut from
//!
//! Every row here that is not a real `<cmdlist>` row is INVENTED, and that is
//! unavoidable rather than lazy: MEASURED, the rows the server pushes today
//! are byte-identical to the shipped table, so `novel()` reports nothing and a
//! supplemental file is never created on a current install. The file only
//! earns its keep on the day Simutronics changes a command.
//!
//! So the merge rules are tested against real rows (`cena-model`'s
//! `cmdlist_updates.rs`), and the disk half against invented ones, which is
//! the honest division.

use std::fs;
use std::path::PathBuf;

use cena_model::{LearnedCommands, MenuCommand, MenuCommands};
use cena_session::menu_store;

/// A directory this test alone owns, cleaned up when it drops.
///
/// Named after the test, following `character_store.rs:18`, so a failure
/// leaves behind a directory that says which test wrote it. Not pre-created:
/// `save` calls `create_dir_all` itself, and a test that created the directory
/// first would hide a regression that dropped it.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("cena-menu-store-test-{tag}"));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn row(coord: &str, label: &str) -> MenuCommand {
    MenuCommand {
        coord: coord.to_owned(),
        label: label.to_owned(),
        command: format!("{label} #"),
        category: "6".to_owned(),
    }
}

fn learned(rows: &[MenuCommand], version: &str) -> LearnedCommands {
    let mut learned = LearnedCommands::default();
    learned.absorb(rows);
    learned.set_version(version);
    learned
}

#[test]
fn a_missing_file_is_an_empty_dictionary_not_an_error() {
    // The ordinary state of a current install: nothing novel has ever been
    // pushed, so no file was ever written. Failing here would refuse to start.
    let scratch = Scratch::new("missing");
    let loaded = menu_store::load(&scratch.0).expect("a missing file loads");
    assert!(loaded.is_empty());
    assert_eq!(loaded.version(), None);
}

#[test]
fn a_saved_dictionary_reads_back_identical() {
    let scratch = Scratch::new("roundtrip");
    let original = learned(
        &[row("9999,1", "frobnicate"), row("9999,2", "defenestrate")],
        "1788300900.1.1.1",
    );
    menu_store::save(&scratch.0, &original).expect("save");

    let loaded = menu_store::load(&scratch.0).expect("load");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded.version(), Some("1788300900.1.1.1"));
    assert_eq!(
        loaded.entry("9999,1").map(|r| r.label.as_str()),
        Some("frobnicate")
    );
}

#[test]
fn the_file_is_the_shipped_tables_format() {
    // So folding it back in is a concatenate-and-sort, not a translation.
    let scratch = Scratch::new("format");
    menu_store::save(&scratch.0, &learned(&[row("9999,1", "frobnicate")], "42.1")).expect("save");

    let text = fs::read_to_string(menu_store::store_path(&scratch.0)).expect("read");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "# cmdtimestamp\t42.1");
    assert_eq!(lines[1], "coord\tlabel\tcommand\tcategory");
    assert_eq!(lines[2], "9999,1\tfrobnicate\tfrobnicate #\t6");
}

#[test]
fn a_second_push_rewrites_the_one_file_with_existing_plus_new() {
    // The author's sequence, and the reason one file is enough: the file is
    // always the accumulated set, never a fragment that needs another file to
    // be understood.
    let scratch = Scratch::new("accumulate");
    menu_store::merge_and_save(&scratch.0, &learned(&[row("9999,1", "frobnicate")], "1"))
        .expect("first push");
    menu_store::merge_and_save(&scratch.0, &learned(&[row("9999,2", "defenestrate")], "2"))
        .expect("second push");

    let files: Vec<_> = fs::read_dir(&scratch.0)
        .expect("dir")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(files.len(), 1, "one file, not a fragment per push");

    let loaded = menu_store::load(&scratch.0).expect("load");
    assert_eq!(loaded.len(), 2, "existing + new");
    assert_eq!(loaded.version(), Some("2"), "and the newer timestamp");
}

#[test]
fn a_repeated_coordinate_replaces_on_disk_too() {
    // Disk and memory cannot disagree about the merge: both go through
    // `LearnedCommands::absorb`.
    let scratch = Scratch::new("replace");
    menu_store::merge_and_save(&scratch.0, &learned(&[row("9999,1", "frobnicate")], "1"))
        .expect("first");
    menu_store::merge_and_save(&scratch.0, &learned(&[row("9999,1", "defrobnicate")], "2"))
        .expect("second");

    let loaded = menu_store::load(&scratch.0).expect("load");
    assert_eq!(loaded.len(), 1, "one row, not two");
    assert_eq!(
        loaded.entry("9999,1").map(|r| r.label.as_str()),
        Some("defrobnicate")
    );
}

#[test]
fn a_row_the_shipped_table_already_holds_is_not_written() {
    // The file's purpose is to show what the shipped table is MISSING. Writing
    // a row identical to the baseline would mean a fold-back reviewer reads
    // 1,106 rows to find the two that changed.
    let scratch = Scratch::new("noise");
    let shipped = MenuCommands::get()
        .entry("2524,1543")
        .expect("attack")
        .clone();
    assert_eq!(shipped.label, "attack @", "guard: the baseline row");

    let written = menu_store::merge_and_save(&scratch.0, &learned(&[shipped], "1")).expect("merge");
    assert!(written.is_none(), "nothing novel, so nothing written");
    assert!(!menu_store::store_path(&scratch.0).exists());
}

#[test]
fn save_itself_drops_a_baseline_row_not_just_its_caller() {
    // MUTATION-FOUND. `save` writing `all()` instead of `novel()` survived the
    // whole suite: `merge_and_save` returns early when nothing is novel, so it
    // never reaches the write, and `prune` filters before calling `save`. Both
    // callers hid the bug. This calls `save` directly with a mixed set, which
    // is the only shape that sees it.
    let scratch = Scratch::new("save_filters");
    let shipped = MenuCommands::get()
        .entry("2524,1543")
        .expect("attack")
        .clone();
    menu_store::save(
        &scratch.0,
        &learned(&[shipped, row("9999,1", "frobnicate")], "1"),
    )
    .expect("save");

    let loaded = menu_store::load(&scratch.0).expect("load");
    assert_eq!(loaded.len(), 1, "only the row the shipped table lacks");
    assert!(
        loaded.entry("2524,1543").is_none(),
        "the baseline row is noise"
    );
}

#[test]
fn a_malformed_row_is_skipped_and_the_good_rows_survive() {
    // Refusing the whole file over one bad line would discard every good one.
    let scratch = Scratch::new("malformed");
    fs::create_dir_all(&scratch.0).expect("scratch dir");
    fs::write(
        menu_store::store_path(&scratch.0),
        "# cmdtimestamp\t7\ncoord\tlabel\tcommand\tcategory\n\
         9999,1\tfrobnicate\tfrobnicate #\t6\n\
         this line has no tabs at all\n\
         \tno\tcoord\there\n\
         9999,2\tdefenestrate\tdefenestrate #\t6\n",
    )
    .expect("write");

    let loaded = menu_store::load(&scratch.0).expect("load");
    assert_eq!(loaded.len(), 2, "both good rows");
    assert_eq!(loaded.version(), Some("7"));
}

mod staleness_chain {
    use super::{MenuCommands, Scratch, fs, menu_store};

    #[test]
    fn the_shipped_table_states_its_version() {
        // The baseline of the chain. Carried in the data file so a release
        // that folds rows in updates one thing, not two.
        assert_eq!(MenuCommands::get().version(), Some("1788300900.1.1.1"));
    }

    #[test]
    fn a_release_that_absorbs_a_row_prunes_it_from_the_file() {
        // The author's chain end to end: a row that WAS novel stops being
        // novel once the shipped table holds it, and pruning drops it.
        //
        // Simulated by storing a row the shipped table already has, which is
        // indistinguishable from the real case -- "a release absorbed it" and
        // "it was always there" produce the same comparison.
        let scratch = Scratch::new("prune");
        let absorbed = MenuCommands::get()
            .entry("2524,1543")
            .expect("attack")
            .clone();
        fs::create_dir_all(&scratch.0).expect("scratch dir");
        fs::write(
            menu_store::store_path(&scratch.0),
            format!(
                "# cmdtimestamp\t9\ncoord\tlabel\tcommand\tcategory\n\
                 {}\t{}\t{}\t{}\n9999,1\tfrobnicate\tfrobnicate #\t6\n",
                absorbed.coord, absorbed.label, absorbed.command, absorbed.category
            ),
        )
        .expect("write");
        assert_eq!(
            menu_store::load(&scratch.0).expect("load").len(),
            2,
            "guard: both rows stored first"
        );

        menu_store::prune(&scratch.0).expect("prune");

        let loaded = menu_store::load(&scratch.0).expect("reload");
        assert_eq!(loaded.len(), 1, "the absorbed row is gone");
        assert!(loaded.entry("9999,1").is_some(), "the novel one stays");
    }

    #[test]
    fn a_file_with_nothing_left_is_deleted_not_left_empty() {
        // An empty supplemental file states "there are learned rows", and
        // there are not.
        let scratch = Scratch::new("prune_empty");
        let absorbed = MenuCommands::get()
            .entry("2524,1543")
            .expect("attack")
            .clone();
        fs::create_dir_all(&scratch.0).expect("scratch dir");
        fs::write(
            menu_store::store_path(&scratch.0),
            format!(
                "# cmdtimestamp\t9\ncoord\tlabel\tcommand\tcategory\n{}\t{}\t{}\t{}\n",
                absorbed.coord, absorbed.label, absorbed.command, absorbed.category
            ),
        )
        .expect("write");

        assert!(menu_store::prune(&scratch.0).expect("prune").is_none());
        assert!(!menu_store::store_path(&scratch.0).exists());
    }
}

#[test]
fn a_restored_baseline_removes_the_file_rather_than_leaving_it() {
    // **A TRANSITION, NOT A SHAPE.** Every test above checks one state:
    // novel rows present, or none present from the start. This checks the
    // move BETWEEN them, which is where the defect was.
    //
    // The server can restore an overridden command to its shipped value. When
    // that was the only override, the merged set becomes non-novel and
    // `merge_and_save` used to return early -- leaving the old file on disk,
    // so the next `load` resurrected a command the server had already
    // withdrawn.
    let scratch = Scratch::new("restored");
    let shipped = MenuCommands::get()
        .entry("2524,1543")
        .expect("attack")
        .clone();
    assert_eq!(shipped.label, "attack @", "guard: the baseline row");

    // The server overrides it, and the override is written.
    let overridden = MenuCommand {
        label: "ambush @".to_owned(),
        ..shipped.clone()
    };
    let written = menu_store::merge_and_save(&scratch.0, &learned(&[overridden], "1"))
        .expect("merge")
        .expect("an override is novel, so it is written");
    assert!(written.exists(), "guard: the override reached disk");

    // The server restores the shipped value. Nothing is novel any more.
    let after = menu_store::merge_and_save(&scratch.0, &learned(&[shipped], "2")).expect("merge");
    assert!(after.is_none(), "nothing novel, so nothing is written");
    assert!(
        !menu_store::store_path(&scratch.0).exists(),
        "the stale file must be REMOVED, not merely left unwritten -- \
         otherwise the next load returns the withdrawn override"
    );

    // And the proof that matters: a reload does not resurrect it.
    let reloaded = menu_store::load(&scratch.0).expect("load");
    assert!(
        reloaded.entry("2524,1543").is_none(),
        "the withdrawn override came back: {:?}",
        reloaded.entry("2524,1543")
    );
}
