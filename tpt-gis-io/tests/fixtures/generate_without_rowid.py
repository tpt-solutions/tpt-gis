"""Writes tests/fixtures/without_rowid.gpkg with Python's sqlite3, the reference SQLite.

The feature table is WITHOUT ROWID with an INTEGER primary key (which is not a rowid alias
in such a table), and is large enough to need interior index pages and an overflow chain.
Rows are inserted in shuffled key order so the index order differs from insertion order.

Run: python generate_without_rowid.py   (writes next to this script)
"""

import os
import random
import sqlite3
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "without_rowid.gpkg")
COUNT = 1500


def point_blob(x, y, srs_id=4326):
    # GeoPackage binary header: magic, version 0, flags (little-endian, no envelope), SRS id.
    header = b"GP" + bytes([0, 0x01]) + struct.pack("<i", srs_id)
    wkb = b"\x01" + struct.pack("<I", 1) + struct.pack("<dd", x, y)
    return header + wkb


def main():
    if os.path.exists(OUT):
        os.remove(OUT)
    db = sqlite3.connect(OUT)
    db.executescript(
        """
        CREATE TABLE gpkg_spatial_ref_sys (
            srs_name TEXT NOT NULL, srs_id INTEGER NOT NULL PRIMARY KEY,
            organization TEXT NOT NULL, organization_coordsys_id INTEGER NOT NULL,
            definition TEXT NOT NULL, description TEXT);
        INSERT INTO gpkg_spatial_ref_sys VALUES
            ('WGS 84', 4326, 'EPSG', 4326, 'GEOGCS["WGS 84"]', NULL);
        CREATE TABLE gpkg_contents (
            table_name TEXT NOT NULL PRIMARY KEY, data_type TEXT NOT NULL,
            identifier TEXT UNIQUE, description TEXT DEFAULT '',
            last_change DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            min_x DOUBLE, min_y DOUBLE, max_x DOUBLE, max_y DOUBLE, srs_id INTEGER);
        CREATE TABLE gpkg_geometry_columns (
            table_name TEXT NOT NULL, column_name TEXT NOT NULL,
            geometry_type_name TEXT NOT NULL, srs_id INTEGER NOT NULL,
            z TINYINT NOT NULL, m TINYINT NOT NULL,
            CONSTRAINT pk_geom_cols PRIMARY KEY (table_name, column_name));
        CREATE TABLE places (
            fid INTEGER NOT NULL PRIMARY KEY,
            name TEXT,
            geom BLOB
        ) WITHOUT ROWID;
        """
    )
    db.execute(
        "INSERT INTO gpkg_contents (table_name, data_type, identifier, srs_id) "
        "VALUES ('places', 'features', 'places', 4326)"
    )
    db.execute(
        "INSERT INTO gpkg_geometry_columns VALUES ('places', 'geom', 'POINT', 4326, 0, 0)"
    )

    ids = list(range(1, COUNT + 1))
    random.Random(7).shuffle(ids)
    rows = []
    for fid in ids:
        # Vary the name length so some rows are long, and make one row overflow a page.
        name = "place-%05d" % fid
        if fid % 97 == 0:
            name += "-" + "x" * 3000
        if fid == 1234:
            name = "y" * 12000
        rows.append((fid, name, point_blob(fid * 0.01, -fid * 0.02)))
    db.executemany("INSERT INTO places VALUES (?, ?, ?)", rows)
    db.commit()
    db.execute("VACUUM")
    db.close()
    print("wrote", OUT, os.path.getsize(OUT), "bytes")


if __name__ == "__main__":
    main()
