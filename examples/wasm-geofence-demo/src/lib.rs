//! Browser/WASM geofence demo.
//!
//! Reuses [`drone_geofence_demo::check_position`] — the exact same `no_std`,
//! zero-allocation geofencing routine that runs in the embedded drone demo — and
//! exposes it to JavaScript through a thin `wasm-bindgen` layer so it can be driven
//! from a dependency-free `index.html` canvas harness (see `index.html` beside this
//! crate).
//!
//! Build with:
//!
//! ```sh
//! cargo build --target wasm32-unknown-unknown
//! wasm-bindgen --target web --out-dir pkg \
//!     target/wasm32-unknown-unknown/debug/wasm_geofence_demo.wasm
//! ```
//!
//! then serve this directory (e.g. `python3 -m http.server`) and open `index.html`.

use drone_geofence_demo::check_position;
use tpt_gis_core::GeoPoint;
use wasm_bindgen::prelude::*;

/// Result of a geofence check, exposed to JavaScript.
///
/// `escape_bearing_deg` / `escape_distance_m` are `undefined` (not breached) when
/// `breached` is `false`.
#[wasm_bindgen]
#[derive(Clone)]
pub struct GeofenceResult {
    pub breached: bool,
    pub escape_bearing_deg: Option<f64>,
    pub escape_distance_m: Option<f64>,
}

/// Checks `(lat, lon)` (decimal degrees) against the shared no-fly zone and returns
/// the breach status plus, when inside, the escape vector from
/// [`drone_geofence_demo::check_position`].
#[wasm_bindgen]
pub fn check(lat: f64, lon: f64) -> GeofenceResult {
    let check = check_position(GeoPoint::new(lat, lon));
    GeofenceResult {
        breached: check.breached,
        escape_bearing_deg: check.escape_bearing_deg,
        escape_distance_m: check.escape_distance_m,
    }
}
