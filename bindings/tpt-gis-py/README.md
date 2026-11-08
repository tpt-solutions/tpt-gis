# tpt-gis-py

Python bindings for [tpt-gis](https://github.com/tpt-solutions/tpt-gis), a pure-Rust
planetary-scale GIS engine. Built with [pyo3](https://pyo3.rs) and
[maturin](https://github.com/PyO3/maturin).

> **Status:** not yet published to PyPI. Build from source (below).

## Scope (v0.1)

- **`Geometry`** — parse and serialise OGC Simple Features geometries as WKT, WKB and GeoJSON.
  Includes `contains_point`, `geometry_type`, `bounds`, `area`, `length`, `__geo_interface__`,
  value equality, hashing and pickling.
- **`coordinates(geometry)`** — every vertex as an `(n, 2)` float64 NumPy array. NumPy is
  imported only when you call it.
- **Shapefile reading** — `read_shapefile_geometry` (`.shp`), `read_shapefile_attributes`
  (`.dbf`) and `read_zipped_shapefile` (a shapefile inside a `.zip`). The readers release the GIL
  while they parse.
- **Geodesy** — `GeoPoint`, `geodesic_distance`, `geodesic_inverse` and `geodesic_destination` on
  the WGS84 and GRS80 ellipsoids (Karney's algorithm).
- **Errors** — `TptGisError` (a `ValueError` subclass), with `GeometryParseError` and
  `GeodesyError` beneath it.
- **Types** — `py.typed` and `.pyi` stubs ship with the package.

Not yet exposed to Python: raster and COG, the index crate, GeoPackage, projections,
and a prebuilt wheel matrix.

## Build and install (development)

Requires Python 3.8+ and a Rust toolchain.

```bash
# from the bindings/tpt-gis-py directory
pip install maturin
maturin develop
```

## Usage

```python
import tpt_gis_py as gis

# WKT / WKB / GeoJSON round-trip
geom = gis.Geometry.from_wkt("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))")
assert geom.contains_point(5.0, 5.0)
assert not geom.contains_point(15.0, 5.0)
wkb = geom.to_wkb()
assert gis.Geometry.from_wkb(wkb).to_geojson() == geom.to_geojson()

# Vertices as a NumPy array
gis.coordinates(gis.Geometry.from_wkt("LINESTRING (0 0, 3 4, 6 0)"))

# Geodesy
flinders = gis.GeoPoint(-37.951_033, 144.424_868)
buninyong = gis.GeoPoint(-37.652_821, 143.926_495)
dist = gis.geodesic_distance(flinders, buninyong)  # ~54_972 m on WGS84
result = gis.geodesic_inverse(flinders, buninyong)
print(result.distance_m, result.initial_bearing_deg)

# Shapefile
geoms = gis.read_shapefile_geometry("states.shp")
attrs = gis.read_shapefile_attributes("states.dbf")
```

See the [Python bindings guide](../../docs/python-bindings.md) for the full API, with
worked examples and the error model.

## Tests

```bash
pip install pytest
pytest tests
```

## License

Dual MIT / Apache-2.0 (same as the rest of tpt-gis). Copyright TPT Solutions.
