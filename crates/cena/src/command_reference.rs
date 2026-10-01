//! Every command a player types to Hydra, family by family: the word, its
//! arguments, what it does, and the module that answers it.
//!
//! It is rustdoc for the reason [`glossary`](crate::glossary) is: each family
//! links to the module that runs it, and the workspace denies broken
//! intra-doc links, so a family removed from the code turns this page red
//! instead of leaving it describing a command that has gone. `;help` in the
//! client ([`HELP`](crate::commands::HELP)) is the short form of this page,
//! and a test in this file keeps its families and the ones here the same set.
//!
//! What each command *does* in depth is in the plan that built it, cited per
//! family. This page is the syntax.
//!
//! # How a line is read
//!
//! A command starts with the character's **command symbol**: `.` unless the
//! character's settings say otherwise (the *General* page, or the `symbol`
//! key of the `commands` section of its settings file; a symbol that is not
//! one character falls back to `.`). Leading spaces before the symbol are
//! allowed and nothing else is: `  .go2 bank` is a command, `say .go2 bank`
//! is speech. The symbol alone is nothing. A word no family knows is answered
//! *I do not know .word* and **never reaches the game**; nothing on this page
//! does.
//!
//! The families hear a line in this order
//! ([`Commands::route`](crate::commands::Commands::route)): `help` and
//! `stop` first; then the starters, travel (with `room`), hunt (with `heal`,
//! `waggle`, `keep`, `sc`), the batches (`multi`, `foreach`) and the relay
//! (`to`, `all`); then loot, combat, history, doll, theme, sorter, targetid,
//! trigger, agent and lich; and the player's own Lich scripts last of all, so
//! a word of Hydra's is never a script's. Travel, hunt, `agent` and `lich`
//! register after login: a word of theirs typed before that is answered
//! *still starting; nothing was sent*.
//!
//! Two lines are read without the symbol. A bare spell number or alias is
//! cast as `sc` would (see *sc*; `sc set typed off` stops it). And with a
//! character's own Lich on, a line starting with `;` is Lich's own command
//! and the rest of what is typed goes to Lich (see *lich*).
//!
//! Every family answers `<word> help` with its own table, and most answer the
//! bare word the same way.
//!
//! # help, stop
//!
//! [`commands`](crate::commands).
//!
//! | Command | Does |
//! |---|---|
//! | `help`, `?` | the families, one line each, with the word that says more |
//! | `stop` | stop everything Hydra is doing on this character: a hunt, a walk (`go2`), a `multi` or `foreach`; answers *Stopped: …* or *Nothing was running*. The play window's Stop button sends it |
//!
//! # hunt
//!
//! [`hunt`](crate::hunt) over [`cena_behavior::hunt::command`]; `plan/30`,
//! `plan/39` (groups). Every hunt word needs a map; without one it says so and
//! sends nothing.
//!
//! | Command | Does |
//! |---|---|
//! | `hunt <profile>` | hunt on a profile |
//! | `hunt <profile> quick` \| `bounty` | clear this room; hunt until the bounty is done |
//! | `hunt <profile> with <name> <name>…` | lead these characters, each hunting its own `<profile>` |
//! | `hunt stop` | stop the hunt, and a heal, keep or waggle under way |
//! | `hunt list` | the profiles there are |
//! | `hunt check <profile>` | read it as this character will run it: what is wrong, what is held |
//! | `hunt show <profile> [setting]` | every setting, or those under one: `hunt show ojandhaart rest` |
//! | `hunt set <profile> <setting> <value>` | change one: `hunt set ojandhaart rooms.resting 29877`. The value is everything after the setting, as typed: `on`/`off`, a number, a list `["a", "b"]`, a table `{ name = "warg", routine = "a" }`, or words. A list entry is picked by number from 1: `targets.2.routine` |
//! | `hunt unset <profile> <setting>` | take one out, so the default decides it |
//! | `hunt import <bigshot yaml> [as <name>]` | bring in a bigshot profile; everything before `as` is the path |
//! | `hunt import-loot <eloot yaml>` | eloot's settings as this character's loot profile |
//! | `hunt setup` | where the map's setup page is |
//! | `hunt`, `hunt help` | the table above |
//!
//! `import`, `import-loot`, `check`, `list`, `stop`, `set`, `unset`, `show`,
//! `help` and `setup` can never be a profile's name.
//!
//! # heal
//!
//! [`hunt`](crate::hunt) over [`cena_behavior::heal`]; `plan/36`.
//!
//! | Command | Does |
//! |---|---|
//! | `heal [spellcast] [ranged] [blood]` | heal with herbs: everything, or only what stops a cast, a shot, or the blood (eherbs' `--spellcast` spelling is taken too) |
//! | `heal show` | the heal settings, the defaults included |
//! | `heal set <setting> <value>` | change one: `heal set container herb pouch` |
//! | `heal unset <setting>` | back to its default |
//! | `heal stock` \| `fill` | stock the herb container at the herbalist; buy one of each herb it lacks |
//! | `heal help` | the table |
//!
//! A hunt heals at every rest once a container is set. `hunt stop` stops a
//! heal under way.
//!
//! # waggle
//!
//! [`hunt`](crate::hunt) over [`cena_behavior::waggle`]; `plan/37` Stage 5.
//!
//! | Command | Does |
//! |---|---|
//! | `waggle [name] [name]…` | cast the waggle spells on these people, or yourself |
//! | `waggle show`, `waggle set <setting> <value>`, `waggle unset <setting>` | its settings, as heal's: `waggle set cast_list [101, 107, 401]` |
//! | `waggle help` | the table |
//!
//! # keep
//!
//! [`hunt`](crate::hunt) over [`cena_behavior::keep`]; `plan/37` Stage 4.
//!
//! | Command | Does |
//! |---|---|
//! | `keep` | keep the listed spells up until `hunt stop` |
//! | `keep list` | the spells kept, the no-cast rooms, and whether Sigil of Power is on |
//! | `keep add <spell>` | add one, by number or name (a name may be several words) |
//! | `keep del <spell>` (`delete`, `remove`, `rem`) | take one out |
//! | `keep nocast add <room>` \| `del <room>` \| `clear` | rooms nothing is cast in |
//! | `keep power` | toggle Sigil of Power when 25 mana short |
//!
//! # sc
//!
//! [`hunt`](crate::hunt) over [`cena_behavior::spellcaster`]; `plan/37`
//! Stage 6. A cast goes beside a running hunt, never in its place. The
//! *Spellcaster* page in Settings has every setting.
//!
//! | Command | Does |
//! |---|---|
//! | `sc <spell\|alias> [target] [count]` | cast it: `sc 401`, `sc 903 kobold`, `sc 111 3` |
//! | `sc alias <spell> <name>`, `sc alias clear <name>` | call a spell by a name of yours: `sc alias 211 bravery` |
//! | `sc verb <spell> <verb>`, `sc verb <spell> clear` | cast it with `cast` (or `incant`), `channel` or `evoke` |
//! | `sc stance <spell> <stance>`, `sc stance <spell> clear` | take this stance to cast it: `offensive`, `advance`, `forward`, `neutral`, `guarded`, `defensive` |
//! | `sc set typed on\|off` | cast a bare `401` or alias typed with no `sc` (on by default) |
//! | `sc set conserve\|safety\|channel\|stance on\|off` | keep mana; need a target; channel attacks; take the stance |
//! | `sc`, `sc help` | the table |
//!
//! # loot
//!
//! [`hunt`](crate::hunt) over [`cena_behavior::loot`] and
//! [`cena_behavior::town`]; `plan/31`, `plan/61`. Each is eloot's command of
//! the same name, run by the character's loot profile, with no hunt around
//! it; a hunt loots and sells by the same settings. `stop` ends one. The
//! reports under the same word are in *loot, combat* below.
//!
//! | Command | Does |
//! |---|---|
//! | `loot` | skin and search the dead here, take what the floor holds |
//! | `loot skin` | only skin the dead here, whatever `skin.enable` says |
//! | `loot box` | empty the open box in hand, then keep it or throw it out, in the pool's bin when there is none here; a reliquary is always kept |
//! | `loot ground` | the same for each box on the ground, the hands put away first and given back after; a locked box is left where it lay |
//! | `loot sell` | the selling round: the locksmith pool, the shops, the bank, and back |
//! | `loot sell type <kinds>` | the round for only these kinds, as the object table names them, between commas or a space apart: `loot sell type gem, skin`. The pool too for `type box` |
//! | `loot sell shop <shops>` | the round at only these shops: `gemshop`, `pawnshop`, `furrier`, `collectibles`, `chronomage`; no pool |
//! | `loot sell item <names>` | the round for only things whose names hold one of these, between commas: `loot sell item blue crystal, silver wand`; no pool. Each of the three sells only what the profile sells |
//! | `loot pool` | the locksmith pool alone: give it the boxes carried, collect what is ready, then bank, keeping the silver carried before |
//! | `loot pool deposit` | only give boxes, then bank as `loot pool` does |
//! | `loot pool return` (`check`, `loot`) | only collect them; no bank |
//! | `loot deposit` | the bank alone, keeping the silver the profile says |
//! | `loot last` | what the last selling round came to: silver by shop, the pool's boxes less its tips and fees, the bank, and what was appraised and kept. Said at the end of every round, a hunt's too |
//! | `loot reset unskinnable [creature]` | forget every creature learned unskinnable, or the one named, in any case |
//! | `loot show`, `loot set <setting> <value>`, `loot unset <setting>` | the loot profile, skinning and selling with it: `loot set skin.enable on`, `loot set town.sell_keep_silver 5000` |
//! | `loot help` | the table |
//!
//! # go2, route2, room
//!
//! [`travel`](crate::travel) over [`cena_behavior::travel`];
//! `plan/21`, `plan/30`. Every word needs a map (`CENA_MAP`, or the one Hydra
//! ships).
//!
//! | Command | Does |
//! |---|---|
//! | `go2 <place>` | walk to the nearest one: `go2 bank` |
//! | `go2 <room>` | to a room, by the map's number or the game's: `go2 228`, `go2 u7120` |
//! | `go2 targets` | the places there are to go |
//! | `go2 list` | the names you have saved |
//! | `go2 save <name>` | this room is `<name>`, for this character; `--global` anywhere in the line, for every character |
//! | `go2 save <name>=228,current` | these rooms (numbers, `u<uid>`, `current`); the nearest is meant |
//! | `go2 delete <name>` | forget it; `--global` for everyone's |
//! | `go2 stop`, `kill go2`, `k go2` | stop walking |
//! | `route2 <place>` | show the way, and send nothing |
//! | `room <number> [number]` | the map's room: number, uid, title, location, description and paths; with `number`, only `#id title`. The minimap's Shift+click and Ctrl+click send these |
//! | `go2 help` | the table |
//!
//! `targets`, `list`, `help` and `stop` are words only when alone: `go2 list
//! of kings` walks there.
//!
//! # multi, foreach
//!
//! [`batch`](crate::batch) over [`cena_behavior::batch`]; `plan/30` §7 M6e.
//! An entry starting with the command symbol is a Hydra command, run and
//! waited for.
//!
//! | Command | Does |
//! |---|---|
//! | `multi <times>,<command>,<command>,…` | the commands, in order, that many times: `multi 6,shake my jar,sell diamond`. The count may come last; with no comma the list splits on `;`; zero and a nested `multi` are refused |
//! | `multi stop` | stop it |
//! | `foreach [options] [[attr=]value] in\|on\|under\|behind <target>[,<target>…][; command; command…]` | for each matching item: `foreach gem in cloak; get item; appraise item; put item in container`. With no commands, list what matches |
//! | `foreach stop` | stop it |
//! | `multi`, `foreach`, and each with `help` | the tables |
//!
//! foreach's parts, as its help says them. **attr**: `type` (the default),
//! `sellable`, `noun`, `name`, `fullname`, `quick` (`t s n m f q`); `*` is a
//! wildcard, `a,b` matches either, `/pattern/` a regular expression,
//! `type=none` is untyped; `all`, `any`, `everything` is no filter.
//! **options**: `unique`, `first N` (or `N`), `after N` (or `skip N`),
//! `sorted`, `nsorted`, `reversed`, each once. **targets**: a container,
//! `floor` (`ground`, `room`), `loot`; a trailing `?` skips it when missing.
//! **in commands**: `item` (`#id`), `noun`, `name`, `container` (`#id`); the
//! separator may be `;`, `/` or `|`. **conveniences**: `move [to] <where>`,
//! `return`, `waitrt`, `waitcastrt`, `sleep N`, `echo X`, `waitfor X`,
//! `waitre /X/`, `waitmana N` (`hp`, `spirit`, `stamina`), `unmark`,
//! `;<hydra command>`.
//!
//! # loot, combat
//!
//! [`loot`](crate::loot) and [`combat`](crate::combat), reports over the
//! character's own database; `plan/34` Stages 3 and 4. Each records only once
//! turned on (Settings, *Recording*).
//!
//! | Command | Does |
//! |---|---|
//! | `loot summary [today\|month\|<hours>]` | what was looted, the last 24 hours unless said (`midnight`, `monthly` also) |
//! | `loot recent [<n>] [<type>]` | the last 20, or `<n>`, of everything or one type (`gem`, `box`, …; plurals taken) |
//! | `loot boxes [<n>]` | the last 10 boxes |
//! | `loot creatures [<n>] [today\|month\|<hours>]` | the 10 best creatures |
//! | `loot cap [last\|<YYYY-MM>]` | this month's cap (`lootcap`; `previous`, `prev`) |
//! | `combat [<id>]` | the latest hunt, or one by id |
//! | `combat hunts [<n>]` (`sessions`) | the last 10 hunts |
//! | `combat all`, `combat last <n>` | over every hunt, or the last 10 |
//! | `combat abilities [<id>\|all\|last <n>]` (`ability`) | what was used |
//! | `combat attacks [<n>]` (`attack`) | the last 15 attacks |
//!
//! # history
//!
//! [`history`](crate::history), the player log read back; `plan/25`. Not
//! `;log`, so that `log.lic` stays reachable.
//!
//! | Command | Does |
//! |---|---|
//! | `history`, `history days` | the days kept |
//! | `history tail [<n>]` | the last lines (20) |
//! | `history last <span>` | a span back from now: `90s`, `15m`, `2h`, `1d`; a bare number is minutes |
//! | `history day <day> [<from> [<to>]]` | a day (`YYYY-MM-DD`, `today`, `yesterday`), or from `HH:MM` to `HH:MM` of it |
//! | `history search <text>` | lines holding the text, any case, newest first; `/<regex>/` for an expression |
//! | `history export <day> [<day>]` | those days, one to the other, written to one text file |
//! | `history help` | the table |
//!
//! Add `in:<stream>[,<stream>]` to any: `in:thoughts`, `in:combat`, `in:cmd`.
//!
//! # sorter, targetid
//!
//! [`sorter`](crate::sorter) and [`targetid`](crate::targetid), each kept in
//! the character's settings file; `plan/30` §7 M6e, `plan/44`.
//!
//! | Command | Does |
//! |---|---|
//! | `sorter [on\|off\|status]` | a container's contents one line per category; bare, it toggles |
//! | `targetid [on\|off\|status]` | a tag after each creature's name, so `tk <tag>` and the script's other commands work; bare, it toggles |
//! | `targetid slot <1-3> <unique\|random\|none\|<mark>>` | what each of the tag's three slots shows; a mark is one character such as `-` or `/` |
//! | `targetid health <off\|front\|back>` | where the creature's health goes in the tag (`none` is `off`) |
//!
//! # doll
//!
//! [`doll`](crate::doll); `plan/55`.
//!
//! | Command | Does |
//! |---|---|
//! | `doll import <folder>` | your `VellumFE` dolls into Hydra's dolls folder, each picture's calibration written into it; `<folder>` is `VellumFE`'s own or its dolls folder. Pick a picture on the Injuries widget's page |
//! | `doll face <degrees>` | turn the Infinite doll: `0` faces you, `90` turns its front to the right, `-90` to the left, `180` away |
//! | `doll`, `doll help` | the table |
//!
//! # theme
//!
//! [`theme_command`](crate::theme_command); `plan/57`. The *Theme* page in a
//! character's settings, and the Theme window, do the same.
//!
//! | Command | Does |
//! |---|---|
//! | `theme list` | every theme, and the one Hydra wears (chosen on the *Window* page) |
//! | `theme mine <name>` \| `off` | this character's own theme, for its window |
//! | `theme accent <#rrggbb>` \| `off` | its own accent colour |
//! | `theme import <file> [as <name>]` | a Wrayth settings file's presets as a theme |
//! | `theme`, `theme help` | the table |
//!
//! # trigger
//!
//! [`triggers`](crate::triggers) over the one triggers file; `plan/45`,
//! `plan/54`. `triggers` is the same word. A name is one word or a `"quoted
//! phrase"`.
//!
//! | Command | Does |
//! |---|---|
//! | `trigger list` | every trigger, by category |
//! | `trigger show <name>` | one trigger's settings |
//! | `trigger add <name> <words>` | a new trigger on those words, making them bold |
//! | `trigger set <name> <setting> <value>` | `look.color #ff4040`, `squelch on`, `substitute <text>`, `redirect.stream <stream>`, … |
//! | `trigger unset <name> <setting>` | take one out |
//! | `trigger remove <name>` | delete it |
//! | `trigger on\|off <name>` | switch one trigger |
//! | `trigger on\|off category <category>` | every trigger in a category |
//! | `trigger on\|off every <look\|squelch\|substitute\|redirect>` | one kind of response, everywhere |
//! | `trigger test <line>` | what this character's triggers would do to that line |
//! | `trigger reload` | read the file again, for every character |
//! | `trigger import <path>` | a Wrayth settings file's highlights, names and ignores, or another player's triggers `.toml` (its commands asked about first) |
//! | `trigger approve <name>` | let a trigger that came from elsewhere send its line |
//! | `trigger`, `trigger help` | the table |
//!
//! # agent
//!
//! [`agent`](crate::agent); `plan/35`. An agent is a program such as Claude
//! Code, connected to the listener Hydra starts with `--agent`.
//!
//! | Command | Does |
//! |---|---|
//! | `agent`, `agent level` | this character's agent level, and what an agent is waiting on you for |
//! | `agent level <level>` | set it, kept for this character: `off` (the default), `observe`, `advise`, `behaviors`, `commands` or `takeover`, each allowing what the one before does and more |
//! | `agent approve <n>`, `agent deny <n>` | let an agent do the one thing it asked, once; or refuse it (`#n` is taken too) |
//! | `agent stop` | stop everything the agent is doing, and take the character back if it holds it |
//! | `agent help` | the table |
//!
//! # lich
//!
//! [`lich`](crate::lich); `plan/51`. With Lich on, a line you type goes to
//! Lich, and one starting with `;` is Lich's own command; give Hydra another
//! symbol, such as `.`, to reach Hydra's. The play window's Lich switch sends
//! `on` and `off`.
//!
//! | Command | Does |
//! |---|---|
//! | `lich` | whether this character's own Lich runs, and where Lich is |
//! | `lich on` | start it, and start it whenever this character starts |
//! | `lich off` | stop it, and leave it off |
//! | `lich folder <folder>` | where your Lich is: the folder with `lich.rbw` in it, for every character |
//! | `lich help` | the table |
//!
//! # to, all
//!
//! [`relay`](crate::relay); `plan/47` step 3. The line runs on the other
//! character's own command line first, so `to Baelor .go2 bank` works.
//!
//! | Command | Does |
//! |---|---|
//! | `to <name> <command>` | send it on another character: the name in full or a unique prefix, `GAME:Name` when one name is on two games |
//! | `all <command>` | on every character |
//! | `all -A,B <command>` | on everyone but A and B |
//! | `all +A,B <command>` | on only A and B |
//!
//! A name that matches nobody refuses the whole line.
//!
//! # keys
//!
//! [`cena_gui::App`], in a play window only: a headless character does not
//! know it; `plan/52` step 9.
//!
//! | Command | Does |
//! |---|---|
//! | `keys import <file>` | a Wrayth settings file's key sets into this character's keys |
//! | `keys import global <file>` | into every character's |
//!
//! # scripts
//!
//! [`scripts`](crate::scripts), the player's own Lich scripts in the
//! character's script runner; `plan/46`. Heard last of all.
//!
//! | Command | Does |
//! |---|---|
//! | `<script> [args]` | run one of your scripts (`.lic`, `.rb`, `.lic.gz`, `.rb.gz`), from `scripts/custom/` first, by exact name then by prefix |
//! | `k`, `kill`, `stop <script>`, `ka`, `killall`, `stopall` | stop one, or all, as in Lich (`kill go2` is travel's; bare `stop` is Hydra's) |
//! | `p`, `pause`, `pa`, `pauseall`, `u`, `unpause`, `ua`, `unpauseall` | pause and resume, as in Lich |
//! | `l`, `la`, `list`, `listall` | what runs, as in Lich |
//! | `force`, `send`, `s`, `e`, `eq`, `exec`, `execq`, `en`, `execname` | Lich's own, passed to the runner |
//! | `scripts` | where the scripts and their settings are, and this table |
//! | `scripts import <Lich folder>` | bring your Lich scripts and their settings here: `C:\Lich5` |
//! | `scripts check <script>` | which of its lines will not work under Hydra, and why |

/// The families this page has a section for, by the first word of each
/// [`HELP`](crate::commands::HELP) line. The test below holds the two lists
/// to the same set, so a family added to one is added to the other.
#[cfg(test)]
const FAMILIES: &[&str] = &[
    "hunt", "heal", "waggle", "keep", "sc", "trigger", "go2", "loot", "combat", "history",
    "sorter", "targetid", "doll", "theme", "multi", "agent", "lich", "stop", "keys", "to",
    "<script>",
];

#[cfg(test)]
mod tests {
    use super::FAMILIES;
    use crate::commands::HELP;

    /// Every family `;help` names has a section here, and the other way
    /// round. `HELP`'s first line is prose; the rest start with the word.
    #[test]
    fn every_help_family_is_on_this_page() {
        let named: Vec<&str> = HELP[1..]
            .iter()
            .filter_map(|line| line.split([' ', ',']).next())
            .collect();
        for word in &named {
            assert!(
                FAMILIES.contains(word),
                "`;help` names `{word}`; the reference page has no section for it"
            );
        }
        for word in FAMILIES {
            assert!(
                named.contains(word),
                "the reference page has `{word}`; `;help` does not name it"
            );
        }
    }
}
