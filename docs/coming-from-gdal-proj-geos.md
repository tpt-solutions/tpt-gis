# Coming from GDAL, PROJ, or GEOS

This page maps familiar operations to tpt-gis and says what is missing. Status is
about the code in this repository, not a release.

- ✅ available
- 🟡 partial (see the note)
- ❌ not yet

## Vector data (GDAL / OGR)

| Operation | GDAL / OGR | tpt-gis | Status |
|-----------|------------|---------|--------|
| Convert between formats | `ogr2ogr` | `tptgis convert` | 🟡 GeoJSON, WKT, WKB, GeoPackage read and write; Shapefile read only |
| Shapefile attributes | `.dbf` via OGR | `tptgis convert`, `tpt_gis_io::shapefile::read_dbf_with` | ✅ `.cpg` encodings (UTF-8, Latin-1, Windows-1252) |
| Summary of a dataset | `ogrinfo -so` | `tptgis info` | ✅ counts, types, bounds, property names |
| Vector reprojection | `ogr2ogr -t_srs` | `tptgis reproject` | 🟡 from WGS84 to Web Mercator or UTM only |
| Streaming large files | OGR readers | `ShpGeometryIter`, streaming spatial join | ✅ |
| Other formats (KML, GML, FlatGeobuf, …) | many drivers | none | ❌ |

## Projections (PROJ)

| Operation | PROJ | tpt-gis | Status |
|-----------|------|---------|--------|
| Web Mercator | `+proj=merc` | `projection::web_mercator` | ✅ |
| UTM | `+proj=utm` | `projection::utm` (`Zone`) | 🟡 Norway and Svalbard exceptions; no general zone override |
| Lambert Conformal Conic, two parallels | `+proj=lcc` | `projection::lcc::LambertConformal` | ✅ Lambert-93 constructor included |
| Albers Equal-Area | `+proj=aea` | `projection::albers::Albers` | ✅ |
| Transverse Mercator, any origin | `+proj=tmerc` | `projection::transverse_mercator::TransverseMercator` | ✅ series accurate within a few degrees of the central meridian |
| Datum shifts (Helmert) | `+towgs84` | `datum::transform_geocentric` | ✅ 7-parameter Helmert only |
| General `transform(from, to)` | `Transformer` | — | ❌ no registry or pipeline |
| EPSG registry | `EPSG:xxxx` | `epsg::lookup` | 🟡 WGS84, Web Mercator, UTM zones; the rest of EPSG is not included |
| Geodesic distance | `Geod` (Karney) | `geodesic::inverse` / `direct` | 🟡 Vincenty; fails to converge for some nearly antipodal pairs |
| Local planar approximation | — | `projection::local_tangent_plane` | ✅ |

## Geometry predicates and operations (GEOS)

| Operation | GEOS / shapely | tpt-gis | Status |
|-----------|----------------|---------|--------|
| `contains` (point in polygon) | `Polygon.contains(Point)` | `Polygon::contains_point` | ✅ holes respected |
| `contains` (polygon in polygon) | `Polygon.contains(Polygon)` | `predicates::polygon_contains_polygon` | ✅ boundary contact allowed |
| `within` | `within` | `overlay::polygon_within` | ✅ polygons only |
| `intersects` | `intersects` | `predicates::polygons_intersect`, `segments_intersect` | 🟡 boundary contact and nesting; not full DE-9IM |
| `touches`, `overlaps` | same | `relate::polygons_touch`, `relate::polygons_overlap` (`alloc`) | 🟡 polygon-polygon only, not full DE-9IM |
| `crosses` | same | — | ❌ |
| Segment intersection point | `intersection` (lines) | `overlay::segment_intersection` | ✅ |
| `area` | `area` | `Polygon::area` | ✅ planar |
| `centroid` | `centroid` | `Polygon::centroid` | ✅ planar, holes removed |
| `length` | `length` | `LineString::length`, `MultiLineString::length` | ✅ planar |
| `convex_hull` | `convex_hull` | `convex_hull` | ✅ allocation-free |
| `simplify` | `simplify` | `simplify::simplify_into` (Douglas-Peucker) | ✅ |
| `is_valid` | `is_valid` | `Polygon::is_valid` | 🟡 rings, self-intersection, holes inside the exterior |
| `buffer` | `buffer` | `buffer::buffer_point`, `buffer_line`, `buffer_polygon` (`alloc`) | 🟡 round joins; holes shrink; no erosion (negative distances) |
| `union`, `intersection`, `difference`, `symmetric_difference` | same | `clip::union` and friends (`alloc`) | ✅ Martinez–Rueda–Feito sweep; holes and coincident edges handled |
| Multi-part geometries | `Multi*` | `MultiPoint`, `MultiLineString`, `MultiPolygon`, `GeometryRef` | ✅ borrowed, zero-allocation |
| Nearest feature | `nearest_points`, sjoin nearest | `RTree::nearest`, `spatial_join::nearest_polygon_join` | ✅ |

Predicates use a relative tolerance (`predicates::EPSILON`) on orientation tests, so
they behave the same at micrometre and projected-metre scales.

## Rasters (GDAL / rasterio)

| Operation | GDAL / rasterio | tpt-gis | Status |
|-----------|-----------------|---------|--------|
| Read GeoTIFF | `rasterio.open` | `raster::geotiff::read_file` | 🟡 single band; strips and tiles; deflate, LZW, PackBits, ZSTD; predictors 2 and 3; PixelIsPoint |
| Validate COG layout | `rio cogeo validate` | `raster::cog::validate` | ✅ |
| Remote COG windows | `/vsicurl/` | `raster::http::CogRangeReader` (`http` feature) | 🟡 Range requests, custom headers; no retries |
| Map algebra | `gdal_calc` | `raster::algebra` | ✅ add, subtract, reclassify, focal statistics |
| Resampling | `gdalwarp -r` | `raster::resample` | 🟡 nearest, bilinear, average |
| Write GeoTIFF or COG | `gdal_translate` | — | ❌ |
| Multi-band, overviews, ZSTD | many | — | ❌ |

## Python

`tpt-gis-py` covers the geometry, format, and geodesy basics. Compared with shapely or
geopandas it has no DataFrame layer and no NumPy interop yet. See
[Python bindings](python-bindings.md).

## Side by side

```python
# shapely
from shapely.geometry import Point, Polygon
Polygon([(0, 0), (10, 0), (10, 10), (0, 10)]).contains(Point(5, 5))
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
