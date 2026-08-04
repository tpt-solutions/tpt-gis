#![allow(clippy::missing_docs_in_private_items)]

use tpt_gis_io::shapefile::read_shp;
use tpt_gis_io::shapefile::ShapefileError;

pub fuzz_target!(|data: &[u8]| {
    let _ = read_shp(data);
});
