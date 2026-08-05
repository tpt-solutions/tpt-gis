#![allow(clippy::missing_docs_in_private_items)]

use libfuzzer_sys::fuzz_target;

// Exercises the GeoTIFF/COG parser and its (now size-capped) decompression paths.
// A hostile payload can no longer make `read` allocate an unbounded amount of memory.
fuzz_target!(|data: &[u8]| {
    let _ = tpt_gis_raster::geotiff::read(data);
});
