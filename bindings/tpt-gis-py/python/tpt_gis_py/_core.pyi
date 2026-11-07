"""Type stubs for the tpt-gis native extension."""

from typing import Any, Literal, final

from typing_extensions import Self

_Ellipsoid = Literal["wgs84", "grs80"]

__all__ = [
    "TptGisError",
    "GeometryParseError",
    "GeodesyError",
    "Geometry",
    "Point",
    "GeoPoint",
    "InverseResult",
    "geodesic_distance",
    "geodesic_inverse",
    "geodesic_destination",
    "read_shapefile_geometry",
    "read_shapefile_attributes",
]


class TptGisError(ValueError):
    """Base class for tpt-gis errors. It subclasses ``ValueError``."""


class GeometryParseError(TptGisError):
    """A geometry string, buffer, or file could not be parsed."""


class GeodesyError(TptGisError):
    """A geodesic calculation got invalid input or did not converge."""


@final
class Point:
    """A planar (projected) coordinate, in the linear unit of its CRS (typically meters)."""

    x: float
    y: float

    def __new__(cls, x: float, y: float) -> Self: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __reduce__(self) -> tuple[type[Self], tuple[float, float]]: ...


@final
class GeoPoint:
    """A geographic coordinate (angular position on an ellipsoid), in decimal degrees."""

    lat_deg: float
    lon_deg: float

    def __new__(cls, lat_deg: float, lon_deg: float) -> Self: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __reduce__(self) -> tuple[type[Self], tuple[float, float]]: ...


@final
class InverseResult:
    """Result of the geodesic inverse problem: distance and bearings between two points."""

    @property
    def distance_m(self) -> float: ...
    @property
    def initial_bearing_deg(self) -> float: ...
    @property
    def final_bearing_deg(self) -> float: ...


@final
class Geometry:
    """A parsed OGC Simple Features geometry."""

    @classmethod
    def from_wkt(cls, s: str) -> "Geometry": ...
    @classmethod
    def from_wkb(cls, b: bytes) -> "Geometry": ...
    @classmethod
    def from_geojson(cls, s: str) -> "Geometry": ...

    @property
    def geometry_type(self) -> str:
        """The OGC type name: "Point", "Polygon", "GeometryCollection", and so on."""
        ...

    def to_wkt(self) -> str: ...
    def to_wkb(self) -> bytes: ...
    def to_geojson(self) -> str: ...
    def contains_point(self, x: float, y: float) -> bool: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __reduce__(self) -> tuple[Any, tuple[bytes]]: ...

    @property
    def __geo_interface__(self) -> dict[str, Any]:
        """The GeoJSON geometry mapping (the ``__geo_interface__`` protocol)."""
        ...

    @property
    def bounds(self) -> tuple[float, float, float, float] | None:
        """Bounding box ``(min_x, min_y, max_x, max_y)``, or None with no coordinates."""
        ...

    @property
    def area(self) -> float:
        """Planar area of the polygonal parts (zero for points and lines)."""
        ...

    @property
    def length(self) -> float:
        """Planar length of lines, or perimeter of polygons (holes included)."""
        ...


def geodesic_distance(a: GeoPoint, b: GeoPoint, ellipsoid: _Ellipsoid = "wgs84") -> float:
    """Ellipsoidal distance between two points, in meters."""
    ...


def geodesic_inverse(a: GeoPoint, b: GeoPoint, ellipsoid: _Ellipsoid = "wgs84") -> InverseResult:
    """Distance and initial and final bearings between two points."""
    ...


def geodesic_destination(
    start: GeoPoint,
    initial_bearing_deg: float,
    distance_m: float,
    ellipsoid: _Ellipsoid = "wgs84",
) -> GeoPoint:
    """The point reached from ``start`` along a bearing after ``distance_m`` meters."""
    ...


def read_shapefile_geometry(path: str) -> list[Geometry]:
    """Every geometry record in a ``.shp`` file."""
    ...


def read_shapefile_attributes(path: str) -> list[dict[str, Any]]:
    """Attribute rows from the ``.dbf`` file that sits next to a shapefile."""
    ...
