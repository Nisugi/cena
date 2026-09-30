//! `plan/45` Stage 4: Wrayth's highlights, names and ignores imported into
//! the triggers file. The fixture is cut from the author's own export
//! (`tests/fixtures/wrayth.xml`, its first line says how).

use cena_behavior::triggers::edit::{self, Merged};
use cena_behavior::triggers::wrayth::{self, IGNORES, Import, NAMES, STRINGS};
use cena_behavior::triggers::{self, Loaded};
use cena_session::trigger::{Color, Matcher, Pattern, Span, Trigger};
use cena_session::{ChunkLine, Line};

const FIXTURE: &str = include_str!("fixtures/wrayth.xml");
const ORIGIN: &str = "Wrayth: Nisugi3.xml";

/// The fixture imported into `file`: the new file's text, and what the
/// import did.
fn imported(file: &str, xml: &str) -> Option<(String, Merged, Import)> {
    let import = wrayth::read(xml, ORIGIN).ok()?;
    let (text, merged) = edit::import(file, ORIGIN, &import).ok()?;
    Some((text, merged, import))
}

fn loaded(text: &str) -> Option<Loaded> {
    triggers::read(text).ok()
}

fn mine(text: &str) -> Vec<Trigger> {
    loaded(text).map_or_else(Vec::new, |loaded| loaded.triggers.for_character("Nisugi"))
}

fn named<'a>(triggers: &'a [Trigger], name: &str) -> Option<&'a Trigger> {
    triggers.iter().find(|trigger| trigger.name == name)
}

fn colour(hex: u32) -> Color {
    let [_, red, green, blue] = hex.to_be_bytes();
    Color { red, green, blue }
}

#[test]
fn every_highlight_name_and_ignore_in_the_fixture_comes_in() {
    let (text, merged, import) = imported("", FIXTURE).unwrap();
    assert_eq!(import.counts, [8, 3, 2], "strings, names, ignores");
    assert_eq!(import.sounds, 2);
    assert_eq!(import.notes, Vec::<String>::new());
    assert_eq!(import.ignores_on, Some(true), "disable='n'");
    assert_eq!(
        merged,
        Merged::default(),
        "nothing replaced, renamed or refused"
    );
    let loaded = loaded(&text).unwrap();
    assert!(loaded.refused.is_empty(), "{:?}", loaded.refused);
    let all = loaded.triggers.for_character("Nisugi");
    assert_eq!(all.len(), 13);

    let gsiv = &named(&all, "GSIV").unwrap().rule;
    assert_eq!(gsiv.category, STRINGS);
    assert_eq!(
        gsiv.pattern,
        Some(Pattern::Literal {
            text: "GSIV".into(),
            whole_word: true,
        })
    );
    assert!(
        !gsiv.case_sensitive,
        "Wrayth ignores case unless case=\"y\""
    );
    let look = gsiv.look.as_ref().unwrap();
    assert_eq!(
        (look.color, look.background),
        (Some(colour(0xff_8080)), None)
    );
    assert_eq!(look.span, Span::Match);

    let lnet = named(&all, "[LNet]-").unwrap().rule.look.as_ref().unwrap();
    assert_eq!(lnet.span, Span::Line, "line=\"y\"");

    // Entities are characters again.
    let bigshot = &named(&all, "[bigshot]>").unwrap().rule;
    assert_eq!(
        bigshot.look.as_ref().unwrap().background,
        Some(colour(0x36_2804))
    );
    assert!(named(&all, "Striking with a serpent's unsettling quickness").is_some());

    let exact = &named(
        &all,
        "[bigshot: Trying to load a profile that does not exist.]",
    )
    .unwrap()
    .rule;
    assert!(exact.case_sensitive, "case=\"y\"");

    // `@N` is the palette's colour.
    let tendril = named(&all, "A dark shadowy tendril rises up from")
        .unwrap()
        .rule
        .look
        .as_ref()
        .unwrap();
    assert_eq!(
        (tendril.color, tendril.background),
        (Some(colour(0xff_ff00)), Some(colour(0x33_6699)))
    );

    let maravel = &named(&all, "Maravel").unwrap().rule;
    assert_eq!(maravel.category, NAMES);
    assert!(maravel.case_sensitive);
    let orsen = named(&all, "Orsen").unwrap().rule.look.as_ref().unwrap();
    assert_eq!(orsen.color, Some(colour(0xff_ff90)));

    let erratic = &named(&all, "is moving too erratically for that.")
        .unwrap()
        .rule;
    assert_eq!(erratic.category, IGNORES);
    assert!(erratic.squelch);
    assert!(erratic.look.is_none());

    // The sound is the path Wrayth wrote, and each trigger says where it
    // came from.
    let private = edit::show(&text, "[Private]").unwrap();
    assert!(
        private.contains(&r"sound = 'C:\fx\data.wav'".to_owned()),
        "{private:?}"
    );
    assert_eq!(
        named(&all, "[Private]").unwrap().rule.sound.as_deref(),
        Some(r"C:\fx\data.wav")
    );
    assert!(
        private.contains(&format!("origin = \"{ORIGIN}\"")),
        "{private:?}"
    );
}

#[test]
fn an_imported_highlight_paints_the_line_it_names() {
    let (text, _, _) = imported("", FIXTURE).unwrap();
    let matcher = Matcher::new(mine(&text)).unwrap();
    let said = Line::new("", ChunkLine::plain("[LNet]-[Private]-Dicate: \"hi\"").runs);
    let shown = matcher.respond(&said);
    let [line] = shown.as_slice() else {
        panic!("{shown:?}");
    };
    // `[LNet]-` colours the whole line; `[Private]`'s colour sits on it.
    let text = line.text();
    let spans: Vec<(String, Option<Color>)> = line
        .paint
        .iter()
        .map(|paint| (text[paint.span.clone()].to_owned(), paint.color))
        .collect();
    assert_eq!(
        spans,
        [
            ("[LNet]-".to_owned(), Some(colour(0xff_8000))),
            ("[Private]".to_owned(), Some(colour(0xec_c013))),
            ("-Dicate: \"hi\"".to_owned(), Some(colour(0xff_8000))),
        ]
    );
    let ignored = Line::new(
        "",
        ChunkLine::plain("The ghost is moving too erratically for that.").runs,
    );
    assert!(
        matcher.respond(&ignored).is_empty(),
        "an ignore is a squelch"
    );
}

#[test]
fn importing_the_same_file_again_replaces_what_it_brought() {
    let (once, _, _) = imported("", FIXTURE).unwrap();
    let (twice, merged, _) = imported(&once, FIXTURE).unwrap();
    assert_eq!(merged.replaced, 13);
    assert!(merged.renamed.is_empty());
    assert_eq!(mine(&twice).len(), 13);
    assert_eq!(once, twice);
}

#[test]
fn a_name_the_player_already_uses_is_left_to_the_player() {
    let file = "[trigger.GSIV]\ntext = 'the realm news'\nsquelch = true\n";
    let (text, merged, _) = imported(file, FIXTURE).unwrap();
    assert_eq!(
        merged.renamed,
        [("GSIV".to_owned(), "GSIV (Wrayth)".to_owned())]
    );
    let all = mine(&text);
    assert!(
        named(&all, "GSIV").unwrap().rule.squelch,
        "the player's own, untouched"
    );
    assert!(named(&all, "GSIV (Wrayth)").unwrap().rule.look.is_some());
    // And a second import does not touch it either.
    let (again, merged, _) = imported(&text, FIXTURE).unwrap();
    assert_eq!(merged.replaced, 13);
    assert_eq!(again, text);
}

#[test]
fn disable_switches_the_ignores_off() {
    let xml = FIXTURE.replace("<ignores disable='n'>", "<ignores disable='y'>");
    let (text, _, import) = imported("", &xml).unwrap();
    assert_eq!(import.ignores_on, Some(false));
    let all = mine(&text);
    assert!(all.iter().all(|trigger| trigger.rule.category != IGNORES));
    assert_eq!(all.len(), 11, "the ignores are there, and off");
    assert!(text.contains("\"Wrayth ignores\" = false"), "{text}");
}

#[test]
fn what_cannot_be_carried_is_named_and_the_rest_comes_in() {
    let xml = r##"<settings>
        <stringsbox><h text="not a section"/></stringsbox>
        <strings>
            <h color="skin" bgcolor="#102030" text="background only"/>
            <h color='#ffffff' text="Nature's > bless &amp; &#62; b"/>
            <h color="@999" text="no such palette entry"/>
            <h color="red" text="a named colour"/>
            <h color="" text="nothing but a sound" sound="C:\fx\ding.wav"/>
            <h color="#ffffff" text=""/>
            <h color="#FFFFFF" text="twice"/>
            <h color="#ffffff" text="twice"/>
        </strings>
        <ignores disable="n"/>
    </settings>"##;
    let import = wrayth::read(xml, ORIGIN).unwrap();
    assert_eq!(
        import.notes,
        [
            "`no such palette entry`: its colour @999 is not in the file's palette, and is left out",
            "`no such palette entry` has no colour Hydra can show, and is left out",
            "`a named colour`: its colour `red` is not #rrggbb, and is left out",
            "`a named colour` has no colour Hydra can show, and is left out",
            "an entry in Wrayth strings has no words, and is left out",
        ]
    );
    let names: Vec<&str> = import
        .triggers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    // A quote of the other kind, and a `>`, inside a value are the value's;
    // and a sound with no colour is a trigger that only sounds.
    assert_eq!(
        names,
        [
            "background only",
            "Nature's > bless & > b",
            "nothing but a sound",
            "twice",
            "twice (2)"
        ]
    );
    assert_eq!(
        import.ignores_on,
        Some(true),
        "a section that closes itself"
    );
    assert_eq!(import.counts, [5, 0, 0]);
    assert_eq!(import.sounds, 1);
}

#[test]
fn a_file_with_no_highlights_is_not_a_wrayth_file() {
    let refused = wrayth::read("<settings client=\"1.0\"><palette/></settings>", ORIGIN);
    assert!(refused.unwrap_err().contains("not a Wrayth settings file"));
}

/// An entry a player commented out stays out, and a comment naming a
/// section does not open it.
#[test]
fn a_commented_out_entry_does_not_come_in() {
    let xml = "<!-- a note on <names> --><settings><strings>\
               <h color=\"#ff0000\" text=\"kept\"/>\
               <!-- <h color=\"#ff0000\" text=\"dropped\"/> -->\
               </strings></settings>";
    let import = wrayth::read(xml, ORIGIN).unwrap();
    let names: Vec<&str> = import
        .triggers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, ["kept"]);
    assert_eq!(import.counts, [1, 0, 0]);
    // A comment never closed runs to the end, `<names>` and all.
    let unclosed = "<settings><strings></strings>\
                    <!-- <names><h color=\"#ff0000\" text=\"x\"/></names>";
    let unclosed = wrayth::read(unclosed, ORIGIN).unwrap();
    assert!(unclosed.triggers.is_empty());
}

/// The presets, for a theme (`plan/57` step 8): each colour resolved
/// through the palette, `skin` left out, what cannot be read noted.
#[test]
fn the_presets_are_read_through_the_palette() {
    let xml = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/wrayth.xml"
    ))
    .expect("the fixture");
    let (presets, notes) = cena_behavior::triggers::wrayth::presets(&xml);
    // roomName's colour is `skin`, left out; bold's is @6, which the fixture's
    // palette does not have.
    assert!(presets.is_empty(), "{presets:?}");
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].contains("bold") && notes[0].contains("@6"),
        "{notes:?}"
    );
    let resolved = cena_behavior::triggers::wrayth::presets(
        r##"<settings><palette><i id="6" color="#FF3300"/></palette>
        <presets><p id="bold" color="@6"/><p id="speech" color="#F0EEE8"/>
        <p id="thought" color="green"/></presets></settings>"##,
    );
    assert_eq!(resolved.0.get("bold").map(String::as_str), Some("#ff3300"));
    assert_eq!(
        resolved.0.get("speech").map(String::as_str),
        Some("#f0eee8")
    );
    assert_eq!(resolved.1.len(), 1, "{:?}", resolved.1);
}
