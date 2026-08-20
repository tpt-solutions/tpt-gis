import struct
import tempfile
import os

import pytest

import tpt_gis_py as gis


def test_wkt_round_trip():
    geom = gis.Geometry.from_wkt("POINT (30 10)")
    assert geom.geometry_type == "Point"
    assert geom.to_wkt() == "POINT (30 10)"


def test_wkb_round_trip():
    geom = gis.Geometry.from_wkt("LINESTRING (30 10, 10 30, 40 40)")
    wkb = geom.to_wkb()
    assert isinstance(wkb, bytes)
    assert gis.Geometry.from_wkb(wkb).to_wkt() == geom.to_wkt()


def test_geojson_round_trip():
    geom = gis.Geometry.from_wkt("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))")
    gj = geom.to_geojson()
    assert '"type":"Polygon"' in gj or '"type": "Polygon"' in gj
    assert gis.Geometry.from_geojson(gj).to_wkt() == geom.to_wkt()


def test_contains_point():
    poly = gis.Geometry.from_wkt("POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))")
    assert poly.contains_point(5.0, 5.0)
    assert not poly.contains_point(15.0, 5.0)
    # Geometry types that aren't (multi-)polygons never contain a point.
    point = gis.Geometry.from_wkt("POINT (1 1)")
    assert not point.contains_point(1.0, 1.0)


def test_multi_polygon_contains_point():
    mp = gis.Geometry.from_wkt(
        "MULTIPOLYGON (((0 0, 2 0, 2 2, 0 2, 0 0)), ((10 10, 12 10, 12 12, 10 12, 10 10)))"
    )
    assert mp.contains_point(1.0, 1.0)
    assert mp.contains_point(11.0, 11.0)
    assert not mp.contains_point(5.0, 5.0)


def _dms(deg, minute, sec):
    return deg + minute / 60.0 + sec / 3600.0


def test_geodesic_distance_flinders_to_buninyong():
    # Vincenty (1975) reference test vector; distance ~54972.271 m on WGS84.
    flinders = gis.GeoPoint(-_dms(37, 57, 3.72030), _dms(144, 25, 29.52440))
    buninyong = gis.GeoPoint(-_dms(37, 39, 10.15610), _dms(143, 55, 35.38390))
    dist = gis.geodesic_distance(flinders, buninyong)
    assert abs(dist - 54972.271) < 1e-3
    # GRS80 is numerically near-identical; should be in the same ballpark.
    dist_grs = gis.geodesic_distance(flinders, buninyong, "grs80")
    assert abs(dist_grs - dist) < 1.0


def test_geodesic_inverse_and_destination():
    flinders = gis.GeoPoint(-_dms(37, 57, 3.72030), _dms(144, 25, 29.52440))
    buninyong = gis.GeoPoint(-_dms(37, 39, 10.15610), _dms(143, 55, 35.38390))
    result = gis.geodesic_inverse(flinders, buninyong)
    assert abs(result.distance_m - 54972.271) < 1e-3
    assert 300.0 < result.initial_bearing_deg < 310.0
    # The direct problem applied to the inverse's output should recover the destination.
    dest = gis.geodesic_destination(
        flinders, result.initial_bearing_deg, result.distance_m
    )
    assert abs(dest.lat_deg - buninyong.lat_deg) < 1e-4
    assert abs(dest.lon_deg - buninyong.lon_deg) < 1e-4


def test_unknown_ellipsoid_errors():
    a = gis.GeoPoint(0.0, 0.0)
    b = gis.GeoPoint(1.0, 1.0)
    with pytest.raises(ValueError):
        gis.geodesic_distance(a, b, "mars")


def test_invalid_wkt_errors():
    with pytest.raises(ValueError):
        gis.Geometry.from_wkt("POINT EMPTY")


def _build_point_shp(path):
    header = struct.pack(">i", 9994)
    header += b"\x00" * 20
    header += struct.pack(">i", 0)  # file length (unused by reader)
    header += struct.pack(">i", 1000)  # version
    header += struct.pack("<i", 1)  # shape type = Point
    header += b"\x00" * 64  # bbox + z/m
    assert len(header) == 100

    content = struct.pack("<i", 1)  # record shape type
    content += struct.pack("<d", 12.5)  # x
    content += struct.pack("<d", -34.25)  # y
    record = struct.pack(">i", 1)  # record number
    record += struct.pack(">i", len(content) // 2)  # content length in 16-bit words
    record += content
    with open(path, "wb") as f:
        f.write(header + record)


def _build_dbf(path):
    # One field "NAME" (Character, length 10), one record "Alice".
    header = bytes([0x03, 0, 0, 0])  # version + 3 date bytes
    header += struct.pack("<i", 1)  # num records
    header_bytes = 32 + 32 + 1
    record_bytes = 1 + 10
    header += struct.pack("<H", header_bytes)
    header += struct.pack("<H", record_bytes)
    header += b"\x00" * 20
    assert len(header) == 32

    name = b"NAME" + b"\x00" * 7  # 11-byte field name
    field = name + b"C" + b"\x00" * 4 + bytes([10, 0]) + b"\x00" * 14
    assert len(field) == 32

    record = bytes([0x20])  # not deleted
    record += b"Alice     "  # 10 bytes

    with open(path, "wb") as f:
        f.write(header + field + b"\x0D" + record)


def test_read_shapefile_geometry_and_attributes():
    with tempfile.TemporaryDirectory() as d:
        shp = os.path.join(d, "test.shp")
        dbf = os.path.join(d, "test.dbf")
        _build_point_shp(shp)
        _build_dbf(dbf)

        geoms = gis.read_shapefile_geometry(shp)
        assert len(geoms) == 1
        assert geoms[0].geometry_type == "Point"
        assert geoms[0].to_wkt() == "POINT (12.5 -34.25)"

        attrs = gis.read_shapefile_attributes(dbf)
        assert len(attrs) == 1
        assert attrs[0]["NAME"] == "Alice"
