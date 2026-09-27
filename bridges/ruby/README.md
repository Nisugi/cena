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
| `runner.rb` | the entry: the environment Hydra starts it with, Lich's engine loaded, the rest started |
| `connection.rb` | `hydra-script/1`'s tools over MCP, one HTTP request per call |
| `edge.rb` | where Lich's engine meets the world, answered by Hydra: `Game.puts`, `respond`, `_respond`, a script's `$stdout`, `Lich.log`, `Lich::Messaging`, and a `Frontend` saying Hydra is Wrayth's family |
| `copy.rb` | the local copy of the character from Hydra's `state` events, `XMLData` answered from it by Lich's names, and Lich's `GameObj` filled from it |
| `map.rb` | `Room.current` and `Room[id]` from Hydra's map, with `wayto` and `timeto` as Lich's |
| `listener.rb` | `listen` in a loop: the copy's changes, game lines to every script, the player's commands to Lich's command table or `Script.start` |

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
command table, the helpers those read at load time, the classes a script reads its character
through (`constants.rb`, `common/gameobj.rb`, `attributes/char.rb`), and the stores (`lich.rb`,
`common/settings.rb` and its folder, `common/vars.rb`, `common/uservars.rb`). The gems are
Lich's installer's: `ox`, `sqlite3`, `sequel`. To take a newer Lich, copy the same
files from the new commit, update the commit above, and run the runner's tests.
