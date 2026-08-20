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
argument; WGS84 is the default. The underlying math is Vincenty's inverse/direct
solution on the reference ellipsoid.

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

## Current scope (v0.1)

In scope: `Geometry` WKT/WKB/GeoJSON round-trip + `contains_point`, shapefile
reading, and `GeoPoint`/geodesic distance on WGS84/GRS80.

Deferred (not in v0.1): raster/COG access, the spatial-index crate, GeoPackage,
coordinate projections, and NumPy interop.

## Building wheels

`maturin build --release` produces a platform wheel under `target/wheels/` (or `dist/`
with `--out`). The crate is built as an abi3 extension, so a single wheel per OS is
compatible with every Python 3.8+ interpreter. CI publishes these wheels to PyPI
automatically on tagged releases.
