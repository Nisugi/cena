//! `;foreach` (`plan/30` §7, M6e), pure: foreach.lic's command line parsed,
//! the items picked, and one item's lines made. The driver's half, against
//! a scripted game, is `batch_foreach_run.rs`.

use cena_behavior::batch::foreach::{Attr, Filter, Foreach, Options, Position, Sort, Target};
use cena_behavior::batch::pick::{Candidate, Group, Looked, Place, filtered, looked, select};
use cena_behavior::batch::{self, Command, Job, Kind, Line, Pool, build};
use cena_session::{ChunkLine, Link, LinkKind, RoomItem};

fn parsed(line: &str) -> Result<Command, String> {
    batch::parse(line, ';').ok_or_else(|| format!("{line:?} was not a batch"))?
}

fn foreach(line: &str) -> Result<Foreach, String> {
    match parsed(line)? {
        Command::Run(Job::Foreach(foreach)) => Ok(foreach),
        other => Err(format!("{line:?} parsed as {other:?}")),
    }
}

fn named(name: &str) -> Target {
    Target::Named {
        name: name.to_owned(),
        optional: false,
    }
}

/// An item as the game lists one: `a <link>`.
fn item(id: &str, noun: &str, name: &str) -> Candidate {
    Candidate::of(&RoomItem {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: name.to_owned(),
        before: Some("a".to_owned()),
        after: None,
        status: None,
    })
}

fn ids(groups: &[Group]) -> Vec<&str> {
    groups
        .iter()
        .flat_map(|group| group.items.iter().map(|item| item.id.as_str()))
        .collect()
}

// ---------------------------------------------------------------------------
// The words
// ---------------------------------------------------------------------------

/// foreach.lic's own first example that needs no inventory (`:6`).
#[test]
fn a_type_in_a_container_with_commands() {
    let run =
        foreach("foreach gem in cloak; get item; appraise item; put item in container").unwrap();
    assert!(
        matches!(&run.filter, Filter::Pattern { attr: Attr::Type, pattern } if pattern.is_match("gem"))
    );
    assert_eq!(run.position, Position::In);
    assert_eq!(run.targets, [named("cloak")]);
    assert_eq!(
        run.commands,
        ["get item", "appraise item", "put item in container"]
    );
    assert_eq!(run.options, Options::default());
}

#[test]
fn the_options_before_the_filter() {
    let run =
        foreach("foreach unique first 3 after 1 sorted reversed noun=sapphire in bag, red sack?")
            .unwrap();
    assert_eq!(
        run.options,
        Options {
            unique: true,
            first: Some(3),
            after: Some(1),
            sort: Sort::Name,
            reversed: true,
        }
    );
    assert!(matches!(
        &run.filter,
        Filter::Pattern {
            attr: Attr::Noun,
            ..
        }
    ));
    assert_eq!(
        run.targets,
        [
            named("bag"),
            Target::Named {
                name: "red sack".to_owned(),
                optional: true
            }
        ]
    );
    assert!(run.commands.is_empty(), "no commands lists the items");
    // A bare number is `first`; `skip` is `after`; `nsorted` sorts by noun.
    let run = foreach("foreach 5 skip 2 nsorted gem in bag").unwrap();
    assert_eq!((run.options.first, run.options.after), (Some(5), Some(2)));
    assert_eq!(run.options.sort, Sort::Noun);
    let twice = parsed("foreach unique unique gem in bag").unwrap_err();
    assert!(twice.contains("more than once"), "{twice}");
    let twice = parsed("foreach sorted nsorted gem in bag").unwrap_err();
    assert!(twice.contains("more than once"), "{twice}");
}

/// `in`, `on`, `under`, `behind`; the filter optional; the ground and `loot`.
#[test]
fn positions_and_targets() {
    let run = foreach("foreach on shelf").unwrap();
    assert_eq!(run.position, Position::On);
    assert!(matches!(run.filter, Filter::All));
    assert_eq!(run.targets, [named("shelf")]);
    assert_eq!(
        foreach("foreach box under bed").unwrap().position,
        Position::Under
    );
    assert_eq!(
        foreach("foreach all in floor, loot, ground")
            .unwrap()
            .targets,
        [Target::Ground, Target::Loot]
    );
    // The first position word is the one: the rest of the line is the target.
    assert_eq!(
        foreach("foreach gem in backpack on table").unwrap().targets,
        [named("backpack on table")]
    );
    assert!(parsed("foreach gem backpack; sell item").is_err(), "no in");
}

/// The separator is the first `;`, `/` or `|` after the target (`:1546`).
#[test]
fn any_of_three_separators() {
    for line in [
        "foreach gem in bag; get item; sell item",
        "foreach gem in bag / get item / sell item",
        "foreach gem in bag|get item|sell item",
    ] {
        assert_eq!(
            foreach(line).unwrap().commands,
            ["get item", "sell item"],
            "{line}"
        );
    }
    // A `/pattern/` before the target is not a separator.
    let run = foreach("foreach q=/red|blue/ in bag; look item").unwrap();
    assert_eq!(run.targets, [named("bag")]);
    assert_eq!(run.commands, ["look item"]);
}

/// Split on the symbol, a Hydra command leaves an empty piece before it,
/// and foreach.lic reads that as "the next is a script" (`:1673-1680`).
#[test]
fn a_hydra_command_among_the_commands() {
    let run =
        foreach("foreach gem in backpack; get item; ;sc 704 item; put item in container").unwrap();
    assert_eq!(
        run.commands,
        ["get item", ";sc 704 item", "put item in container"]
    );
    let run = foreach("foreach gem in backpack | get item | ;sc 704 item").unwrap();
    assert_eq!(run.commands, ["get item", ";sc 704 item"]);
    let nested = parsed("foreach gem in bag; ;foreach box in sack").unwrap_err();
    assert!(nested.contains("inside this one"), "{nested}");
}

/// foreach.lic's rewrites (`:1693-1760`): verbs from their first letters, a
/// bare verb on the item, the implicit `get` before a first `sell`, and the
/// implicit `return` after a lone `appraise`.
#[test]
fn commands_are_rewritten_as_foreach_rewrites_them() {
    let commands = |line: &str| foreach(line).unwrap().commands;
    assert_eq!(
        commands("foreach gem in bag; sel"),
        ["get item", "sell item"]
    );
    assert_eq!(
        commands("foreach gem in bag; ap"),
        ["get item", "appraise item", "return"]
    );
    assert_eq!(
        commands("foreach gem in bag; appraise; put item in sack"),
        ["get item", "appraise item", "put item in sack"],
        "not alone, so no return"
    );
    assert_eq!(
        commands("foreach gem in bag; get item; sell"),
        ["get item", "sell item"],
        "not first, so no implicit get"
    );
    assert_eq!(commands("foreach gem in bag; dr"), ["_drag item drop"]);
    assert_eq!(commands("foreach gem in bag; l"), ["look item"]);
    assert_eq!(commands("foreach gem in bag; loo item"), ["look item"]);
    assert_eq!(
        commands("foreach gem in bag; g item"),
        ["g item"],
        "`ge` is the least"
    );
    assert_eq!(
        commands("foreach gem in bag; !sell item"),
        ["get item", "sell item"]
    );
    assert_eq!(
        commands("foreach gem in bag; sell item;"),
        ["get item", "sell item"]
    );
}

#[test]
fn filters_and_what_they_take() {
    let filter = |line: &str| foreach(line).unwrap().filter;
    let feather = item("1", "feather", "nacreous disir feather");
    let orb = item("2", "orb", "shimmering green orb");
    let whiskey = item("3", "whiskey", "tumbler of black cherry whiskey");
    let takes = |f: &Filter| {
        [&feather, &orb, &whiskey]
            .iter()
            .filter(|i| f.matches(i))
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(takes(&filter("foreach noun=orb in bag")), ["2"]);
    assert_eq!(takes(&filter("foreach n=feather,orb in bag")), ["1", "2"]);
    // `name` and `fullname` match whole; `quick` anywhere.
    assert!(takes(&filter("foreach name=disir in bag")).is_empty());
    assert_eq!(takes(&filter("foreach name=*disir* in bag")), ["1"]);
    assert_eq!(takes(&filter("foreach q=disir in bag")), ["1"]);
    assert_eq!(
        takes(&filter("foreach fullname=a shimmering* in bag")),
        ["2"]
    );
    assert_eq!(
        takes(&filter("foreach q=/(green|cherry)/ in bag")),
        ["2", "3"]
    );
    // In any case.
    assert_eq!(takes(&filter("foreach NOUN=ORB in bag")), ["2"]);
    // Types, from the gameobj table.
    assert_eq!(takes(&filter("foreach uncommon in bag")), ["1"]);
    assert_eq!(takes(&filter("foreach type=none in bag")), ["3"]);
    assert_eq!(takes(&filter("foreach sellable=gemshop in bag")), ["2"]);
    assert_eq!(takes(&filter("foreach all in bag")), ["1", "2", "3"]);
}

/// `:1645` tests `all|any|everything` unanchored, so `small` meant all;
/// here it is a type, and there is none.
#[test]
fn a_type_nobody_has_is_refused_with_a_hint() {
    let why = parsed("foreach small in bag; sell item").unwrap_err();
    assert!(why.contains("no item type matches 'small'"), "{why}");
    assert!(why.contains("noun=small"), "{why}");
    let why = parsed("foreach quartz orb in bag").unwrap_err();
    assert!(why.contains("name=quartz orb"), "{why}");
    let why = parsed("foreach colour=red in bag").unwrap_err();
    assert!(why.contains("not an attribute"), "{why}");
    let why = parsed("foreach q=/(/ in bag").unwrap_err();
    assert!(why.contains("not a pattern"), "{why}");
}

/// What is not built is refused by name, never ignored.
#[test]
fn what_is_not_built_is_refused_by_name() {
    for (line, says) in [
        ("foreach marked gem in bag", "'marked' is not built"),
        (
            "foreach unregistered gem in bag",
            "'unregistered' is not built",
        ),
        ("foreach box in inv; move to locker", "`inv` is not built"),
        ("foreach box in qinv", "`qinv` is not built"),
        ("foreach box in worn", "`worn` is not built"),
        ("foreach box in locker", "`locker` is not built"),
        ("foreach box in previous", "`previous` is not built"),
        ("foreach box in desc", "`desc` is not built"),
        ("foreach gem in bag; stash", "is not built"),
        ("foreach gem in bag; giveitem Bob", "is not built"),
        ("foreach gem in bag; get item; pause", "no pause"),
        ("foreach gem in bag; get item; locker", "is not built"),
    ] {
        let why = parsed(line).unwrap_err();
        assert!(why.contains(says), "{line}: {why}");
    }
}

#[test]
fn help_stop_and_other_words() {
    assert!(matches!(
        parsed("foreach"),
        Ok(Command::Help(Kind::Foreach))
    ));
    assert!(matches!(
        parsed("foreach help"),
        Ok(Command::Help(Kind::Foreach))
    ));
    assert!(matches!(
        parsed("foreach stop"),
        Ok(Command::Stop(Kind::Foreach))
    ));
    assert!(batch::parse("foreachx gem in bag", ';').is_none());
    let why = parsed("foreach gem").unwrap_err();
    assert!(why.contains("Usage"), "{why}");
}

// ---------------------------------------------------------------------------
// Picking (`finalize`, :609-678)
// ---------------------------------------------------------------------------

fn bag(id: &str, items: Vec<Candidate>) -> Group {
    Group {
        place: Place::Container {
            id: id.to_owned(),
            name: format!("bag {id}"),
        },
        items,
    }
}

/// Of two items with one name, `unique` keeps the one listed first, even
/// reversed: Lich filters before it reverses (`:621-646`).
#[test]
fn unique_keeps_the_first_listed_even_reversed() {
    let groups = vec![bag(
        "10",
        vec![
            item("1", "feather", "disir feather"),
            item("2", "orb", "green orb"),
            item("3", "feather", "disir feather"),
        ],
    )];
    let options = Options {
        unique: true,
        reversed: true,
        ..Options::default()
    };
    assert_eq!(ids(&select(&options, groups)), ["2", "1"]);
}

/// Sorted by full name without its article; `nsorted` by noun first.
#[test]
fn sorted_by_name_or_noun() {
    let group = || {
        vec![bag(
            "10",
            vec![
                item("1", "orb", "zircon orb"),
                item("2", "feather", "amber feather"),
                item("3", "amulet", "orange amulet"),
            ],
        )]
    };
    let sorted = |sort| {
        let options = Options {
            sort,
            ..Options::default()
        };
        ids(&select(&options, group())).join(",")
    };
    assert_eq!(sorted(Sort::Listed), "1,2,3");
    assert_eq!(sorted(Sort::Name), "2,3,1");
    assert_eq!(sorted(Sort::Noun), "3,2,1");
}

/// `after` and `first` count across the targets in order (`:648-664`).
#[test]
fn after_and_first_count_across_targets() {
    let groups = || {
        vec![
            bag("10", vec![item("1", "a", "a"), item("2", "b", "b")]),
            bag("20", vec![item("3", "c", "c"), item("4", "d", "d")]),
        ]
    };
    let picked = |after, first| {
        let options = Options {
            after,
            first,
            ..Options::default()
        };
        ids(&select(&options, groups())).join(",")
    };
    assert_eq!(picked(Some(1), Some(2)), "2,3");
    assert_eq!(picked(Some(3), None), "4");
    assert_eq!(picked(None, Some(1)), "1");
    assert_eq!(picked(Some(9), None), "");
}

/// An item listed in two targets is taken once (`add_item!`, `:229-237`).
#[test]
fn each_item_once_and_only_what_the_filter_takes() {
    let groups = vec![
        bag(
            "10",
            vec![item("1", "gem", "blue gem"), item("2", "rock", "grey rock")],
        ),
        bag(
            "20",
            vec![item("1", "gem", "blue gem"), item("3", "gem", "red gem")],
        ),
    ];
    let filter = foreach("foreach noun=gem in a, b").unwrap().filter;
    assert_eq!(ids(&filtered(&filter, groups)), ["1", "3"]);
}

// ---------------------------------------------------------------------------
// Reading a look (`INV_PATTERN`, :190). The shapes the game sends for a
// container are read off real looks in `batch_foreach_run.rs`; the answers
// that are not a container carry no markup, and are Lich's own text.
// ---------------------------------------------------------------------------

#[test]
fn a_look_that_found_no_container() {
    let said = |text: &str| looked(&[ChunkLine::plain(text)]);
    assert_eq!(said("There is nothing in the leather sack."), Looked::Empty);
    assert_eq!(said("There is nothing on the table."), Looked::Empty);
    assert_eq!(said("That is closed."), Looked::Closed);
    assert_eq!(
        said("I could not find what you were referring to."),
        Looked::NotFound
    );
    assert_eq!(said("You see nothing unusual."), Looked::Unread);
    assert_eq!(looked(&[]), Looked::Unread);
}

/// The sorted view's header (`FLAG SORTEDVIEW ON`): `In the <sack>:`.
#[test]
fn the_sorted_view_names_its_container_alone_on_a_line() {
    let template = ChunkLine::plain("").runs.runs[0].clone();
    let piece = |text: &str| {
        let mut piece = template.clone();
        piece.text = text.to_owned();
        piece
    };
    let mut sack = piece("leather sack");
    sack.link = Some(Link {
        kind: LinkKind::Exist {
            id: "77".to_owned(),
            noun: "sack".to_owned(),
        },
        text: "leather sack".to_owned(),
        coord: None,
    });
    let mut line = ChunkLine::plain("");
    line.runs.runs = vec![piece("In the "), sack, piece(":")];
    assert_eq!(
        looked(&[line]),
        Looked::Container {
            id: "77".to_owned(),
            name: "leather sack".to_owned()
        }
    );
}

// ---------------------------------------------------------------------------
// One item's lines (`:1936-2126`)
// ---------------------------------------------------------------------------

fn lines(commands: &[&str], place: &Place) -> Result<Vec<Line>, String> {
    let commands: Vec<String> = commands.iter().map(|c| (*c).to_owned()).collect();
    build::lines(
        &commands,
        place,
        &item("42", "feather", "disir feather"),
        ';',
    )
}

fn sack() -> Place {
    Place::Container {
        id: "9".to_owned(),
        name: "leather sack".to_owned(),
    }
}

fn send(text: &str) -> Line {
    Line::Send(text.to_owned())
}

/// `item`, `noun`, `name` and `container`, whole words, any case.
#[test]
fn the_words_are_filled_in() {
    assert_eq!(
        lines(&["get ITEM", "put item in container"], &sack()).unwrap(),
        [send("get #42"), send("put #42 in #9")]
    );
    assert_eq!(
        lines(&["say my noun is a name", "echo items stay"], &sack()).unwrap(),
        [
            send("say my feather is a disir feather"),
            Line::Echo("items stay".to_owned())
        ]
    );
    let why = lines(&["put item in container"], &Place::Ground).unwrap_err();
    assert!(why.contains("on the ground"), "{why}");
}

#[test]
fn the_conveniences() {
    let one = |command: &str, place: &Place| lines(&[command], place).unwrap();
    assert_eq!(
        one(";sc 704 item", &sack()),
        [Line::Hydra("sc 704 #42".to_owned())]
    );
    assert_eq!(one("move to #7", &sack()), [send("_drag #42 #7")]);
    assert_eq!(
        one("fmove item to ground", &sack()),
        [send("_drag #42 drop")]
    );
    assert_eq!(
        one("move to red sack", &sack()),
        [send("get #42"), send("put #42 in red sack")]
    );
    assert_eq!(
        one("mv my gem to floor", &sack()),
        [send("get my gem"), send("place my gem")]
    );
    assert_eq!(one("return", &sack()), [send("put #42 in #9")]);
    assert_eq!(one("return", &Place::Ground), [send("place #42")]);
    assert_eq!(one("unmark item", &sack()), [send("mark #42 remove")]);
    assert_eq!(one("waitrt?", &sack()), [Line::WaitRt]);
    assert_eq!(one("waitcastrt", &sack()), [Line::WaitCastRt]);
    assert_eq!(
        one("waitmana 30", &sack()),
        [Line::WaitVital(Pool::Mana, 30)]
    );
    assert_eq!(one("waitsp 5", &sack()), [Line::WaitVital(Pool::Spirit, 5)]);
    assert_eq!(
        one("sleep 0.5", &sack()),
        [Line::Sleep(std::time::Duration::from_millis(500))]
    );
    assert_eq!(
        one("sleep well", &sack()),
        [send("sleep well")],
        "not a number"
    );
    assert_eq!(
        one("waitfor You hand item", &sack()),
        [Line::WaitFor("You hand #42".to_owned())]
    );
    assert_eq!(
        one("waitre /accepts? your/i", &sack()),
        [Line::WaitRe("(?i)accepts? your".to_owned())]
    );
    assert!(lines(&["waitre /(/"], &sack()).is_err());
    assert_eq!(one("sell item", &sack()), [send("sell #42")]);
}
