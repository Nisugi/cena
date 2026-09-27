//! The script checker (`plan/46` §11 step 5): a script of Hydra's own with a
//! line of each kind the checker finds, and lines it must not find, read by
//! the runner's own Ruby against the runner's own names.
//!
//! **It needs Ruby 4.0**, as the runner does (`tests/runner.rs`).

use std::path::Path;

use cena_agent::scripts::checker::{Verdict, check};
use cena_agent::scripts::runner::{find_ruby, unpack};

#[tokio::test(flavor = "current_thread")]
async fn the_checker_finds_each_kind_and_nothing_else() {
    let ruby = find_ruby().expect("no Ruby: the checker needs Ruby 4.0, as the runner does");
    let dir = std::env::temp_dir().join(format!("cena-checker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    unpack(&dir.join("runner")).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checktest.lic");

    let checked = check(&ruby, &dir.join("runner"), &dir.join("data"), &[fixture])
        .await
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let script = &checked[0];
    assert_eq!(
        (script.name.as_str(), script.verdict),
        ("checktest", Verdict::Stops)
    );
    let found: Vec<(u64, Verdict, &str, bool)> = script
        .findings
        .iter()
        .map(|f| (f.line, f.kind, f.what.as_str(), f.hydra))
        .collect();
    assert_eq!(
        found,
        [
            (3, Verdict::Stops, "Stats", true),
            (4, Verdict::Stops, "Spell#cast", true),
            (5, Verdict::Stops, "XMLData.bounty_task", true),
            (6, Verdict::Stops, "File.exists?", false),
            (7, Verdict::Stops, "undefined_helper", false),
            (8, Verdict::Markup, "status_tags", true),
            (9, Verdict::Windows, "Gtk::Window", true),
            (
                10,
                Verdict::Differs,
                "DownstreamHook pattern <pushBold",
                true
            ),
            (11, Verdict::Markup, "$_SERVERBUFFER_", true),
        ],
        "{:#?}",
        script.findings
    );
    assert!(
        script.findings[1].why.contains("cast with fput"),
        "a method not answered yet says what to use instead"
    );
}
