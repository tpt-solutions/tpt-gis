# Coming from GDAL, PROJ, GEOS, or geopandas

This page maps familiar operations to tpt-gis and says what is missing. Status is about
the code in this repository, not a release. For the full list of formats, CRS codes and
operations in one place, see the [support matrix](support-matrix.md).

- ✅ available
- 🟡 partial (see the note)
- ❌ not yet

## Vector data (GDAL / OGR)

| Operation | GDAL / OGR | tpt-gis | Status |
|-----------|------------|---------|--------|
| Convert between formats | `ogr2ogr` | `tptgis convert` | ✅ GeoJSON, WKT, WKB, GeoPackage, and Shapefile read and write. Other formats are not supported. |
| Shapefile attributes | `.dbf` via OGR | `tptgis convert`, `tpt_gis_io::shapefile::read_dbf_with` | ✅ `.cpg` encodings (UTF-8, Latin-1, Windows-1252); attributes written too |
| `.prj` for output | OGR writes `.prj` | `tptgis convert` | 🟡 written for GeoJSON input (WGS84) and for shapefile input; other inputs have no recorded projection |
| GeoPackage spatial index | `SPATIAL_INDEX=YES` | `tptgis convert --spatial-index`, `WriteOptions::spatial_index` | ✅ R-tree and its triggers, checked with SQLite's `rtreecheck` |
| Summary of a dataset | `ogrinfo -so` | `tptgis info` | ✅ counts, types, bounds, property names; projection for shapefiles |
| Vector reprojection | `ogr2ogr -t_srs` | `tptgis reproject`, `tpt_gis_core::crs::transform` | 🟡 the CLI reprojects from WGS84 only; the library converts between any two supported CRS (see below) |
| Streaming large files | OGR readers | `ShpGeometryIter`, streaming spatial join | ✅ |
| Other formats (KML, GML, FlatGeobuf, …) | many drivers | none | ❌ |

## Projections (PROJ)

| Operation | PROJ | tpt-gis | Status |
|-----------|------|---------|--------|
| Web Mercator | `+proj=merc` | `projection::web_mercator` | ✅ longitude wraps, latitude clamps |
| UTM | `+proj=utm` | `projection::utm` (`Zone`) | ✅ Norway and Svalbard exceptions; `Zone::try_new` rejects bad input. No arbitrary zone override. |
| Lambert Conformal Conic, two parallels | `+proj=lcc` | `projection::lcc::LambertConformal` | ✅ `lambert93()` for EPSG:2154 |
| Albers Equal-Area | `+proj=aea` | `projection::albers::Albers` | ✅ Conus Albers (EPSG:5070) |
| Transverse Mercator, any origin | `+proj=tmerc` | `projection::transverse_mercator::TransverseMercator` | ✅ series accurate within a few degrees of the central meridian (the British grid check is worst 10 mm at 4°) |
| Datum shifts (Helmert) | `+towgs84` | `datum::transform_geocentric` | ✅ 7-parameter Helmert. OSGB36 to WGS84 uses the OS parameters (about 5 m). |
| Grid-based datum shifts (NTv2, OSTN15) | `+nadgrids` | — | ❌ the OSTN15 grid is not bundled because its licence is not MIT/Apache. A user-supplied loader is open. |
| General `transform(from, to)` | `Transformer` | `tpt_gis_core::crs::transform(p, from, to)` | ✅ between the supported CRS only, not an arbitrary pipeline |
| EPSG registry | `EPSG:xxxx` | `epsg::lookup` | 🟡 a curated set: 4326, 3857, 2154, 5070, 4277, 27700, and the UTM zones 32601–32660 and 32701–32760. Not the full EPSG database. |
| Geodesic distance | `Geod` (Karney) | `geodesic::inverse` / `direct` | ✅ Karney's algorithm (GeographicLib port), checked on 613 problems |
| Geodesic area and perimeter | `Geod.polygon_area` | `geodesic::polygon_measure` | ✅ checked against GeographicLib's PolygonArea |
| Points along a geodesic | `InverseLine` | `geodesic::sample` | ✅ equal steps |
| Local planar approximation | — | `projection::local_tangent_plane` | ✅ handles the antimeridian and the poles |

## Geometry predicates and operations (GEOS)

| Operation | GEOS / shapely | tpt-gis | Status |
|-----------|----------------|---------|--------|
| `contains` (point in polygon) | `Polygon.contains(Point)` | `Polygon::contains_point` | ✅ holes respected |
| `contains` (polygon in polygon) | `Polygon.contains(Polygon)` | `predicates::polygon_contains_polygon` | ✅ boundary contact allowed; outer holes respected |
| `within` | `within` | `overlay::polygon_within`, `relate::line_within_polygon`, `relate::linestring_within_linestring` | ✅ polygons and lines |
| `intersects` | `intersects` | `predicates::polygons_intersect`, `predicates::line_intersects_polygon`, `de9im::relate` | ✅ |
| `touches`, `overlaps`, `crosses` | same | `de9im::relate` (`IntersectionMatrix::matches`), `relate::polygons_touch`, `relate::polygons_overlap`, `relate::line_crosses_polygon`, `line_relate::linestrings_cross` | ✅ checked against GEOS. `de9im` covers point, line and polygon-with-hole pairs. Collections are not supported. |
| Full DE-9IM matrix | `relate` | `de9im::relate` | 🟡 point, line and polygon pairs; multi-polygon parts are assumed not to overlap |
| Segment intersection point | `intersection` (lines) | `overlay::segment_intersection` | ✅ |
| `area` | `area` | `Polygon::area` | ✅ planar |
| `centroid` | `centroid` | `Polygon::centroid` | ✅ planar, holes removed |
| `length` | `length` | `LineString::length`, `MultiLineString::length` | ✅ planar |
| `convex_hull` | `convex_hull` | `convex_hull` | ✅ allocation-free (writes into a caller slice) |
| `simplify` | `simplify` | `simplify::simplify_into` (Douglas-Peucker) | ✅ |
| `is_valid` | `is_valid` | `Polygon::is_valid` | 🟡 rings, self-intersection, holes inside the exterior |
| `buffer` | `buffer` | `buffer::buffer_point`, `buffer_line`, `buffer_polygon` (`alloc`) | ✅ round joins; holes shrink; negative distances erode |
| `union`, `intersection`, `difference`, `symmetric_difference` | same | `clip::union` and friends (`alloc`) | ✅ Martinez–Rueda–Feito sweep, tested against a point-membership oracle and GEOS |
| Multi-part geometries | `Multi*` | `MultiPoint`, `MultiLineString`, `MultiPolygon`, `GeometryRef` | ✅ borrowed, zero-allocation |
| Nearest feature | `nearest_points`, sjoin nearest | `RTree::nearest`, `spatial_join::nearest_polygon_join` | ✅ |

Predicates use a relative tolerance (`predicates::EPSILON`) on orientation tests, so
they behave the same at micrometre and projected-metre scales.

## Rasters (GDAL / rasterio)

| Operation | GDAL / rasterio | tpt-gis | Status |
|-----------|-----------------|---------|--------|
| Read GeoTIFF | `rasterio.open` | `geotiff::read_file`, `geotiff::read_band` | ✅ multi-band; strips and tiles; deflate, LZW, PackBits, ZSTD; predictors 2 and 3; PixelIsPoint |
| Validate COG layout | `rio cogeo validate` | `cog::validate` | ✅ tiling, ordering, overviews and georeferencing |
| Remote COG windows | `/vsicurl/` | `http::CogRangeReader` (`http` feature) | 🟡 Range requests, custom headers, timeouts, a size cap; no retries |
| Map algebra | `gdal_calc` | `algebra` | ✅ add, subtract, multiply, reclassify, focal statistics, `focal_kernel` |
| Resampling | `gdalwarp -r` | `resample` | 🟡 nearest, bilinear, average |
| Write GeoTIFF | `gdal_translate` | `geotiff::write` | ✅ uncompressed, single strip, georeferencing and nodata. Multi-band output comes from `write_cog`. |
| Write Cloud Optimized GeoTIFF | `gdal_translate -of COG` | `geotiff::write_cog` | 🟡 tiled, multi-band, deflate, chained overviews from 2×2 means that skip nodata. Other overview methods, predictors and BigTIFF on write are not done. |
| Overviews | `gdaladdo` | `geotiff::write_cog` | 🟡 2×2 mean only |

## Python (shapely, geopandas)

`tpt-gis-py` covers the geometry, format, and geodesy basics. The bindings are
built as abi3 wheels for Python 3.8 and later (Linux x86_64 and aarch64, musllinux,
macOS universal2, and Windows x64). See [Python bindings](python-bindings.md).

| Operation | shapely / geopandas | tpt-gis-py | Status |
|-----------|---------------------|------------|--------|
| Parse WKT / WKB / GeoJSON | `wkt.loads`, `wkb.loads`, `shape` | `Geometry.from_wkt`, `from_wkb`, `from_geojson` | ✅ |
| Write WKT / WKB / GeoJSON | `wkt.dumps`, `mapping` | `to_wkt`, `to_wkb`, `to_geojson` | ✅ |
| Geo interface | `__geo_interface__` | `__geo_interface__` | ✅ `shapely.geometry.shape(g)` accepts these objects |
| `contains` (point) | `contains` | `contains_point` | ✅ |
| `area`, `length`, `bounds` | same | `area`, `length`, `bounds` | 🟡 planar only |
| Vertices as an array | `get_coordinates` | `coordinates()` | ✅ NumPy array (NumPy is imported lazily) |
| Read a shapefile | `geopandas.read_file` | `read_shapefile_geometry`, `read_shapefile_attributes`, `read_zipped_shapefile` | 🟡 returns a list of geometries and a list of dicts, not a DataFrame |
| Geodesic distance | `pyproj.Geod` | `geodesic_distance`, `geodesic_inverse`, `geodesic_destination` | ✅ |
| Reproject | `to_crs` | — | ❌ use `tptgis reproject` or the Rust API |
| Spatial join | `sjoin` | — | ❌ use `tptgis spatial-join` or the Rust API |
| DataFrame layer | `GeoDataFrame` | — | ❌ |

## Rust (`geo` crate)

The [`geo`](https://crates.io/crates/geo) crate works on `f64` `Coord`s and allocates
its outputs. tpt-gis uses `f64` too, but its geometries borrow their coordinate slices
and its core and geometry crates are `no_std`, so they can run on a microcontroller.

| Operation | `geo` | tpt-gis | Note |
|-----------|-------|---------|------|
| Contains | `Contains` | `Polygon::contains_point`, `predicates::polygon_contains_polygon` | |
| Intersects | `Intersects` | `predicates::polygons_intersect`, `de9im::relate` | |
| Boolean ops | `BooleanOps` | `clip::union`, `intersection`, `difference`, `symmetric_difference` | needs the `alloc` feature |
| Buffer | `Buffer` | `buffer::buffer_polygon`, `buffer_line`, `buffer_point` | needs the `alloc` feature |
| Area, centroid | `Area`, `Centroid` | `Polygon::area`, `Polygon::centroid` | planar |
| Geodesic area | `GeodesicArea` | `geodesic::polygon_measure` | |
| Convex hull, simplify | `ConvexHull`, `Simplify` | `convex_hull`, `simplify::simplify_into` | the hull writes into a caller slice |

## Side by side

```python
# shapely
from shapely.geometry import Point, Polygon
Polygon([(0, 0), (10, 0), (10, 10), (0, 10)]).contains(Point(5, 5))
```

```python
# tpt-gis-py
from tpt_gis_py import Geometry
Geometry.from_wkt("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))").contains_point(5, 5)
```

```rust
// tpt-gis
use tpt_gis_geom::{Point, Polygon};
const RING: [Point; 5] = [
    Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0),
    Point::new(0.0, 10.0), Point::new(0.0, 0.0),
];
assert!(Polygon::from_exterior(&RING).contains_point(Point::new(5.0, 5.0)));
```

The Rust version borrows its ring, so it can live in `static` memory with no allocation.
This is the reason the core and geometry crates are `no_std`.
