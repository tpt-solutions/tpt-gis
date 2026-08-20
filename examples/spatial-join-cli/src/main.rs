//! Demo binary for the spatial-join CLI.
//!
//! The CLI implementation lives in the publishable [`tpt-gis-cli`](https://crates.io/crates/tpt-gis-cli)
//! crate (binary name `tptgis`); this example just runs it so the engine can be
//! exercised with `cargo run -p spatial-join-cli`. The criterion benchmark for the
//! spatial-join kernel is kept here (in `benches/`) so the published crate stays lean.

fn main() {
    if let Err(e) = tpt_gis_cli::cli_main() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
