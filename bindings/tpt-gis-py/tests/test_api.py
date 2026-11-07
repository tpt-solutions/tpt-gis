"""Tests for the value semantics, geometry properties, exceptions, and pickling."""

import math
import pickle

import pytest

import tpt_gis_py as gis


SQUARE = "POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))"


def test_exception_hierarchy_keeps_value_error_compatibility():
    assert issubclass(gis.TptGisError, ValueError)
    assert issubclass(gis.GeometryParseError, gis.TptGisError)
    assert issubclass(gis.GeodesyError, gis.TptGisError)
    with pytest.raises(gis.GeometryParseError):
        gis.Geometry.from_wkt("NOT A GEOMETRY")
    with pytest.raises(ValueError):
        gis.Geometry.from_wkt("NOT A GEOMETRY")


def test_exception_module_name_is_the_public_package():
    assert gis.GeodesyError.__module__ == "tpt_gis_py"


def test_geodesy_errors_use_geodesy_error():
    with pytest.raises(gis.GeodesyError):
        gis.geodesic_distance(gis.GeoPoint(float("nan"), 0.0), gis.GeoPoint(0.0, 0.0))


def test_geometry_equality_and_hash():
    a = gis.Geometry.from_wkt(SQUARE)
    b = gis.Geometry.from_wkt(SQUARE)
    c = gis.Geometry.from_wkt("POINT (1 2)")
    assert a == b
    assert a != c
    assert hash(a) == hash(b)
    assert len({a, b, c}) == 2
    assert a != "POLYGON"


def test_point_value_semantics():
    assert gis.Point(1.0, 2.0) == gis.Point(1.0, 2.0)
    assert gis.Point(1.0, 2.0) != gis.Point(2.0, 1.0)
    assert len({gis.Point(1.0, 2.0), gis.Point(1.0, 2.0)}) == 1
    assert gis.GeoPoint(1.0, 2.0) == gis.GeoPoint(1.0, 2.0)
    assert hash(gis.GeoPoint(1.0, 2.0)) == hash(gis.GeoPoint(1.0, 2.0))


def test_geo_interface_is_a_geojson_mapping():
    point = gis.Geometry.from_wkt("POINT (30 10)")
    assert point.__geo_interface__ == {"type": "Point", "coordinates": [30.0, 10.0]}
    polygon = gis.Geometry.from_wkt(SQUARE).__geo_interface__
    assert polygon["type"] == "Polygon"
    assert len(polygon["coordinates"][0]) == 5


def test_bounds_area_and_length():
    square = gis.Geometry.from_wkt(SQUARE)
    assert square.bounds == (0.0, 0.0, 10.0, 10.0)
    assert square.area == pytest.approx(100.0)
    assert square.length == pytest.approx(40.0)
    line = gis.Geometry.from_wkt("LINESTRING (0 0, 3 4)")
    assert line.length == pytest.approx(5.0)
    assert line.area == 0.0
    assert line.bounds == (0.0, 0.0, 3.0, 4.0)


def test_holes_reduce_area_and_add_perimeter():
    holed = gis.Geometry.from_wkt(
        "POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0), (4 4, 6 4, 6 6, 4 6, 4 4))"
    )
    assert holed.area == pytest.approx(96.0)
    assert holed.length == pytest.approx(40.0 + 8.0)


def test_multipolygon_area_is_the_sum_of_parts():
    multi = gis.Geometry.from_wkt(
        "MULTIPOLYGON (((0 0, 1 0, 1 1, 0 1, 0 0)), ((5 5, 7 5, 7 7, 5 7, 5 5)))"
    )
    assert multi.area == pytest.approx(1.0 + 4.0)


def test_point_has_no_area_or_length():
    point = gis.Geometry.from_wkt("POINT (1 2)")
    assert point.area == 0.0
    assert point.length == 0.0
    assert point.bounds == (1.0, 2.0, 1.0, 2.0)


def test_pickle_round_trips_geometry_and_points():
    geom = gis.Geometry.from_wkt(SQUARE)
    restored = pickle.loads(pickle.dumps(geom))
    assert restored == geom
    assert restored.to_wkt() == geom.to_wkt()

    assert pickle.loads(pickle.dumps(gis.Point(1.5, -2.0))) == gis.Point(1.5, -2.0)
    assert pickle.loads(pickle.dumps(gis.GeoPoint(51.5, -0.1))) == gis.GeoPoint(51.5, -0.1)


def test_empty_geometry_has_no_bounds():
    empty = gis.Geometry.from_geojson('{"type": "GeometryCollection", "geometries": []}')
    assert empty.bounds is None
    assert math.isclose(empty.area, 0.0)


def test_coordinates_returns_every_vertex_as_an_array():
    np = pytest.importorskip("numpy")
    from tpt_gis_py import Geometry, coordinates

    point = Geometry.from_wkt("POINT (1 2)")
    assert coordinates(point).tolist() == [[1.0, 2.0]]

    line = Geometry.from_wkt("LINESTRING (0 0, 3 4, 6 0)")
    arr = coordinates(line)
    assert arr.dtype == np.float64 and arr.shape == (3, 2)
    assert arr.tolist() == [[0.0, 0.0], [3.0, 4.0], [6.0, 0.0]]


def test_coordinates_lists_the_exterior_then_the_holes():
    pytest.importorskip("numpy")
    from tpt_gis_py import Geometry, coordinates

    poly = Geometry.from_wkt(
        "POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0), (2 2, 4 2, 4 4, 2 4, 2 2))"
    )
    arr = coordinates(poly)
    assert arr.shape == (10, 2)
    assert arr[0].tolist() == [0.0, 0.0] and arr[5].tolist() == [2.0, 2.0]


def test_coordinates_of_a_collection_walks_its_members():
    pytest.importorskip("numpy")
    from tpt_gis_py import Geometry, coordinates

    gc = Geometry.from_wkt("GEOMETRYCOLLECTION (POINT (7 8), LINESTRING (1 1, 2 2))")
    assert coordinates(gc).tolist() == [[7.0, 8.0], [1.0, 1.0], [2.0, 2.0]]
