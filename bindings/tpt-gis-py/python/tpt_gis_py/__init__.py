"""Python bindings for tpt-gis: geometry parsing, shapefile reading, and geodesy.

The compiled extension lives in ``tpt_gis_py._core``; everything it exports is
re-exported here so ``import tpt_gis_py`` is the public entry point.
"""

from ._core import *  # noqa: F401,F403
from ._core import __doc__ as _core_doc  # noqa: F401

from typing import Any

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
    "coordinates",
]


def coordinates(geometry: "Geometry") -> "numpy.ndarray":  # noqa: F821
    """Every vertex of ``geometry`` as an ``(n, 2)`` float64 NumPy array of ``(x, y)``.

    Rings are included in order: a polygon's exterior first, then its holes. A geometry with no
    vertices (such as an empty multi-part geometry) gives an array with zero rows. NumPy is
    imported when this function is called, so the package works without it.

    Raises:
        ImportError: if NumPy is not installed.
    """
    try:
        import numpy as np
    except ImportError as err:  # pragma: no cover - depends on the environment
        raise ImportError("coordinates() needs NumPy: install it with `pip install numpy`") from err

    rows: list[list[float]] = []
    _collect_pairs(geometry.__geo_interface__, rows)
    return np.asarray(rows, dtype=np.float64).reshape(-1, 2)


def _collect_pairs(node: Any, rows: list[list[float]]) -> None:
    """Appends the ``(x, y)`` pairs under a ``__geo_interface__`` node, depth first."""
    if node.get("type") == "GeometryCollection":
        for member in node["geometries"]:
            _collect_pairs(member, rows)
        return
    _walk(node["coordinates"], rows)


def _walk(value: Any, rows: list[list[float]]) -> None:
    """A coordinate is a flat list of numbers; anything else is a list of nested lists."""
    if value and isinstance(value[0], (int, float)):
        rows.append([float(value[0]), float(value[1])])
        return
    for item in value:
        _walk(item, rows)
