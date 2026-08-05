#![allow(clippy::missing_docs_in_private_items)]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = tpt_gis_io::shapefile::read_shp(data);
});
