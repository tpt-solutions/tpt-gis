"""Python bindings for tpt-gis: geometry parsing, shapefile reading, and geodesy.

The compiled extension lives in ``tpt_gis_py._core``; everything it exports is
re-exported here so ``import tpt_gis_py`` is the public entry point.
"""

from ._core import *  # noqa: F401,F403
from ._core import __doc__ as _core_doc  # noqa: F401

__all__ = [
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
