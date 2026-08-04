#![allow(clippy::missing_docs_in_private_items)]

use tpt_gis_io::wkt::parse_wkt;
use tpt_gis_io::geometry::Geometry;

pub fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    let _ = parse_wkt(&s);
});
