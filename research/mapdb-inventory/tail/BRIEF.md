# Porting brief: one slice of the long tail

You are porting upstream Ruby map scripts into data for **Hydra** (working name `cena`), a
Rust game client with **no scripting language**. An offline converter reads the Lich map
database and turns each scripted exit into a list of primitive steps, or a gated cost. You
are writing the converter's *recogniser arms* for one slice of what is still unported.

## Read these first, in full

- `crates/cena-mapdb-convert/src/recognise.rs` — the entry points and the helpers
  (`holes`, `quoted`, `is_plain_argument`, `is_word`, `always`). Read the module docs:
  **an arm is a template, not a parser.**
- `crates/cena-mapdb-convert/src/recognise/moves.rs`, `costs.rs`, `tests.rs` — every arm so
  far. Yours should read like these.
- `crates/cena-map/src/step.rs`, `cond.rs`, `exit.rs` — the vocabulary you may emit.

## Your file, and only your file

`crates/cena-mapdb-convert/src/recognise/tail_<letter>.rs`. It is already wired in: its
`crossing(script, from, to)` (or `cost(script)`) is called after every existing arm. Put
your arms there as private functions, chain them with `.or_else`, and put your tests at the
foot of the same file in `#[cfg(test)] mod tests`. You may `use super::{holes, quoted,
is_plain_argument, is_word, always}`, `super::moves::{cast_if_able, quoted_list}` and
`super::costs::gated`.

**Edit nothing else.** Not `recognise.rs`, not another slice's file, not the baseline, not
the plan. Others are porting other slices at the same moment.

## Your worklist

`research/mapdb-inventory/tail/slice_<letter>.tsv`: `edges`, `shape_id`, `sample`
(`from->to`), and the *normalised* shape (strings → `S`, numbers → `N`, regexes → `R`).
Work **most exits first**.

The normalised shape is not the script. Get the real text from the upstream file — it is
outside the worktree, at the absolute path
`E:\Cena\reference\mapdb\map-1789942730.json` (a JSON list of rooms; a crossing is
`room["wayto"][to]`, a cost is `room["timeto"][to]`, both strings starting `;e`). For each
shape, write a throwaway Python script **in your scratch directory** that prints *every*
script sharing the sample's skeleton, so you see how the holes vary (quote style, spacing,
trailing `;`) before you write the template. Never commit anything from `reference/`.

## The rules of an arm

1. **Exact or nothing.** The template is the upstream text verbatim with holes. No regex,
   no "contains", no tolerance. A script one character different does not match, and that is
   the intended failure.
2. **Validate every hole**: `is_plain_argument` for a quoted command, `is_word` for an
   identifier, `parse()` with a range for a number. A hole must not be able to swallow a
   second statement.
3. **The vocabulary is frozen.** You may emit only what exists today:
   - `Action::{Move, Put, Await, Cast, Pause, EmptyHands, FillHands, Remember, Forget}`
   - `Cond::{All, Any, Not, Otherwise, Setting, SettingIsSet, Flag, Remembered, Profession,
     Month, EncumbranceOver, SkillUnder, SpellActive, SpellKnown, SpellAffordable}`
   - `Cost::Gated { when, then, otherwise }` and `Cost::Fixed`
   If a shape needs anything else — a loop, branching on what the game replied, a random
   choice, an item or NPC check, a level or race or society test, arithmetic — **do not
   approximate it and do not add to the vocabulary. Skip it** and list it in your report
   with exactly what it would need. A skipped shape costs nothing; a wrong one strands a
   character.
4. **Mean what upstream means.**
   - `Move(cmd)`: a command that **changes rooms** (the exit's destination is another room),
     however upstream sent it — `move`, `fput`, `put`, `dothistimeout`. It already includes
     waiting out roundtime, standing up, retrying and opening a closed door, so a trailing
     `waitrt?` adds nothing.
   - `Put(cmd)`: a command sent where the walker stands, that does not change rooms.
   - `Pause(ms)`: `sleep`/`pause` in whole milliseconds.
   - `echo` / `respond` / `_respond` are dropped: they talk to a Lich user.
   - Spells are named, not numbered. Check names in `crates/cena-model/data/spells.tsv`.
5. **Two kinds of check (the author's rule).** A condition that can make an exit unusable
   belongs in the **cost** and is asked while planning. A step's `when` is asked in the room
   and may only change *how* the exit is crossed — never leave the walker with no move.
   `cena_map::moves_whatever_is_known(steps)` must hold for every step list you emit; use
   `Cond::Otherwise` for the second of two ways across. If a crossing script *itself*
   refuses the walker (`else; echo 'you need X'; end`), skip it and say so.
6. **Unknown answers no.** Do not port upstream's `!defined?(X) or` escape hatches as
   "allowed when unknown".
7. Every arm gets a test: the real upstream text in, the exact steps out, and at least one
   near-miss that must **not** match.

## House rules (the build enforces these)

- No `unwrap`, `expect` or `panic!` outside `#[test]` functions — helpers in test modules
  included; return `Option`.
- `cargo clippy` runs pedantic with `-D warnings`. No file over 800 lines: **if yours passes
  ~700, stop and report.**
- Match the surrounding code's comment style: say *why*, cite the count of exits, keep it
  short.
- On this machine Bash heredocs mangle backslashes. **Write Rust with the Write/Edit tools,
  never `cat <<EOF`.**
- Do not commit, do not push, do not switch branches, do not run the `cena` binary.

## Checking your work

```
cargo fmt -p cena-mapdb-convert
cargo test -p cena-mapdb-convert
cargo clippy -p cena-mapdb-convert --all-targets -- -D warnings
CENA_MAPDB="E:/Cena/reference/mapdb/map-1789942730.json" cargo test --release -p cena-mapdb-convert --test ratchet -- --nocapture
```

The ratchet **will fail once you have ported something** — "That is progress — turn the
baseline down to N". That is expected: **do not edit the baseline.** Report the N it prints
for `crossings` (or `costs`). It must *never* fail with "no move" or with `reachable` going
down; if it does, your arm is wrong.

## Your report

- Each shape ported: `shape_id`, exits, the arm's function name, and one line on what it
  became.
- Each shape skipped: `shape_id`, exits, and precisely what it needs.
- The unported count the ratchet printed, before and after.
- **Anything you were unsure about the meaning of** — especially whether a command changes
  rooms. Say so plainly; the arms are reviewed against the Ruby before they are merged, and a
  doubt you name is cheap while one you hide is not.
