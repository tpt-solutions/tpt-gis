"""Writes tests/fixtures/places.zip: a shapefile (with .cpg, .prj and .shx) inside a folder, written
by GDAL through pyogrio, so the archive is a real producer's output.

The layer has three points with a UTF-8 name, so the .cpg encoding matters, and a numeric value.

Run with the scratchpad venv on PYTHONPATH:
  PYTHONPATH=<venv>/Lib/site-packages python generate_zipped_shapefile.py <output.zip>
"""

import os
import shutil
import sys
import tempfile
import zipfile

import numpy as np
import shapely
from pyogrio.raw import write

FEATURES = [
    ("Zoë", 1.5, shapely.Point(-3.2, 55.9)),
    ("Łódź", 2.5, shapely.Point(19.5, 51.8)),
    ("Plain", 3.5, shapely.Point(-0.1, 51.5)),
]


def main():
    out_zip = sys.argv[1]
    work = tempfile.mkdtemp()
    try:
        folder = os.path.join(work, "nested")
        os.makedirs(folder)
        shp = os.path.join(folder, "places.shp")
        geometry = np.array([shapely.to_wkb(g) for _, _, g in FEATURES], dtype=object)
        names = np.array([n for n, _, _ in FEATURES], dtype=object)
        values = np.array([v for _, v, _ in FEATURES], dtype=np.float64)
        write(
            shp,
            geometry=geometry,
            field_data=[names, values],
            fields=["name", "value"],
            geometry_type="Point",
            crs="EPSG:4326",
            driver="ESRI Shapefile",
            encoding="UTF-8",
        )
        with zipfile.ZipFile(out_zip, "w", zipfile.ZIP_DEFLATED) as archive:
            for entry in sorted(os.listdir(folder)):
                archive.write(os.path.join(folder, entry), os.path.join("nested", entry))
        print("wrote", out_zip, sorted(zipfile.ZipFile(out_zip).namelist()))
    finally:
        shutil.rmtree(work)


if __name__ == "__main__":
    main()
