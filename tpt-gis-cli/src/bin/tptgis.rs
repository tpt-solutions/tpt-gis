//! `tptgis` — the command-line entry point for the tpt-gis spatial join engine.
//!
//! Delegates to [`tpt_gis_cli::cli_main`], printing any error and exiting
//! non-zero on failure.

fn main() {
    if let Err(e) = tpt_gis_cli::cli_main() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
