# 60. Aliases: a word typed for a line, as Lich's `alias.lic`

**Status: PROPOSED 2026-09-30.** The author: *"we need to plan implementing the
functionality of the lich script alias"*. Questions for the author in §6.

An alias is a word (or a few) the player types that Hydra replaces with a line before
anything else sees it: `ls` for `look`, `zap` for `;eq cast(901, "\?")`, with the rest of
what was typed carried into the replacement. Lich has it as a script, `alias.lic`
(`reference/scripts/scripts/alias.lic`, 520 lines, version 1.0.5 of 2026-06-30). Hydra
has nothing like it: a typed line goes to a script runner's input hooks, then Hydra's
command line, then Lich or the game (`crates/cena-session/src/command/round_trip.rs:192-309`),
and no step rewrites it from the player's own table.

## 1. What `alias.lic` does, measured

Read whole. Everything below cites it.

- **Two tables, each `(trigger, target)` with the trigger unique**: `global`, and one per
  character named `<game>_<name>` (`:61-64`), in `alias.db3` in Lich's data folder.
- **A trigger matches the start of a typed line, case-insensitively, as a whole word**: the
  regex is `^(?:<c>)?(trigger|trigger|...)(?:\s+|$)(.*)` (`:77`, `:87`), the triggers
  escaped, so a trigger may be several words, or begin with `;` (the help's own example,
  `;alias add --global ;code = ;lnet chat on code`, `:448`). What follows the trigger is
  `extra`.
- **The character's table is tried before the global one** (`:455`, `:479`).
- **The target is cut at `\r` into lines** (`:458`), the two characters, not a carriage
  return, and each line is handed to Lich's client path as if typed (`:474`), so a target
  may be a Lich command, a game command, or another alias.
- **`\?` is where the rest of the line goes** (`:459-473`):
  - nothing typed after the trigger: every `\?` is removed;
  - something typed, and some line of the target has `\?`: it replaces every `\?`; in a
    line that is `;e...` with `"\?"` in quotes, it goes in as a Ruby string literal
    (`extra.inspect`, `:463-464`), so `;eq cast(901, "\?")` typed as `zap kobold` runs
    `cast(901, "kobold")`;
  - something typed, no `\?` anywhere, **one** line: the rest is appended with a space
    (`:471-472`);
  - something typed, no `\?`, **several** lines: the rest is dropped. Not documented; it
    is what the code does.
- **One level of expansion.** A target that is itself an alias is not expanded again
  (`caller.count { |x| x =~ /do_client/ } < 2`, `:454`; changelog 0.2, *"don't crash from
  recursive aliases"*).
- **The commands** (`:360-452`): `;alias add|set [--global] <trigger> = <target>` (updates
  in place and says the old target); `;alias remove|rem|delete|del [--global] <trigger>`;
  `;alias list` (global, then the character's, sorted); `;alias reload`; `;alias stop`;
  `;alias setup`, a Gtk window; anything else, the usage.
- **It is an `UpstreamHook`** (`:515`), so it sees every line the client sends, including
  what a macro key sends, and runs before Lich's own command parsing: `;alias` itself is
  caught inside the hook (`:95`).

**The author barely uses it.** MEASURED (sqlite, both of this machine's Lich data folders):
`C:\Gemstone\lich-5\data\alias.db3` has 8 tables and **0 rows**; `E:\Gemstone\dev\lich-5\data\alias.db3`
has 10 tables and **1 row**, global: `stunblind` → `;en stun_blind DownstreamHook.add(...)`.
No `\r`, no `\?`, no multi-word trigger in either. So the design below is for the feature
as Lich's players know it, not tuned to the author's table; and an import from
`alias.db3` is cheap but, for the author, carries one row.

**Under the Ruby bridge today**, `alias.lic` does not run (`inventory/13-ruby-bridge-evidence.md:482`: *alias* among the
scripts in neither tier). It would need `Script.open_file('db3')` and sees Gtk; and
the thing it does, rewriting typed lines, is what Hydra's own `UpstreamHook` path exists
for (`plan/46` step 4). Building aliases into Hydra removes the reason to run it.

## 2. Where it goes in Hydra

**In the session, on the typed path, before Hydra's command line.** The reasons:

- Every frontend types through `SessionHandle::send_typed_at` (`round_trip.rs:192`): the
  play window (`cena-gui/src/sessions.rs:559`), Despana (`cena-web/src/socket.rs:158`), a
  key macro's commands (`cena-gui/src/app/keyed.rs`, which sends them as typed), and the
  Lich relay's tests. One place, and aliases work headless and in the browser without
  three implementations.
- Lich's order is hooks, then commands; a trigger may begin with Lich's symbol. Hydra's
  equivalent is: the runner's input hooks (a script may still rewrite the line first, as
  under Lich), **then aliases**, then the claimant (`Desk::claim`), then Lich or the game.
  So `.hunt` can be aliased to `.hunt ojandhaart` and `zap` to `;eq ...`, as under Lich.
- The expansion's lines go back through the same path **once** (Lich's one level): each
  may be a Hydra command, Lich's, or the game's. A line that an alias produced is marked
  so it is not expanded again; nothing else about it differs from a typed line, so a
  character's Lich still has it with `typed`.

**Not** in the GUI's macro cutter, though the shape rhymes: `plan/52`'s `Macro::Send` cuts
at `\r` with `sN` waits (`cena-gui/src/keys/binding.rs:6-12`). An alias target is cut the
same way for the player's sake, one spelling of a multi-command string across Hydra, but
the cutting function moves down to `cena-ui` (`validate_line` is already there) or
`cena-session` so both use it. Rule of two with a third in sight (the gamepad's binds,
`plan/56`, are `keys::Macro`s already), so moving it is the DRY call, not an abstraction.

What the expansion needs to know, and does not today: whether a line was typed by a
person (it has `typed`), the character's aliases and everyone's (a table set on the
handle, as the command symbol is: `SessionHandle::set_command_symbol`, `handle.rs:313`),
and nothing of the game.

## 3. The shape

- **An alias is a trigger and a target.** The trigger is one or more words, matched at the
  start of the line as a whole word, case-insensitively, exactly Lich's rule, so an
  imported table behaves as it did. The target is text with `\r` between lines and `\?`
  for the rest of what was typed.
- **`\?` and the rest, Lich's four cases**, with one change to put to the author (§6 item
  2): several lines and no `\?` **appends the rest to the last line** rather than dropping
  it. Lich drops it silently, which no player can want; but it is a difference an imported
  alias could feel.
- **The `;e "\?"` quoting** (§6 item 1): kept, narrowed to a line starting with Lich's
  symbol and `e`, `eq`, `en`, `exec`, `execq`, `execname`, since it is the help's headline
  example and the author's one alias is an `;en`.
- **The character's, then everyone's**, as Lich tries them, and as the keys files layer
  (`plan/52` step 2): `aliases.toml` for everyone and `<instance>_<name>.aliases.toml`
  beside the character's settings, through `cena_session::store::character_path` as
  `keys.rs:409` makes the keys file's name. TOML, not sqlite: the player can read and
  edit it, as every other Hydra file.

  ```toml
  # aliases.toml -- everyone's
  ls = "look"
  zap = ';eq cast(901, "\?")'
  "sell all" = "sell gems\rsell boxes\rs1\rsell skins"
  ```

  A multi-word trigger is a quoted key. A single-quoted TOML string keeps `\?` and `\r` as
  the two characters, as the keys file keeps `\r` (`plan/52` §2, where it is the character
  itself; here it must stay the pair, because `\?` has no character form, so the file says
  so in a comment the writer keeps).
- **A change reaches the running character at once**, as the triggers' changes do
  (`plan/45` Stage 6): the command edits the file and sets the new table on every handle
  it applies to. No `reload`, no `stop`: the files are read when a character starts and
  rewritten by the command, and `;alias` is always there.
- **One level**, Lich's. A target line that is itself an alias goes as written.
- **Nothing is echoed but what was typed.** The story shows `zap kobold`, as Lich's client
  shows it; the expansion is in the log as what was sent (`Origin::Manual`), as today.
- **Shown where settings are shown.** An *Aliases* page in the settings menu, the
  character's own over everyone's with the same *global* switch the Keys page has
  (`cena-gui/src/keys/page.rs:14-20`), listing trigger, target and where it came from.
  `;alias` and the page write the same file through one writer, `plan/50` §6's rule.

## 4. The commands

`;alias` on Hydra's command line (`crate::commands`, a handler like `;theme`'s), on the
reference page `crate::command_reference`:

| Command | Does |
|---|---|
| `alias add <trigger> = <target>` | this character's; `set` the same. An existing trigger is updated and the old target said, as Lich says it |
| `alias add --global <trigger> = <target>` | everyone's; `--global` before or after `add`, as Lich takes it (`:360`) |
| `alias remove [--global] <trigger>` | `rem`, `delete`, `del` too |
| `alias list` | everyone's, then this character's, sorted, with `(none)` |
| `alias import <Lich folder>` | its `data/alias.db3`: the global table into everyone's, `<game>_<name>` into that character's own, where the game and name are a roster character's; a trigger already present kept, and said |
| `alias`, `alias help` | the table |

`;alias setup` is the Aliases page; `reload` and `stop` have no counterpart and are said
so if typed.

## 5. Steps

0. **The cutter moves down.** `Macro::Send`'s `\r` and `sN` cutting (`keys/binding.rs`)
   to `cena-session` (or `cena-ui`), the keys tests going with it; the GUI calls it.
   No behaviour change. Its own commit.
1. **The table and the expansion, pure** (`cena-session/src/command/alias.rs`): `Aliases`
   (trigger → target, the character's and everyone's), `expand(line) -> Option<Vec<String>>`
   with Lich's matching and the four `\?` cases, the `;e` quoting, tested against the
   cases in §1 and §3 including the one Hydra changes.
2. **On the typed path**: `send_typed_at` asks the hooks, then expands, then each line
   through `manual_at` as typed, once. A test with a scripted game: `ls` reaches the game
   as `look`; `zap kobold` with Lich attached reaches Lich as `;eq cast(901, "kobold")`;
   an alias to a Hydra command runs it; an alias whose target is an alias goes as
   written; a key macro's command is expanded.
3. **The files** (`cena-session` or the binary): read at start for everyone and for the
   character, written by one writer that keeps comments as the keys writer does
   (`keys/write.rs`), the handle told on each change.
4. **`;alias`** in the binary: the words of §4 but `import`; the help; on the reference
   page and in `HELP` (the test on `command_reference.rs` holds them together).
5. **`;alias import`**: `alias.db3` read with `rusqlite` (already a dependency of the
   binary), the tables mapped to the roster.
6. **The Aliases page** in Settings, beside Keys, over the same writer; the character's
   page with *global*.

Steps 0-2 are the feature; 3-4 make it the player's; 5-6 are conveniences. A branch,
`aliases`, since it is a planned feature (the author, 2026-09-28: fixes on main, features
on a branch).

## 6. For the author

**ANSWERED 2026-09-30**, the same day: 1 *"keep it"*; 2 *"sure append"*; 3 *"we can
try"*; 6 *"allow"*. Item 4 was asked again in plain words (a key bound to `zap kobold`
expanding as typing does); item 5 waits on the author, taken as *later* until then.

1. **Lich's `;e "\?"` quoting**: keep it (a line starting with Lich's symbol and an exec
   word, `"\?"` becoming a Ruby string literal), or drop it and let `\?` be plain text
   everywhere? Recommendation: keep; it is the help's first example.
2. **Several lines and no `\?`**: Lich drops what was typed after the trigger. Append it to
   the last line instead (recommended), or keep Lich's behaviour for a faithful import?
3. **Import from `alias.db3`**: wanted at all, given one row on this machine? It is cheap
   (step 5) and other players' tables are fuller.
4. **Expansion of what a key macro sends** (Lich: yes, since macros go through the client
   path). Recommendation: yes, one rule for a typed line wherever it came from. A `;to`
   relayed line runs on the other character's command line and is not typed there, so it
   is not expanded; say if it should be.
5. **The Aliases page now or later**: step 6 can wait for a round of settings pages.
6. **A trigger that is a game verb** (`l` for `look` is fine; `look` for something else
   shadows the game's `look` for that character). Lich allows it. Allow it, with `list`
   showing it plainly? Recommendation: allow; it is the player's table.
