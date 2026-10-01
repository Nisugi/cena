# The Ruby script runner

Runs one character's Lich scripts against Hydra (`plan/46`, M7b): Lich's own script engine,
in Ruby, with the edges -- the game, the player's screen, the player's `;` commands --
answered by Hydra over `hydra-script/1` (`crates/cena-agent/SCRIPTS.md`).

Hydra carries these files in its binary (`crates/cena-agent/src/scripts/runner.rs`, `FILES`),
writes them to its data folder before it starts a runner, and starts `hydra/runner.rb` with the
player's Ruby, one runner per character. Lich's engine needs Ruby 4.0.

## `hydra/`: Hydra's edge

| File | Is |
|---|---|
| `runner.rb` | the entry: the environment Hydra starts it with, then the engine, then `listen`; with windows, Gtk's loop on the main thread, as Lich's |
| `engine.rb` | Lich's engine and Hydra's edges, loaded as a runner runs them, with what Lich loads before any script (`OpenStruct`, `YAML`, `Terminal::Table`...), each loaded when a script first names it, and `HAVE_GTK` false, as a Lich started `--no-gtk` |
| `rexml.rb` | `REXML` as Lich loads it, the stream listener with it, for `engine.rb` to load when a script first names `REXML` |
| `check.rb` | the checker (`plan/46` §1): which lines of a script will not work under Hydra and why, read by Ruby's own parser against the engine as `engine.rb` loads it; `ruby check.rb [--tsv] FILE...` |
| `connection.rb` | `hydra-script/1`'s tools over MCP, one HTTP request per call |
| `edge.rb` | where Lich's engine meets the world, answered by Hydra: `Game.puts`, `respond`, `_respond`, a script's `$stdout`, `Lich.log`, `Lich::Messaging`, and a `Frontend` saying Hydra is Wrayth's family |
| `copy.rb` | the local copy of the character from Hydra's `state` events, `XMLData` answered from it by Lich's names, and Lich's `GameObj` filled from it |
| `map.rb` | `Room.current` and `Room[id]` from Hydra's map, with `wayto` and `timeto` as Lich's; `Map.dijkstra`, `path_to`, the `find_nearest` family and `estimate_time` answered by Hydra, priced as the character's walk; `Map.list`, every room, asked once and completed as read |
| `spell.rb` | `Spell[n]` from Hydra's spell table, evaluated for the character; known and up from the copy; Lich's own `cast` and the `force_` family, over the table's way to cast each |
| `infomon.rb` | Lich's `Infomon` answered from the copy's sheet: the stats, skills, circles, PSMs, society, resources, currency and experience by Lich's keys |
| `gemstone.rb` | `Lich::Util` and its commands, those that read the game's markup saying so; and the game's own classes, loaded once `XMLData` is made as Lich's game loader loads them: `Stats`, `Skills`, `Spells`, `Society`, `Experience`, `Resources`, `Currency`, the PSMs, `Effects`, `Wounds`, `Scars`, `Injured`, over `infomon.rb`; and Lich's `CharacterStatus` from `games.rb`, which the runner does not load |
| `builtins.rb` | Lich scripts Hydra has built in (`go2`), started by name as Lich's are, as an exec script named after them that waits on Hydra's run |
| `hooks.rb` | a script's display and input hooks, kept by Lich's own registries, told to Hydra as they come and go, and asked about each line the player is shown or types |
| `listener.rb` | `listen` in a loop: the copy's changes, game lines to every script and then to the display hooks, the player's commands to Lich's command table or `Script.start`, the player's typing to the input hooks; ten unanswered in a row and Hydra is taken to be gone, and the runner ends |

## `lich/`: Lich's engine, unchanged

The files under `lich/lib` are Lich 5's, **byte for byte** as upstream wrote them:
`elanthia-online/lich-5` at `236a9a2c6974339927d68c7bd07895b3f785d8e5` (2026-09-16,
`LICH_VERSION` 5.21.0). Lich is BSD 3-Clause; its license is `lich/LICENSE.txt` and goes
wherever these files go. `.gitattributes` keeps them out of line-ending conversion, so each
one's `git hash-object` equals upstream's blob at that commit:

```sh
git -C reference/lich-5 rev-parse 236a9a2:lib/common/script.rb
git hash-object bridges/ruby/lich/lib/common/script.rb
```

Only what the runner loads is here: the script engine (`common/script.rb` and the two files
it requires), the calls scripts make (`global_defs.rb` and what it requires), Lich's `;`
command table, the helpers those read at load time, Lich's extensions of Ruby's own classes that it loads before any script
(`common/class_exts/`: `5.minutes`, `90.as_time`, `StringProc`, less its client socket's), the classes a script reads its character
through (`constants.rb`, `common/gameobj.rb`, `attributes/char.rb`), the stores (`lich.rb`,
`common/settings.rb` and its folder, `common/vars.rb`, `common/uservars.rb`), and the hooks
(`common/downstreamhook.rb`, `common/upstreamhook.rb` and the `common/hook_registry.rb` they
share), the character's sheet (`attributes/stats.rb`, `skills.rb`, `spells.rb`, `resources.rb`;
`gemstone/society.rb` and its `societies/`, `experience.rb`, `psms.rb` and its `psms/`,
`currency.rb`, `effects.rb`, `injured.rb`, `wounds.rb`, `scars.rb`, and `stance.rb`, which a cast
uses; `util/util.rb` and the
`util/deep_freeze.rb` it requires), and Lich's Gtk support (`common/gtk.rb`, `util/gtk_compaction.rb`), loaded only when Hydra
lets the runner open windows (`HYDRA_WINDOWS`) and the player has the `gtk3` gem. The gems are
Lich's installer's: `ox`, `sqlite3`, `sequel`. To take a newer Lich, copy the same
files from the new commit, update the commit above, and run the runner's tests.
