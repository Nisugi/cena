//! A shop's `order` menu belongs to the room it was read in (the review of
//! 2026-09-29): kept after a move, `;heal stock` at a second herbalist ordered
//! by the first shop's numbers.

use cena_model::GameState;
use cena_protocol::Parser;

/// Fold `wire` into `state`, as the actor does.
fn fold(state: &mut GameState, parser: &mut Parser, wire: &[u8]) {
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
}

const MENU: &[u8] = b"<d cmd='order 3'>some acantha leaf</d>  <d cmd='order 7'>a sprig of wolifrew</d>\n<prompt time='1'>&gt;</prompt>\n";

#[test]
fn a_menu_is_kept_in_its_room_and_forgotten_on_a_move() {
    let (mut state, mut parser) = (GameState::default(), Parser::new());
    fold(&mut state, &mut parser, b"<nav rm='1001'/>\n");
    fold(&mut state, &mut parser, MENU);
    assert_eq!(
        state.order_menu.number("some acantha leaf"),
        Some(3),
        "guard: read"
    );

    // The same room again, a refresh: still that shop's menu.
    fold(&mut state, &mut parser, b"<nav rm='1001'/>\n");
    assert_eq!(state.order_menu.number("sprig of wolifrew"), Some(7));

    // Another room: another shop's numbers, which nobody has read yet.
    fold(&mut state, &mut parser, b"<nav rm='2002'/>\n");
    assert!(
        state.order_menu.is_empty(),
        "the last shop's menu outlived the move"
    );
}
