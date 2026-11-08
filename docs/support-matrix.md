# Support matrix and FAQ

What tpt-gis reads, writes, projects and computes today. For the mapping from GDAL, PROJ,
GEOS and shapely names, see [Coming from GDAL, PROJ, GEOS, or geopandas](coming-from-gdal-proj-geos.md).

Status is about the code in this repository. The project is pre-1.0 and not yet on
crates.io or PyPI.

## Vector formats

| Format | Read | Write | Notes |
|--------|------|-------|-------|
| GeoJSON | ✅ | ✅ | `FeatureCollection`, bare `Feature` and bare geometry objects, `properties: null`. Coordinates are 2D. |
| WKT | ✅ | ✅ | `Z`, `M` and `ZM` tags and `SRID=` prefixes are accepted; Z and M values are dropped. `EMPTY` is rejected with a clear error. |
| WKB | ✅ | ✅ | ISO Z/M and EWKB (SRID) are accepted; extra ordinates are dropped. |
| Shapefile (`.shp`, `.shx`, `.dbf`, `.cpg`, `.prj`) | ✅ | ✅ | Z and M shape types are read as 2D. `.cpg` selects UTF-8, Latin-1 or Windows-1252 for attributes. Written with attributes, `.cpg` and `.prj` when the projection is known. |
| Shapefile in a `.zip` (Python) | ✅ | — | Read through `read_zipped_shapefile`. |
| GeoPackage (`.gpkg`) | ✅ | ✅ | Reads rowid and `WITHOUT ROWID` tables, and leaf and interior b-tree pages. Writes rowid tables, optionally with an R-tree spatial index (`--spatial-index`). |
| Other (KML, GML, FlatGeobuf, GeoParquet, …) | ❌ | ❌ | Not planned for the current phase. |

## Raster formats

| Format | Read | Write | Notes |
|--------|------|-------|-------|
| GeoTIFF, classic and BigTIFF | ✅ | ✅ | Reads strips and tiles, single or multi-band. Compression: none, DEFLATE, LZW, PackBits, ZSTD. Predictors 2 and 3. PixelIsPoint. Writes uncompressed single-strip files, or COG (below). |
| Cloud Optimized GeoTIFF | ✅ validate | ✅ | `cog::validate` checks tiling, internal ordering, overviews and georeferencing. `geotiff::write_cog` writes tiled, deflated, multi-band files with chained 2×2 mean overviews. |
| COG over HTTP | ✅ | — | `http::CogRangeReader` reads only the tiles a window needs. Needs the `http` feature. |

Raster data types, map algebra and resampling are in [the raster crate](../tpt-gis-raster/README.md).

## Coordinate reference systems

Supported in the library (`tpt_gis_core::epsg::lookup` and `crs::transform`):

| EPSG | Name | Notes |
|------|------|-------|
| 4326 | WGS84 geographic | |
| 3857 | Web Mercator | longitude wraps; latitude is clamped |
| 32601–32660 | WGS84 / UTM north, zones 1–60 | Norway and Svalbard exceptions handled |
| 32701–32760 | WGS84 / UTM south, zones 1–60 | |
| 2154 | RGF93 / Lambert-93 | |
| 5070 | NAD83 / Conus Albers | |
| 4277 | OSGB36 geographic | Helmert to WGS84 (about 5 m) |
| 27700 | British National Grid | checked against PROJ and the OS worked example |

`crs::transform` converts between any two of these. Other codes are rejected, and the
error lists the supported ones. A general EPSG registry, arbitrary zone overrides, and
grid-based datum shifts (NTv2, OSTN15) are not implemented.

## Geometry operations

| Operation | Planar | Geodesic | Notes |
|-----------|--------|----------|-------|
| Predicates: `contains`, `within`, `intersects`, `touches`, `overlaps`, `crosses` | ✅ | — | Full DE-9IM for point, line and polygon pairs (`de9im::relate`). Collections are not supported. |
| `area`, `centroid`, `length` | ✅ | ✅ `polygon_measure` | Geodesic area and perimeter use Karney's algorithm. |
| `distance` | ✅ | ✅ `geodesic::inverse` | Planar distance for points, segments, lines and polygon boundaries. |
| `buffer` | ✅ | — | Points, lines and polygons, with round joins. Holes shrink; negative distances erode. |
| `union`, `intersection`, `difference`, `symmetric_difference` | ✅ | — | Martinez–Rueda–Feito sweep (`alloc` feature). |
| `convex_hull`, `simplify` | ✅ | — | |
| `is_valid` | 🟡 | — | Checks rings, self-intersection and holes inside the exterior. |
| Multi-part geometries | ✅ | — | `MultiPoint`, `MultiLineString`, `MultiPolygon`, `GeometryRef`. |
| Spatial index: R-tree, quadtree, H3, S2 | ✅ | — | S2 ids are Morton (Z-order), not Hilbert. They are not compatible with Google's S2 library. |
| Spatial join, nearest join | ✅ | — | R-tree based. |

## Accuracy

Checked against independent implementations; numbers are worst cases over the test sets:

| Area | Reference | Result |
|------|-----------|--------|
| Geodesic inverse and direct | GeographicLib | 613 problems |
| Geodesic polygon area | GeographicLib `PolygonArea` | relative error 7×10⁻⁹ on 225 polygons |
| UTM, Lambert-93, Conus Albers, Web Mercator | PROJ (rasterio) | UTM about 1 mm; Lambert-93 and Albers about 7 µm; Web Mercator about 34 µm |
| British National Grid | PROJ | worst 10 mm over 80 points |
| Polygon predicates and boolean ops | GEOS (shapely) | 400 random polygon pairs; 1200 random point, line and polygon pairs for DE-9IM |
| GeoTIFF, COG, GeoPackage | GDAL | GDAL-written fixtures, checked cell by cell |

The differential tests and their fixture generators are in each crate's `tests/` folder.

## FAQ

**Does it need GDAL, PROJ or GEOS?**
No. The engine is pure Rust with no C FFI. `deny.toml` rejects any GPL or LGPL dependency.

**Can I use it on a microcontroller?**
`tpt-gis-core` and `tpt-gis-geom` are `no_std`. They build for `thumbv7em-none-eabihf`,
`riscv32imc-unknown-none-elf` (ESP32-C3) and `wasm32-unknown-unknown`. The I/O, index and
raster crates need `std`. The Xtensa targets (ESP32, ESP32-S3) need the `esp-rs` toolchain
and are not yet checked in CI.

**Are Z and M coordinates kept?**
No. The data model is 2D. Z and M values are read and dropped, so a round trip loses them.

**Which projections can I use?**
The codes in the table above. An unknown code is an error, not a silent fallback.

**Why not use the full EPSG database?**
The project keeps a small, checked registry rather than shipping the EPSG dataset, whose
licence is not compatible with MIT/Apache-2.0. Adding a code means adding a test against
PROJ.

**Is the S2 index compatible with Google's S2?**
No. The cell ids are Morton (Z-order) codes. They are internally consistent, but a Google
S2 cell id will not match one from this crate.

**Is it published?**
No. Nothing is on crates.io or PyPI yet. The GitHub Actions workflows have been written and
checked locally; they have not yet run on GitHub.

**Where do I report a bug or ask a question?**
Open an issue on the [GitHub repository](https://github.com/tpt-solutions/tpt-gis). The
project takes issues only; pull requests are not accepted.

**What is the licence?**
MIT OR Apache-2.0, at your option.
