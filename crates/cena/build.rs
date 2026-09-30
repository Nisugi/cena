//! The hydra-mapper commit `Cargo.lock` holds, handed to the build as
//! `HYDRA_MAPPER_COMMIT`: the minimap's layout cache is named by it
//! (`src/atlas/service.rs`), so a newer mapper lays the map out again and
//! never reads an older one's areas. Hydra follows the mapper's main; this
//! is read, never written by hand.

fn main() {
    let lock = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
        .join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let commit = std::fs::read_to_string(&lock)
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.contains("github.com/Nisugi/hydra-mapper"))
                .and_then(|line| line.trim_end_matches('"').rsplit('#').next())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=HYDRA_MAPPER_COMMIT={commit}");
}
