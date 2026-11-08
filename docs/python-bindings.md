# Python Bindings (`tpt-gis-py`)

`tpt-gis` exposes its geometry, format, and geodesy engines to Python through
[`tpt-gis-py`](https://pypi.org/project/tpt-gis-py/), a native extension module built
with [pyo3](https://pyo3.rs) and [maturin](https://github.com/PyO3/maturin). The
module wraps the `tpt-gis-io` (parsing) and `tpt-gis-core` (geodesy) Rust crates; the
heavy lifting stays in compiled Rust, so you get C-speed geometry and distance math
without writing any Rust yourself.

## Installation

From PyPI (recommended) — wheels are provided for Linux (manylinux/musllinux), macOS,
and Windows, for Python 3.8 and newer:

```sh
pip install tpt-gis-py
```

Build from source (needs a Rust toolchain):

```sh
cd bindings/tpt-gis-py
pip install maturin
maturin develop          # builds the extension into your active environment
pytest -v               # runs the test suite
```

## API overview

### `Geometry` — formats and containment

A `Geometry` holds any OGC Simple Features geometry (Point, LineString, Polygon,
Multi*, GeometryCollection). Parse from WKT, WKB, or GeoJSON and serialize back to any
of them:

```python
import tpt_gis_py as gis

geom = gis.Geometry.from_wkt("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))")
assert geom.geometry_type == "Polygon"
assert geom.contains_point(5.0, 5.0)        # point-in-polygon (with holes)
assert not geom.contains_point(15.0, 5.0)

wkb = geom.to_wkb()                          # bytes
assert gis.Geometry.from_wkb(wkb).to_geojson() == geom.to_geojson()
```

`contains_point(x, y)` works for `Polygon` and `MultiPolygon`; other geometry types
always return `False`.

### `GeoPoint` and geodesy

```python
london = gis.GeoPoint(51.5074, -0.1278)
paris  = gis.GeoPoint(48.8566, 2.3522)

gis.geodesic_distance(london, paris)             # ≈ 343_500 m on WGS84
result = gis.geodesic_inverse(london, paris)     # InverseResult(distance_m, ...)
result.distance_m
result.initial_bearing_deg

# The direct problem: destination after travelling a bearing/distance.
dest = gis.geodesic_destination(london, result.initial_bearing_deg, result.distance_m)
```

All three functions take an optional `ellipsoid="wgs84"` (or `"grs80"`) keyword
argument; WGS84 is the default. The underlying math is Karney's inverse/direct
solution (a port of GeographicLib) on the reference ellipsoid.

### Coordinates as NumPy

`coordinates(geometry)` returns every vertex as an `(n, 2)` float64 array, with rings in
order (exterior, then holes). NumPy is imported only when this function is called.

```python
import tpt_gis_py as gis
gis.coordinates(gis.Geometry.from_wkt("LINESTRING (0 0, 3 4, 6 0)")).tolist()
# [[0.0, 0.0], [3.0, 4.0], [6.0, 0.0]]
```

### Shapefile reading

```python
geoms = gis.read_shapefile_geometry("states.shp")   # list[Geometry]
attrs = gis.read_shapefile_attributes("states.dbf") # list[{field: value}]
```

`read_shapefile_geometry` reads the `.shp` main file (the `.shx` index is not needed);
`read_shapefile_attributes` reads the `.dbf` attribute records and returns a list of
dicts mapping each field name to a Python value (`str`, `float`, `Optional[bool]`, or
an ISO `YYYY-MM-DD` date string). See `tpt-gis-io`'s shapefile module for the full
scope notes (Z/M shapes and multi-exterior polygons are not supported).

A shapefile zipped into an archive can be read directly. `read_zipped_shapefile` takes the path
of a `.zip`, reads the first `.shp` in it with its `.dbf` and `.cpg` (the `.dbf` is optional), and
returns `(geometries, attributes)`:

```python
geoms, attrs = gis.read_zipped_shapefile("states.zip")
```

It copies the members to a temporary directory under fixed names, so member names are never
used as file paths.

### Geometry properties, equality, and pickling

```python
geom = gis.Geometry.from_wkt("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))")
geom.bounds          # (0.0, 0.0, 10.0, 10.0)
geom.area            # 100.0 (planar; holes subtracted)
geom.length          # 40.0 (perimeter for polygons, path length for lines)
geom.__geo_interface__   # {"type": "Polygon", "coordinates": [...]}
geom == gis.Geometry.from_wkt("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))")  # True
pickle.loads(pickle.dumps(geom)) == geom                                   # True
```

Geometries, points, and geo-points compare by value and hash consistently, so they
work in sets and as dict keys. Pickling uses WKB, or the constructor's arguments for
points.

### Errors

All errors subclass `tpt_gis_py.TptGisError`, which subclasses `ValueError`, so
existing `except ValueError` code keeps working:

- `GeometryParseError`: a WKT, WKB, GeoJSON, shapefile, or DBF input could not be parsed.
- `GeodesyError`: invalid coordinates (NaN, infinite, or latitude outside ±90°), or a
  nearly antipodal inverse problem that did not converge.

## Current scope (v0.1)

In scope: `Geometry` WKT/WKB/GeoJSON round-trip, `contains_point`, bounds, area, length,
`__geo_interface__`, equality, and pickling; shapefile reading; `GeoPoint`/geodesic
distance and destination on WGS84/GRS80; type stubs (`py.typed`) checked against the
extension by `mypy.stubtest` in CI.

Deferred (not in v0.1): raster/COG access, the spatial-index crate, GeoPackage,
coordinate projections, and NumPy interop.

## Building wheels

`maturin build --release` produces a platform wheel under `target/wheels/` (or `dist/`
with `--out`). The crate is built as an abi3 extension, so a single wheel per OS is
compatible with every Python 3.8+ interpreter. CI publishes these wheels to PyPI
automatically on tagged releases.
