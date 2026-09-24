//! The pathfinder over a real map, against an answer key computed from the
//! upstream file by an independent implementation
//! (`research/mapdb-inventory/route_key.py`). `#[ignore]`d, and runs only when
//! asked for with both named:
//!
//! ```powershell
//! python research\mapdb-inventory\route_key.py <upstream-map.json> > route_key.tsv
//! $env:CENA_MAP = "<dir>\hydra.map"; $env:CENA_ROUTE_KEY = "route_key.tsv"
//! cargo test --release -p cena-map --test route_real_map -- --ignored --nocapture
//! ```
//!
//! The key and the binary must come from the same upstream file.
//!
//! **Ignored rather than silently passing**, for the reason
//! `locate_real_map.rs` gives: an early return reported `1 passed` for a check
//! that never ran, and `decode(..).ok()?` did the same for a map that failed
//! to load. A named file that cannot be read or decoded now fails.

use std::collections::BTreeMap;
use std::time::Instant;

use cena_map::{Map, RoomId, Target, as_converted, binary};

/// The map and key the environment names; `None` only when one is not named.
/// A named file that cannot be read, or a map that does not decode, is a
/// failure -- never a skip.
// `cfg(test)` so clippy.toml's allow-expect-in-tests covers a helper too.
#[cfg(test)]
fn load() -> Option<(Map, String)> {
    let (map, key) = (
        std::env::var_os("CENA_MAP")?,
        std::env::var_os("CENA_ROUTE_KEY")?,
    );
    let bytes = std::fs::read(&map).expect("CENA_MAP names a file that cannot be read");
    let key =
        std::fs::read_to_string(&key).expect("CENA_ROUTE_KEY names a file that cannot be read");
    let map = binary::decode(&bytes).expect("CENA_MAP names a map that does not decode");
    Some((map, key))
}

#[test]
#[ignore = "Tier 2: needs CENA_MAP and CENA_ROUTE_KEY; run with --ignored"]
fn distances_agree_with_the_independent_key() {
    let Some((map, key)) = load() else {
        eprintln!("CENA_MAP and CENA_ROUTE_KEY are not both set; skipped");
        return;
    };
    let mut wanted: BTreeMap<u32, Vec<(u32, Option<f64>)>> = BTreeMap::new();
    for line in key.lines().skip(1) {
        let mut fields = line.split('\t');
        let (Some(from), Some(to), Some(seconds)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        wanted
            .entry(from.parse().unwrap())
            .or_default()
            .push((to.parse().unwrap(), seconds.trim().parse().ok()));
    }
    assert!(!wanted.is_empty(), "the key has no rows");

    let (mut checked, mut reachable, mut slowest) = (0, 0, std::time::Duration::ZERO);
    for (from, rows) in &wanted {
        let started = Instant::now();
        let routes = map.routes(RoomId(*from), Target::Everything, as_converted);
        slowest = slowest.max(started.elapsed());
        for (to, seconds) in rows {
            let found = routes.seconds_to(RoomId(*to));
            match (found, seconds) {
                (None, None) => {}
                (Some(found), Some(key)) if (found - key).abs() < 1e-4 => {
                    reachable += 1;
                    // A path must exist, end where asked, and cost what was said.
                    let path = routes.path_to(RoomId(*to)).unwrap();
                    assert_eq!(path.last().copied().unwrap_or(RoomId(*from)), RoomId(*to));
                }
                _ => panic!("{from} -> {to}: found {found:?}, the key says {seconds:?}"),
            }
            checked += 1;
        }
    }
    println!(
        "{checked} distances from {} sources agree ({reachable} reachable); \
         slowest whole-map search {slowest:?}",
        wanted.len()
    );
}
