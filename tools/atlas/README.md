# Offline atlas build

Python 3, Node 24 and the repository's pinned Rust toolchain are used. No npm
packages or game credentials are needed for generation. Put scratch/build
directories on a data drive; the native `.map` input is not changed.

```sh
cargo build --locked --manifest-path tools/atlas-export/Cargo.toml
python3 tools/atlas/build.py \
  --map /data-drive/gs.map \
  --exporter tools/atlas-export/target/debug/hydra-atlas-export \
  --catalogue tools/atlas/creature-catalogue.json \
  --work /data-drive/atlas-build-new \
  --output /data-drive/atlas-data-new
```

Use `hydra-atlas-export.exe` on Windows. If `CARGO_TARGET_DIR` is set, use the
corresponding binary path. Work/output directories must be new; generation
refuses to overwrite an older snapshot. The exporter lockfile pins both native
repositories. `build.py` records the source map and exporter binary hashes.
Changing the engine requires updating its recorded revision as well as the lock.

The generator partitions by native `area:` and `region:` metadata. Unassigned
rooms get provisional connected display groups only. Each export is compared
against the decoded native records before any presentation wrapper is added.
New region names fail for explicit profile review rather than being dropped.

After reviewing a new build, replace `crates/cena-web/atlas-data` with the
generated output and update the literal path/region inventory in
`crates/cena-web/src/atlas_data.rs` if its files changed. Do not copy scratch
exports or the raw `.map` into the source tree. Keep native room content out of
handwritten tests: test the shipped snapshot and use tiny synthetic cases for
specific invariants. Context copies must never enter canonical room counts.

To refresh the independently sourced creature reference catalogue:

```sh
node tools/atlas/context.mjs freeze /path/to/Lich5/lib/gemstone/creatures /data-drive/new-catalogue.json
```

Review unsupported literals and habitat coverage before replacing the checked-in
catalogue. The reader rejects executable Ruby and interpolation. Names do not
substitute for habitat UIDs. The shipped Lich license notice must be retained.

Verification:

```sh
python3 -m unittest discover -s tools/atlas -p 'test_*.py'
node --test crates/cena-web/assets/atlas/tests/*.mjs
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo doc --workspace --no-deps
cargo build --locked -p cena-web --example atlas_preview
node crates/cena-web/browser-tests/atlas.mjs
```

The browser test uses Playwright (same pinned install as the existing browser
smoke), optionally `PLAYWRIGHT_MODULE` and `BROWSER_EXECUTABLE_PATH`. It launches
only the offline example, verifies same-origin GET requests and no WebSockets,
and writes a screenshot under `target/atlas-evidence`. CI runs these tests too.
