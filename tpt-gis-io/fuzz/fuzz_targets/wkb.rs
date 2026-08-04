#![allow(clippy::missing_docs_in_private_items)]

use tpt_gis_io::wkb::parse_wkb;
use tpt_gis_io::geometry::Geometry;

pub fuzz_target!(|data: &[u8]| {
    let _ = parse_wkb(data);
});
