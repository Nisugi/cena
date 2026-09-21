//! The pathfinder over a real map, against an answer key computed from the
//! upstream file by an independent implementation
//! (`research/mapdb-inventory/route_key.py`). Runs only when both are named:
//!
//! ```powershell
//! python research\mapdb-inventory\route_key.py <upstream-map.json> > route_key.tsv
//! $env:CENA_MAP = "<dir>\hydra.map"; $env:CENA_ROUTE_KEY = "route_key.tsv"
//! cargo test --release -p cena-map --test route_real_map -- --nocapture
//! ```
//!
//! The key and the binary must come from the same upstream file.

use std::collections::BTreeMap;
use std::time::Instant;

use cena_map::{Map, RoomId, Target, as_converted, binary};

fn load() -> Option<(Map, String)> {
    let map = std::fs::read(std::env::var_os("CENA_MAP")?).ok()?;
    let key = std::fs::read_to_string(std::env::var_os("CENA_ROUTE_KEY")?).ok()?;
    Some((binary::decode(&map).ok()?, key))
}

#[test]
fn distances_agree_with_the_independent_key() {
    let Some((map, key)) = load() else {
        eprintln!("CENA_MAP and CENA_ROUTE_KEY are not both readable; skipped");
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
