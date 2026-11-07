//! Python bindings for `tpt-gis` (pyo3 + maturin).
//!
//! v0.1 scope (see `todo.md`):
//! - `Geometry`: WKT / WKB / GeoJSON round-trip + `contains_point`
//! - Shapefile reading (`.shp` geometry + `.dbf` attributes)
//! - `GeoPoint` and geodesic distance from `tpt-gis-core`
//!
//! Deferred: raster/COG, the index crate, GeoPackage, projections, numpy interop.

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyType};
use pyo3::Bound;
use std::hash::{DefaultHasher, Hash, Hasher};

use tpt_gis_core::geodesic::{self, GeodesicError};
use tpt_gis_core::{Ellipsoid, GeoPoint as CoreGeoPoint};
use tpt_gis_geom::Point as GeomPoint;
use tpt_gis_io::geometry::Geometry as InnerGeometry;
use tpt_gis_io::shapefile::DbfValue;

pyo3::create_exception!(
    _core,
    TptGisError,
    PyValueError,
    "Base class for tpt-gis errors. It subclasses ValueError."
);
pyo3::create_exception!(
    _core,
    GeometryParseError,
    TptGisError,
    "A geometry string, buffer, or file could not be parsed."
);
pyo3::create_exception!(
    _core,
    GeodesyError,
    TptGisError,
    "A geodesic calculation got invalid input or did not converge."
);

/// Converts a geometry or file parse failure into a `GeometryParseError`.
fn to_pyerr<E: std::fmt::Display>(e: E) -> PyErr {
    GeometryParseError::new_err(e.to_string())
}

/// Converts a geodesy failure into a `GeodesyError`.
fn geodesy_err(e: GeodesicError) -> PyErr {
    GeodesyError::new_err(match e {
        GeodesicError::ConvergenceFailure => {
            "geodesic inverse failed to converge (near-antipodal points)".to_string()
        }
        GeodesicError::InvalidInput => {
            "invalid coordinates: values must be finite and latitude within [-90, 90]".to_string()
        }
    })
}

/// Hashes any `Hash` value to the `isize` Python expects from `__hash__`.
fn hash_of<T: Hash>(value: &T) -> isize {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish() as isize
}

/// Every coordinate of a geometry, for `bounds`.
fn collect_points(g: &InnerGeometry, out: &mut Vec<GeomPoint>) {
    match g {
        InnerGeometry::Point(p) => out.push(*p),
        InnerGeometry::LineString(pts) | InnerGeometry::MultiPoint(pts) => out.extend(pts),
        InnerGeometry::Polygon(p) => p.rings.iter().for_each(|r| out.extend(r)),
        InnerGeometry::MultiLineString(lines) => lines.iter().for_each(|l| out.extend(l)),
        InnerGeometry::MultiPolygon(polys) => {
            polys.iter().flat_map(|p| p.rings.iter()).for_each(|r| out.extend(r))
        }
        InnerGeometry::GeometryCollection(items) => {
            items.iter().for_each(|i| collect_points(i, out))
        }
    }
}

/// Planar area: polygon parts contribute their exterior minus holes.
fn planar_area(g: &InnerGeometry) -> f64 {
    match g {
        InnerGeometry::Polygon(p) => {
            let mut scratch = Vec::new();
            p.as_geom(&mut scratch).area()
        }
        InnerGeometry::MultiPolygon(polys) => {
            polys.iter().map(|p| planar_area(&InnerGeometry::Polygon(p.clone()))).sum()
        }
        InnerGeometry::GeometryCollection(items) => items.iter().map(planar_area).sum(),
        _ => 0.0,
    }
}

/// Planar length: line lengths, and polygon perimeters (all rings, holes included).
fn planar_length(g: &InnerGeometry) -> f64 {
    let path = |pts: &[GeomPoint]| -> f64 {
        pts.windows(2).map(|w| tpt_gis_geom::distance::point_to_point(w[0], w[1])).sum()
    };
    match g {
        InnerGeometry::LineString(pts) => path(pts),
        InnerGeometry::MultiLineString(lines) => lines.iter().map(|l| path(l)).sum(),
        InnerGeometry::Polygon(p) => p.rings.iter().map(|r| path(r)).sum(),
        InnerGeometry::MultiPolygon(polys) => {
            polys.iter().flat_map(|p| p.rings.iter()).map(|r| path(r)).sum()
        }
        InnerGeometry::GeometryCollection(items) => items.iter().map(planar_length).sum(),
        _ => 0.0,
    }
}

/// Resolves a named reference ellipsoid.
fn parse_ellipsoid(name: &str) -> PyResult<Ellipsoid> {
    match name.to_ascii_lowercase().as_str() {
        "wgs84" | "wgs-84" | "wgs" => Ok(Ellipsoid::WGS84),
        "grs80" | "grs-80" => Ok(Ellipsoid::GRS80),
        other => Err(PyValueError::new_err(format!(
            "unknown ellipsoid {other:?}; expected 'wgs84' or 'grs80'"
        ))),
    }
}

/// A planar (projected) coordinate, in the linear unit of its CRS (typically meters).
#[pyclass]
#[derive(Clone, Copy)]
pub struct Point {
    /// Easting / X coordinate.
    #[pyo3(get, set)]
    pub x: f64,
    /// Northing / Y coordinate.
    #[pyo3(get, set)]
    pub y: f64,
}

#[pymethods]
impl Point {
    /// Creates a `Point` from an `x` (easting) and `y` (northing) ordinate.
    #[new]
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn __repr__(&self) -> String {
        format!("Point(x={}, y={})", self.x, self.y)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Point>>()
            .is_ok_and(|o| self.x.to_bits() == o.x.to_bits() && self.y.to_bits() == o.y.to_bits())
    }

    fn __hash__(&self) -> isize {
        hash_of(&(self.x.to_bits(), self.y.to_bits()))
    }

    /// Pickles as `Point(x, y)`.
    fn __reduce__(&self, py: Python<'_>) -> PyResult<(PyObject, (f64, f64))> {
        let cls = py.get_type::<Point>().into_any().unbind();
        Ok((cls, (self.x, self.y)))
    }
}

/// A geographic coordinate (angular position on an ellipsoid), in decimal degrees.
#[pyclass]
#[derive(Clone, Copy)]
pub struct GeoPoint {
    /// Latitude in decimal degrees, positive north.
    #[pyo3(get, set)]
    pub lat_deg: f64,
    /// Longitude in decimal degrees, positive east.
    #[pyo3(get, set)]
    pub lon_deg: f64,
}

#[pymethods]
impl GeoPoint {
    /// Creates a `GeoPoint` from decimal degrees (latitude first, then longitude).
    #[new]
    fn new(lat_deg: f64, lon_deg: f64) -> Self {
        Self { lat_deg, lon_deg }
    }

    fn __repr__(&self) -> String {
        format!("GeoPoint(lat_deg={}, lon_deg={})", self.lat_deg, self.lon_deg)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, GeoPoint>>().is_ok_and(|o| {
            self.lat_deg.to_bits() == o.lat_deg.to_bits()
                && self.lon_deg.to_bits() == o.lon_deg.to_bits()
        })
    }

    fn __hash__(&self) -> isize {
        hash_of(&(self.lat_deg.to_bits(), self.lon_deg.to_bits()))
    }

    /// Pickles as `GeoPoint(lat_deg, lon_deg)`.
    fn __reduce__(&self, py: Python<'_>) -> PyResult<(PyObject, (f64, f64))> {
        let cls = py.get_type::<GeoPoint>().into_any().unbind();
        Ok((cls, (self.lat_deg, self.lon_deg)))
    }
}

/// Result of the geodesic inverse problem: the distance and bearings between two points.
#[pyclass(name = "InverseResult")]
#[derive(Clone)]
pub struct PyInverseResult {
    /// Ellipsoidal (geodesic) distance between the two points, in meters.
    #[pyo3(get)]
    pub distance_m: f64,
    /// Initial bearing at the first point, in decimal degrees clockwise from north.
    #[pyo3(get)]
    pub initial_bearing_deg: f64,
    /// Bearing at the second point, in decimal degrees clockwise from north.
    #[pyo3(get)]
    pub final_bearing_deg: f64,
}

#[pymethods]
impl PyInverseResult {
    fn __repr__(&self) -> String {
        format!(
            "InverseResult(distance_m={}, initial_bearing_deg={}, final_bearing_deg={})",
            self.distance_m, self.initial_bearing_deg, self.final_bearing_deg
        )
    }
}

/// A parsed OGC Simple Features geometry (owned, backed by `Vec`s).
#[pyclass]
#[derive(Clone)]
pub struct Geometry(pub(crate) InnerGeometry);

#[pymethods]
impl Geometry {
    /// Parses a WKT (Well-Known Text) geometry string.
    #[classmethod]
    fn from_wkt(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Ok(Geometry(tpt_gis_io::wkt::parse_geometry(s).map_err(to_pyerr)?))
    }

    /// Parses a WKB (Well-Known Binary) geometry from bytes.
    #[classmethod]
    fn from_wkb(_cls: &Bound<'_, PyType>, b: &[u8]) -> PyResult<Self> {
        Ok(Geometry(tpt_gis_io::wkb::parse_geometry(b).map_err(to_pyerr)?))
    }

    /// Parses a GeoJSON geometry object from a JSON string.
    #[classmethod]
    fn from_geojson(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Ok(Geometry(tpt_gis_io::geojson::parse_geometry(s).map_err(to_pyerr)?))
    }

    /// Serializes this geometry to its WKT (Well-Known Text) representation.
    fn to_wkt(&self) -> String {
        tpt_gis_io::wkt::write_geometry(&self.0)
    }

    /// Serializes this geometry to its WKB (Well-Known Binary) representation.
    fn to_wkb(&self) -> Vec<u8> {
        tpt_gis_io::wkb::write_geometry(&self.0)
    }

    /// Serializes this geometry to a GeoJSON geometry object string.
    fn to_geojson(&self) -> String {
        tpt_gis_io::geojson::write_geometry(&self.0)
    }

    /// The OGC geometry type name ("Point", "Polygon", "GeometryCollection", ...).
    #[getter]
    fn geometry_type(&self) -> &'static str {
        match &self.0 {
            InnerGeometry::Point(_) => "Point",
            InnerGeometry::LineString(_) => "LineString",
            InnerGeometry::Polygon(_) => "Polygon",
            InnerGeometry::MultiPoint(_) => "MultiPoint",
            InnerGeometry::MultiLineString(_) => "MultiLineString",
            InnerGeometry::MultiPolygon(_) => "MultiPolygon",
            InnerGeometry::GeometryCollection(_) => "GeometryCollection",
        }
    }

    /// Returns `True` if the planar point `(x, y)` lies inside this geometry.
    ///
    /// Works for `Polygon` and `MultiPolygon` (point-in-polygon with holes); returns
    /// `False` for all other geometry types.
    fn contains_point(&self, x: f64, y: f64) -> bool {
        let p = GeomPoint::new(x, y);
        match &self.0 {
            InnerGeometry::Polygon(poly) => {
                let mut scratch = Vec::new();
                poly.as_geom(&mut scratch).contains_point(p)
            }
            InnerGeometry::MultiPolygon(polys) => polys.iter().any(|poly| {
                let mut scratch = Vec::new();
                poly.as_geom(&mut scratch).contains_point(p)
            }),
            _ => false,
        }
    }

    fn __repr__(&self) -> String {
        format!("Geometry({})", self.geometry_type())
    }

    /// Equal when the two geometries have the same structure and coordinates.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, Geometry>>().is_ok_and(|o| self.0 == o.0)
    }

    fn __hash__(&self) -> isize {
        hash_of(&self.to_wkb())
    }

    /// Pickles as the geometry's WKB, rebuilt with `Geometry.from_wkb`.
    fn __reduce__(&self, py: Python<'_>) -> PyResult<(PyObject, (PyObject,))> {
        let from_wkb = py.get_type::<Geometry>().getattr("from_wkb")?.unbind();
        let bytes = PyBytes::new(py, &self.to_wkb()).into_any().unbind();
        Ok((from_wkb, (bytes,)))
    }

    /// The GeoJSON-geometry mapping, for the `__geo_interface__` protocol (used by
    /// geopandas, shapely, and similar libraries).
    #[getter]
    fn __geo_interface__(&self, py: Python<'_>) -> PyResult<PyObject> {
        let json = py.import("json")?;
        Ok(json.call_method1("loads", (self.to_geojson(),))?.unbind())
    }

    /// The bounding box `(min_x, min_y, max_x, max_y)`, or `None` if there are no
    /// coordinates.
    #[getter]
    fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut pts = Vec::new();
        collect_points(&self.0, &mut pts);
        pts.iter().fold(None, |acc, p| {
            Some(match acc {
                None => (p.x, p.y, p.x, p.y),
                Some((a, b, c, d)) => (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
            })
        })
    }

    /// Planar area of the polygonal parts (zero for points and lines).
    #[getter]
    fn area(&self) -> f64 {
        planar_area(&self.0)
    }

    /// Planar length: the length of lines, or the perimeter of polygons (holes
    /// included). Zero for points.
    #[getter]
    fn length(&self) -> f64 {
        planar_length(&self.0)
    }
}

/// Geodesic (ellipsoidal) distance between two geographic points, in meters.
#[pyfunction(signature = (a, b, ellipsoid="wgs84"))]
fn geodesic_distance(a: &GeoPoint, b: &GeoPoint, ellipsoid: &str) -> PyResult<f64> {
    let ell = parse_ellipsoid(ellipsoid)?;
    let p1 = CoreGeoPoint::new(a.lat_deg, a.lon_deg);
    let p2 = CoreGeoPoint::new(b.lat_deg, b.lon_deg);
    match geodesic::inverse(&ell, p1, p2) {
        Ok(r) => Ok(r.distance_m),
        Err(e) => Err(geodesy_err(e)),
    }
}

/// Solves the geodesic inverse problem: distance and initial/final bearings, in meters/degrees.
#[pyfunction(signature = (a, b, ellipsoid="wgs84"))]
fn geodesic_inverse(a: &GeoPoint, b: &GeoPoint, ellipsoid: &str) -> PyResult<PyInverseResult> {
    let ell = parse_ellipsoid(ellipsoid)?;
    let p1 = CoreGeoPoint::new(a.lat_deg, a.lon_deg);
    let p2 = CoreGeoPoint::new(b.lat_deg, b.lon_deg);
    match geodesic::inverse(&ell, p1, p2) {
        Ok(r) => Ok(PyInverseResult {
            distance_m: r.distance_m,
            initial_bearing_deg: r.initial_bearing_deg,
            final_bearing_deg: r.final_bearing_deg,
        }),
        Err(e) => Err(geodesy_err(e)),
    }
}

/// Solves the geodesic direct problem: the destination point reached from `start` along
/// `initial_bearing_deg` after travelling `distance_m` meters.
#[pyfunction(signature = (start, initial_bearing_deg, distance_m, ellipsoid="wgs84"))]
fn geodesic_destination(
    start: &GeoPoint,
    initial_bearing_deg: f64,
    distance_m: f64,
    ellipsoid: &str,
) -> PyResult<GeoPoint> {
    let ell = parse_ellipsoid(ellipsoid)?;
    let p0 = CoreGeoPoint::new(start.lat_deg, start.lon_deg);
    let dest = geodesic::direct(&ell, p0, initial_bearing_deg, distance_m)
        .map_err(geodesy_err)?
        .destination;
    Ok(GeoPoint { lat_deg: dest.lat_deg, lon_deg: dest.lon_deg })
}

/// Reads every geometry record from a `.shp` file (the `.shx` index file is not needed).
#[pyfunction]
fn read_shapefile_geometry(py: Python<'_>, path: &str) -> PyResult<Vec<Geometry>> {
    // Release the GIL for the file read and the parse, so other Python threads run.
    let geoms = py.allow_threads(|| -> Result<_, PyErr> {
        let bytes = std::fs::read(path).map_err(|e| PyIOError::new_err(e.to_string()))?;
        tpt_gis_io::shapefile::read_shp(&bytes).map_err(to_pyerr)
    })?;
    Ok(geoms.into_iter().map(Geometry).collect())
}

/// Reads every attribute record from a `.dbf` file, returning a list of `{field: value}` dicts.
#[pyfunction]
fn read_shapefile_attributes(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    // The .cpg sidecar names the text encoding; without one, UTF-8 (lossy) is used.
    let records = py.allow_threads(|| -> Result<_, PyErr> {
        let bytes = std::fs::read(path).map_err(|e| PyIOError::new_err(e.to_string()))?;
        let encoding = std::fs::read_to_string(std::path::Path::new(path).with_extension("cpg"))
            .ok()
            .and_then(|text| tpt_gis_io::shapefile::encoding_from_cpg(&text))
            .unwrap_or(tpt_gis_io::shapefile::DbfEncoding::Utf8);
        tpt_gis_io::shapefile::read_dbf_with(&bytes, encoding).map_err(to_pyerr)
    })?;
    Python::with_gil(|py| {
        let mut rows: Vec<Bound<'_, PyDict>> = Vec::with_capacity(records.len());
        for rec in &records {
            let dict = PyDict::new(py);
            for (name, val) in &rec.values {
                let obj: Bound<'_, pyo3::PyAny> = match val {
                    DbfValue::Character(s) => s.clone().into_pyobject(py)?.into_any(),
                    DbfValue::Numeric(n) => n.into_pyobject(py)?.into_any(),
                    DbfValue::Logical(l) => l.into_pyobject(py)?.into_any(),
                    DbfValue::Date { year, month, day } => {
                        format!("{year:04}-{month:02}-{day:02}").into_pyobject(py)?.into_any()
                    }
                };
                dict.set_item(name, obj)?;
            }
            rows.push(dict);
        }
        let list = PyList::new(py, rows)?;
        Ok(list.into_any().unbind())
    })
}

/// The native extension module, `tpt_gis_py._core`. The `tpt_gis_py` package re-exports it.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("TptGisError", py.get_type::<TptGisError>())?;
    m.add("GeometryParseError", py.get_type::<GeometryParseError>())?;
    m.add("GeodesyError", py.get_type::<GeodesyError>())?;
    m.add_class::<Geometry>()?;
    m.add_class::<Point>()?;
    m.add_class::<GeoPoint>()?;
    m.add_class::<PyInverseResult>()?;
    m.add_function(wrap_pyfunction!(geodesic_distance, m)?)?;
    m.add_function(wrap_pyfunction!(geodesic_inverse, m)?)?;
    m.add_function(wrap_pyfunction!(geodesic_destination, m)?)?;
    m.add_function(wrap_pyfunction!(read_shapefile_geometry, m)?)?;
    m.add_function(wrap_pyfunction!(read_shapefile_attributes, m)?)?;
    // Report the public package name, not the private extension module. Pickle looks
    // classes up by module and name, so this also makes the classes picklable.
    for ty in [
        "TptGisError",
        "GeometryParseError",
        "GeodesyError",
        "Geometry",
        "Point",
        "GeoPoint",
        "InverseResult",
    ] {
        m.getattr(ty)?.setattr("__module__", "tpt_gis_py")?;
    }
    Ok(())
}
