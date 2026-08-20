# tpt-gis-py

Python bindings for [tpt-gis](https://github.com/tpt-solutions/tpt-gis), a pure-Rust
planetary-scale GIS engine. Built with [pyo3](https://pyo3.rs) and
[maturin](https://github.com/PyO3/maturin).

v0.1 scope:

- **`Geometry`** — parse and serialize OGC Simple Features geometries across WKT, WKB,
  and GeoJSON, plus `contains_point` for point-in-polygon tests.
- **Shapefile reading** — `read_shapefile_geometry` (`.shp`) and
  `read_shapefile_attributes` (`.dbf`).
- **Geodesy** — `GeoPoint` and geodesic distance / inverse / destination on the WGS84
  and GRS80 ellipsoids (Vincenty's formulae).

Deferred (not in v0.1): raster/COG, the index crate, GeoPackage, projections, and numpy
interop.

## Build & install (development)

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

## License

Dual MIT / Apache-2.0 (same as the rest of tpt-gis). Copyright TPT Solutions.
