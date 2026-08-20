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
use pyo3::Bound;
use pyo3::types::{PyDict, PyList, PyType};

use tpt_gis_core::geodesic::{self, GeodesicError};
use tpt_gis_core::{Ellipsoid, GeoPoint as CoreGeoPoint};
use tpt_gis_geom::Point as GeomPoint;
use tpt_gis_io::geometry::Geometry as InnerGeometry;
use tpt_gis_io::shapefile::DbfValue;

/// Converts any of `tpt-gis`'s error types into a Python `ValueError`.
fn to_pyerr<E: std::fmt::Display>(e: E) -> PyErr {
    PyValueError::new_err(e.to_string())
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
        Ok(Geometry(
            tpt_gis_io::wkt::parse_geometry(s).map_err(to_pyerr)?,
        ))
    }

    /// Parses a WKB (Well-Known Binary) geometry from bytes.
    #[classmethod]
    fn from_wkb(_cls: &Bound<'_, PyType>, b: &[u8]) -> PyResult<Self> {
        Ok(Geometry(
            tpt_gis_io::wkb::parse_geometry(b).map_err(to_pyerr)?,
        ))
    }

    /// Parses a GeoJSON geometry object from a JSON string.
    #[classmethod]
    fn from_geojson(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Ok(Geometry(
            tpt_gis_io::geojson::parse_geometry(s).map_err(to_pyerr)?,
        ))
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
}

/// Geodesic (ellipsoidal) distance between two geographic points, in meters.
#[pyfunction(signature = (a, b, ellipsoid="wgs84"))]
fn geodesic_distance(a: &GeoPoint, b: &GeoPoint, ellipsoid: &str) -> PyResult<f64> {
    let ell = parse_ellipsoid(ellipsoid)?;
    let p1 = CoreGeoPoint::new(a.lat_deg, a.lon_deg);
    let p2 = CoreGeoPoint::new(b.lat_deg, b.lon_deg);
    match geodesic::inverse(&ell, p1, p2) {
        Ok(r) => Ok(r.distance_m),
        Err(GeodesicError::ConvergenceFailure) => Err(PyValueError::new_err(
            "geodesic inverse failed to converge (near-antipodal points)",
        )),
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
        Err(GeodesicError::ConvergenceFailure) => Err(PyValueError::new_err(
            "geodesic inverse failed to converge (near-antipodal points)",
        )),
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
    let dest = geodesic::direct(&ell, p0, initial_bearing_deg, distance_m).destination;
    Ok(GeoPoint {
        lat_deg: dest.lat_deg,
        lon_deg: dest.lon_deg,
    })
}

/// Reads every geometry record from a `.shp` file (the `.shx` index file is not needed).
#[pyfunction]
fn read_shapefile_geometry(path: &str) -> PyResult<Vec<Geometry>> {
    let bytes = std::fs::read(path).map_err(|e| PyIOError::new_err(e.to_string()))?;
    let geoms = tpt_gis_io::shapefile::read_shp(&bytes).map_err(to_pyerr)?;
    Ok(geoms.into_iter().map(Geometry).collect())
}

/// Reads every attribute record from a `.dbf` file, returning a list of `{field: value}` dicts.
#[pyfunction]
fn read_shapefile_attributes(path: &str) -> PyResult<PyObject> {
    let bytes = std::fs::read(path).map_err(|e| PyIOError::new_err(e.to_string()))?;
    let records = tpt_gis_io::shapefile::read_dbf(&bytes).map_err(to_pyerr)?;
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

/// The `tpt_gis_py` extension module.
#[pymodule]
fn tpt_gis_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Geometry>()?;
    m.add_class::<Point>()?;
    m.add_class::<GeoPoint>()?;
    m.add_class::<PyInverseResult>()?;
    m.add_function(wrap_pyfunction!(geodesic_distance, m)?)?;
    m.add_function(wrap_pyfunction!(geodesic_inverse, m)?)?;
    m.add_function(wrap_pyfunction!(geodesic_destination, m)?)?;
    m.add_function(wrap_pyfunction!(read_shapefile_geometry, m)?)?;
    m.add_function(wrap_pyfunction!(read_shapefile_attributes, m)?)?;
    Ok(())
}
