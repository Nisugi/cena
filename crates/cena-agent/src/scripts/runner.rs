//! The Ruby script runner, carried in Hydra, and how one is started
//! (`plan/46` §2, §10 question 9).
//!
//! **Its files travel in the binary** (`bridges/ruby`): Hydra's own edge, and
//! Lich's engine unchanged. Before a runner starts they are written out to a
//! folder of Hydra's, so the runner Hydra starts is always the one this
//! build speaks `hydra-script/1` with, whatever an older run left there.
//!
//! **The player's Ruby runs it**, found as a Lich player's is: `ruby` on the
//! `PATH`, else where Lich's Windows installer puts it
//! (`C:\Ruby4Lich5\<version>\bin`). Lich's engine needs Ruby 4.0; the runner
//! says so and stops on an older one.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::gemstone::runner::FILES as GAME_FILES;

/// The runner's files, by their path under the folder they are written to:
/// its own (`bridges/ruby/hydra`), and Lich's engine as upstream wrote it
/// (`bridges/ruby/lich`, `bridges/ruby/README.md`). The game's own are the
/// game module's (`crate::gemstone::runner`, `plan/05` Rule 3.4).
pub const FILES: &[(&str, &str)] = &[
    (
        "hydra/runner.rb",
        include_str!("../../../../bridges/ruby/hydra/runner.rb"),
    ),
    (
        "hydra/engine.rb",
        include_str!("../../../../bridges/ruby/hydra/engine.rb"),
    ),
    (
        "hydra/check.rb",
        include_str!("../../../../bridges/ruby/hydra/check.rb"),
    ),
    (
        "hydra/rexml.rb",
        include_str!("../../../../bridges/ruby/hydra/rexml.rb"),
    ),
    (
        "hydra/connection.rb",
        include_str!("../../../../bridges/ruby/hydra/connection.rb"),
    ),
    (
        "hydra/edge.rb",
        include_str!("../../../../bridges/ruby/hydra/edge.rb"),
    ),
    (
        "hydra/listener.rb",
        include_str!("../../../../bridges/ruby/hydra/listener.rb"),
    ),
    (
        "hydra/copy.rb",
        include_str!("../../../../bridges/ruby/hydra/copy.rb"),
    ),
    (
        "hydra/map.rb",
        include_str!("../../../../bridges/ruby/hydra/map.rb"),
    ),
    (
        "hydra/spell.rb",
        include_str!("../../../../bridges/ruby/hydra/spell.rb"),
    ),
    (
        "hydra/builtins.rb",
        include_str!("../../../../bridges/ruby/hydra/builtins.rb"),
    ),
    (
        "hydra/hooks.rb",
        include_str!("../../../../bridges/ruby/hydra/hooks.rb"),
    ),
    (
        "hydra/infomon.rb",
        include_str!("../../../../bridges/ruby/hydra/infomon.rb"),
    ),
    (
        "lich/lib/common/gtk.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/gtk.rb"),
    ),
    (
        "lich/lib/util/gtk_compaction.rb",
        include_str!("../../../../bridges/ruby/lich/lib/util/gtk_compaction.rb"),
    ),
    (
        "lich/lib/common/hook_registry.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/hook_registry.rb"),
    ),
    (
        "lich/lib/common/downstreamhook.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/downstreamhook.rb"),
    ),
    (
        "lich/lib/common/upstreamhook.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/upstreamhook.rb"),
    ),
    (
        "lich/lib/common/class_exts/hash.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/class_exts/hash.rb"),
    ),
    (
        "lich/lib/common/class_exts/matchdata.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/class_exts/matchdata.rb"),
    ),
    (
        "lich/lib/common/class_exts/numeric.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/class_exts/numeric.rb"),
    ),
    (
        "lich/lib/common/class_exts/string.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/class_exts/string.rb"),
    ),
    (
        "lich/lib/common/class_exts/stringproc.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/class_exts/stringproc.rb"),
    ),
    (
        "lich/lib/attributes/stats.rb",
        include_str!("../../../../bridges/ruby/lich/lib/attributes/stats.rb"),
    ),
    (
        "lich/lib/attributes/skills.rb",
        include_str!("../../../../bridges/ruby/lich/lib/attributes/skills.rb"),
    ),
    (
        "lich/lib/attributes/spells.rb",
        include_str!("../../../../bridges/ruby/lich/lib/attributes/spells.rb"),
    ),
    (
        "lich/lib/attributes/resources.rb",
        include_str!("../../../../bridges/ruby/lich/lib/attributes/resources.rb"),
    ),
    (
        "lich/lib/util/util.rb",
        include_str!("../../../../bridges/ruby/lich/lib/util/util.rb"),
    ),
    (
        "lich/lib/util/deep_freeze.rb",
        include_str!("../../../../bridges/ruby/lich/lib/util/deep_freeze.rb"),
    ),
    (
        "lich/LICENSE.txt",
        include_str!("../../../../bridges/ruby/lich/LICENSE.txt"),
    ),
    (
        "lich/lib/version.rb",
        include_str!("../../../../bridges/ruby/lich/lib/version.rb"),
    ),
    (
        "lich/lib/constants.rb",
        include_str!("../../../../bridges/ruby/lich/lib/constants.rb"),
    ),
    (
        "lich/lib/common/gameobj.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/gameobj.rb"),
    ),
    (
        "lich/lib/attributes/char.rb",
        include_str!("../../../../bridges/ruby/lich/lib/attributes/char.rb"),
    ),
    (
        "lich/lib/lich.rb",
        include_str!("../../../../bridges/ruby/lich/lib/lich.rb"),
    ),
    (
        "lich/lib/common/settings.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/settings.rb"),
    ),
    (
        "lich/lib/common/settings/database_adapter.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/settings/database_adapter.rb"),
    ),
    (
        "lich/lib/common/settings/path_navigator.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/settings/path_navigator.rb"),
    ),
    (
        "lich/lib/common/settings/settings_proxy.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/settings/settings_proxy.rb"),
    ),
    (
        "lich/lib/common/settings/instance_settings.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/settings/instance_settings.rb"),
    ),
    (
        "lich/lib/common/settings/sessions_settings.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/settings/sessions_settings.rb"),
    ),
    (
        "lich/lib/common/settings/session_database_adapter.rb",
        include_str!(
            "../../../../bridges/ruby/lich/lib/common/settings/session_database_adapter.rb"
        ),
    ),
    (
        "lich/lib/common/settings/charsettings.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/settings/charsettings.rb"),
    ),
    (
        "lich/lib/common/settings/gamesettings.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/settings/gamesettings.rb"),
    ),
    (
        "lich/lib/common/vars.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/vars.rb"),
    ),
    (
        "lich/lib/common/uservars.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/uservars.rb"),
    ),
    (
        "lich/lib/common/class_exts/nilclass.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/class_exts/nilclass.rb"),
    ),
    (
        "lich/lib/common/limitedarray.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/limitedarray.rb"),
    ),
    (
        "lich/lib/common/feature_flags.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/feature_flags.rb"),
    ),
    (
        "lich/lib/messaging.rb",
        include_str!("../../../../bridges/ruby/lich/lib/messaging.rb"),
    ),
    (
        "lich/lib/common/detachable_client_registry.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/detachable_client_registry.rb"),
    ),
    (
        "lich/lib/common/xml_entities.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/xml_entities.rb"),
    ),
    (
        "lich/lib/common/markup.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/markup.rb"),
    ),
    (
        "lich/lib/common/script.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/script.rb"),
    ),
    (
        "lich/lib/common/script_death.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/script_death.rb"),
    ),
    (
        "lich/lib/common/script_execution_guard.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/script_execution_guard.rb"),
    ),
    (
        "lich/lib/common/move.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/move.rb"),
    ),
    (
        "lich/lib/stash.rb",
        include_str!("../../../../bridges/ruby/lich/lib/stash.rb"),
    ),
    (
        "lich/lib/global_defs.rb",
        include_str!("../../../../bridges/ruby/lich/lib/global_defs.rb"),
    ),
    (
        "lich/lib/common/client_commands.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/client_commands.rb"),
    ),
    (
        "lich/lib/common/client_commands/builtins.rb",
        include_str!("../../../../bridges/ruby/lich/lib/common/client_commands/builtins.rb"),
    ),
];

/// The file a runner starts from, under the folder [`unpack`] writes.
pub const ENTRY: &str = "hydra/runner.rb";

/// Write the runner's files under `dir`, each only where it differs.
///
/// # Errors
///
/// A file could not be written.
pub fn unpack(dir: &Path) -> std::io::Result<()> {
    for (path, text) in FILES.iter().chain(GAME_FILES) {
        let target = dir.join(path);
        if std::fs::read_to_string(&target).is_ok_and(|kept| kept == *text) {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, text)?;
    }
    Ok(())
}

/// What a runner is told as it starts (`SCRIPTS.md`, Connecting).
#[derive(Clone, Debug)]
pub struct Start<'a> {
    /// The Ruby to run it with.
    pub ruby: &'a Path,
    /// The folder [`unpack`] wrote its files to.
    pub dir: &'a Path,
    /// The scripts listener: `http://127.0.0.1:<port>/mcp`.
    pub url: &'a str,
    /// The token [`super::Runners::admit`] gave it.
    pub token: &'a str,
    /// Its character.
    pub character: &'a str,
    /// The game's login code: `GS3`, `GSX`...
    pub game: &'a str,
    /// The folder the player's scripts are in.
    pub scripts: &'a Path,
    /// The folder the runner keeps its own data in.
    pub data: &'a Path,
    /// The character's command symbol.
    pub symbol: char,
    /// Whether it may open Lich's windows: load the gtk3 gem, when the
    /// player has it, as Lich does (`HYDRA_WINDOWS`).
    pub windows: bool,
}

/// Start a runner. It lives until killed or dropped; its standard error is
/// piped for whoever started it to keep, and it reads and writes nothing
/// else of Hydra's. Its environment is Hydra's less any password
/// ([`crate::child`]), and what it is told here.
///
/// # Errors
///
/// The process could not be started.
pub fn start(start: &Start<'_>) -> std::io::Result<tokio::process::Child> {
    crate::child::command(start.ruby)
        .arg(start.dir.join(ENTRY))
        .env("HYDRA_URL", start.url)
        .env("HYDRA_TOKEN", start.token)
        .env("HYDRA_CHARACTER", start.character)
        .env("HYDRA_GAME", start.game)
        .env("HYDRA_SCRIPTS", start.scripts)
        .env("HYDRA_DATA", start.data)
        .env("HYDRA_SYMBOL", start.symbol.to_string())
        .env("HYDRA_WINDOWS", if start.windows { "1" } else { "0" })
        .current_dir(start.data)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
}

/// The player's Ruby: `ruby` on the `PATH`, else the newest under Lich's
/// Windows install. `None` when there is neither.
#[must_use]
pub fn find_ruby() -> Option<PathBuf> {
    let name = if cfg!(windows) { "ruby.exe" } else { "ruby" };
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    });
    on_path.or_else(|| lich_install(Path::new(r"C:\Ruby4Lich5"), name))
}

/// `<root>\<version>\bin\<name>`, the highest version first.
fn lich_install(root: &Path, name: &str) -> Option<PathBuf> {
    let mut versions: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|version| version.join("bin").join(name).is_file())
        .collect();
    versions.sort_by_key(|version| {
        version
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n.split('.')
                    .map(|part| part.parse::<u32>().unwrap_or(0))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    versions.pop().map(|version| version.join("bin").join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file of the runner (`bridges/ruby/hydra`, `bridges/ruby/lich`)
    /// travels in the binary, and nothing that is not there: a file added to
    /// the runner and left out of [`FILES`] would be missing only from the
    /// runner Hydra writes out.
    #[test]
    fn every_file_of_the_bridge_is_carried() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bridges/ruby");
        let mut on_disk = Vec::new();
        let mut folders = vec![root.join("hydra"), root.join("lich")];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                } else {
                    let relative = path.strip_prefix(&root).unwrap();
                    on_disk.push(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        on_disk.sort();
        let mut carried: Vec<String> = FILES
            .iter()
            .chain(GAME_FILES)
            .map(|(path, _)| (*path).to_owned())
            .collect();
        carried.sort();
        assert_eq!(carried, on_disk);
    }

    #[test]
    fn the_newest_lich_ruby_is_found() {
        let root = std::env::temp_dir().join(format!("cena-ruby-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for version in ["3.4.1", "4.0.3", "4.0.10"] {
            let bin = root.join(version).join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            std::fs::write(bin.join("ruby.exe"), "").unwrap();
        }
        std::fs::create_dir_all(root.join("R4LInstall")).unwrap();
        assert_eq!(
            lich_install(&root, "ruby.exe"),
            Some(root.join("4.0.10").join("bin").join("ruby.exe"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
