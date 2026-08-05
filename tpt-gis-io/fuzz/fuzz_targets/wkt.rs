#![allow(clippy::missing_docs_in_private_items)]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    let _ = tpt_gis_io::wkt::parse_geometry(&s);
});
