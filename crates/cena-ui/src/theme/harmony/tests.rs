//! The generator's promises, `VellumFE`'s tests carried over and widened to
//! the whole palette: deterministic and complete, every colour clears the
//! floor on a dark and on a light background, tokens in a group stay apart,
//! a pin survives, an anchored token keeps its hue under every scheme.

use super::*;
use crate::theme::oklch::hue_distance;
use crate::theme::parse_hex;

fn hex(text: &str) -> Rgb {
    parse_hex(text).unwrap()
}

/// Nord's red accent on its background.
fn recipe() -> Recipe {
    Recipe {
        seed: hex("#bf616a"),
        background: hex("#2e3440"),
        ..Recipe::default()
    }
}

/// The tokens the contrast floor is for: what is drawn on a surface.
fn on_a_surface(token: Token) -> bool {
    matches!(
        token.role(),
        Role::Free { .. } | Role::Anchored { .. } | Role::Onto { .. }
    )
}

#[test]
fn generate_is_deterministic_and_complete() {
    let r = recipe();
    let a = generate(&r);
    let b = generate(&r);
    assert_eq!(a, b);
    for token in Token::ALL {
        if on_a_surface(token) {
            assert_ne!(
                a.get(token),
                token.bare(),
                "{} was not generated",
                token.name()
            );
        }
    }
}

#[test]
fn every_colour_clears_the_floor_on_a_dark_and_a_light_background() {
    for background in ["#2e3440", "#0d1115", "#f4f1ea", "#ffffff"] {
        for scheme in Scheme::ALL {
            let r = Recipe {
                background: hex(background),
                scheme,
                ..recipe()
            };
            let palette = generate(&r);
            for token in Token::ALL.into_iter().filter(|t| on_a_surface(*t)) {
                let cr = contrast(palette.get(token), r.background);
                assert!(
                    cr >= r.contrast - 0.35,
                    "{} = {:?} only {cr:.2}:1 against {background} ({})",
                    token.name(),
                    palette.get(token),
                    scheme.name()
                );
            }
        }
    }
}

#[test]
fn tokens_in_a_group_are_mutually_distinct() {
    let palette = generate(&recipe());
    let drawn: Vec<Token> = Token::ALL
        .into_iter()
        .filter(|t| on_a_surface(*t))
        .collect();
    for (i, a) in drawn.iter().enumerate() {
        for b in drawn.iter().skip(i + 1).filter(|b| b.group() == a.group()) {
            assert!(
                delta_e(palette.get(*a), palette.get(*b)) > 0.015,
                "{} and {} are nearly identical",
                a.name(),
                b.name()
            );
        }
    }
}

#[test]
fn a_pinned_token_survives_generation() {
    let mut r = recipe();
    r.pins.insert(Token::Speech, hex("#12ab34"));
    let palette = generate(&r);
    assert_eq!(palette.get(Token::Speech), hex("#12ab34"));
}

#[test]
fn an_anchored_token_keeps_its_hue_under_every_scheme_and_seed() {
    for scheme in Scheme::ALL {
        for seed in ["#50fa7b", "#6a9fb5", "#c09eff"] {
            let r = Recipe {
                seed: hex(seed),
                scheme,
                ..recipe()
            };
            let palette = generate(&r);
            for token in Token::ALL {
                let Role::Anchored { .. } = token.role() else {
                    continue;
                };
                let anchor = to_lch(token.bare());
                let [_, chroma, hue] = to_lch(palette.get(token));
                // A grey has no hue to keep; only a colour that visibly
                // carries one is held to its band.
                if anchor[1] >= 0.04 && chroma >= 0.04 {
                    assert!(
                        hue_distance(hue, anchor[2]) < 25.0,
                        "{} hue {hue:.0} left its band round {:.0} ({} seed {seed})",
                        token.name(),
                        anchor[2],
                        scheme.name()
                    );
                }
            }
        }
    }
}

#[test]
fn health_is_still_red_and_mana_still_blue() {
    let palette = generate(&Recipe {
        seed: hex("#50fa7b"),
        scheme: Scheme::Golden,
        ..recipe()
    });
    let hue = |token| to_lch(palette.get(token))[2];
    assert!(
        hue_distance(hue(Token::Health), 25.0) < 25.0,
        "{}",
        hue(Token::Health)
    );
    assert!(
        hue_distance(hue(Token::Mana), 258.0) < 25.0,
        "{}",
        hue(Token::Mana)
    );
}

#[test]
fn the_room_plate_hits_the_requested_spread() {
    for spread in [2.5, 7.0] {
        let r = Recipe {
            room_spread: spread,
            ..recipe()
        };
        let palette = generate(&r);
        let cr = contrast(palette.get(Token::RoomName), palette.get(Token::RoomPlate));
        assert!(
            (cr - spread).abs() < 0.6,
            "the room's name on its plate is {cr:.2}:1, wanted about {spread}"
        );
    }
}

#[test]
fn a_surface_follows_the_background_and_a_fixed_token_stays() {
    let r = recipe();
    let palette = generate(&r);
    let bg = to_lch(r.background);
    let map = to_lch(palette.get(Token::MapBackground));
    assert!(
        hue_distance(map[2], bg[2]) < 5.0 || map[1] < 0.02,
        "{map:?} vs {bg:?}"
    );
    assert_eq!(palette.get(Token::Veil), Token::Veil.bare());
    assert_eq!(palette.get(Token::Grid), Token::Grid.bare());
}

#[test]
fn hue_variants_are_distinct_and_readable() {
    let r = recipe();
    let variants = hue_variants(Token::Speech, &r);
    assert!(variants.len() >= 5, "want a real choice: {variants:?}");
    for v in &variants {
        assert!(
            contrast(*v, r.background) >= r.contrast - 0.35,
            "variant {v:?} unreadable"
        );
    }
    assert!(
        hue_variants(Token::Health, &r).is_empty(),
        "anchored: no hues to choose"
    );
}

#[test]
fn seed_swatches_drop_greys_the_background_and_duplicates() {
    let bg = hex("#2e3440");
    let raw = [
        hex("#808080"), // grey: dropped (chroma)
        hex("#2f3541"), // near the background: dropped
        hex("#bf616a"), // vivid: kept
        hex("#bf616b"), // a perceptual duplicate of the one above: dropped
        hex("#88c0d0"), // kept
        hex("#a3be8c"), // kept
    ];
    let out = seed_swatches(&raw, bg, 12);
    assert_eq!(out.len(), 3, "{out:?}");
    for kept in ["#bf616a", "#88c0d0", "#a3be8c"] {
        assert!(out.contains(&hex(kept)), "{kept} dropped");
    }
    // The cap, and the most vivid first.
    let capped = seed_swatches(&raw, bg, 2);
    assert_eq!(capped.len(), 2);
    assert!(
        to_lch(capped[0])[1] >= to_lch(capped[1])[1],
        "sorted by chroma"
    );
    // An all-grey set still offers its greys.
    assert_eq!(seed_swatches(&[hex("#808080")], bg, 3), [hex("#808080")]);
}

#[test]
fn scheme_names_round_trip() {
    for scheme in Scheme::ALL {
        assert_eq!(Scheme::parse(scheme.name()), Some(scheme));
        assert_eq!(Scheme::parse(&scheme.name().to_uppercase()), Some(scheme));
        assert!(!scheme.description().is_empty());
    }
    assert_eq!(Scheme::parse("cubist"), None);
}
