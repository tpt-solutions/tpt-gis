"""Writes tests/fixtures/geos_de9im.csv: DE-9IM matrices from GEOS (via shapely) for random pairs
of points, line strings and polygons (with holes) on an integer grid, so that vertex contacts,
shared edges and collinear overlaps are common.

Row format:  kind_a;coords_a;kind_b;coords_b;matrix
  kind: P (point), L (line string), A (polygon)
  coords: parts separated by '|'; a part is points "x,y" separated by spaces. A polygon's
  first part is the exterior ring and the rest are holes. Rings are given open (no repeated
  first point), as shapely accepts them.

Run with the scratchpad venv on PYTHONPATH:
  PYTHONPATH=<venv>/Lib/site-packages python generate_de9im_pairs.py <output>
"""

import random
import sys

from shapely.geometry import LineString, MultiPoint, Point, Polygon

ROWS = 1200
GRID = 12


def rand_point(rng):
    return (rng.randint(0, GRID), rng.randint(0, GRID))


def rand_polygon(rng):
    while True:
        pts = [rand_point(rng) for _ in range(rng.randint(3, 7))]
        poly = Polygon(pts).convex_hull
        if poly.geom_type != "Polygon" or poly.area < 2:
            continue
        holes = []
        if rng.random() < 0.3:
            # A small square hole strictly inside the exterior, when one fits.
            minx, miny, maxx, maxy = poly.bounds
            cx = rng.randint(int(minx) + 1, int(maxx) - 1) if maxx - minx > 2 else None
            cy = rng.randint(int(miny) + 1, int(maxy) - 1) if maxy - miny > 2 else None
            if cx is not None and cy is not None:
                hole = Polygon([(cx, cy), (cx + 1, cy), (cx + 1, cy + 1), (cx, cy + 1)])
                if poly.contains(hole):
                    holes.append(hole)
        geom = Polygon(poly.exterior.coords, [h.exterior.coords for h in holes])
        if geom.is_valid and geom.area > 0:
            return geom


def rand_line(rng, poly=None):
    n = rng.randint(2, 4)
    pts = [rand_point(rng) for _ in range(n)]
    if poly is not None and rng.random() < 0.4:
        # Start from a polygon vertex so the line meets the boundary at a vertex.
        pts[0] = rng.choice(list(poly.exterior.coords)[:-1])
    line = LineString(pts)
    return line if line.length > 0 and line.is_valid else rand_line(rng, poly)


def enc_points(coords):
    return " ".join(f"{x},{y}" for x, y in coords)


def encode(geom):
    if geom.geom_type == "Point":
        return "P", enc_points([(geom.x, geom.y)])
    if geom.geom_type == "LineString":
        return "L", enc_points(geom.coords)
    rings = [geom.exterior.coords[:-1]] + [i.coords[:-1] for i in geom.interiors]
    return "A", "|".join(enc_points(r) for r in rings)


def random_geometry(rng, kind, other=None):
    if kind == "P":
        if other is not None and rng.random() < 0.5:
            # Put the point on the other geometry's vertices.
            verts = list(other.exterior.coords)[:-1] if other.geom_type == "Polygon" else list(other.coords)
            return Point(rng.choice(verts))
        return Point(rand_point(rng))
    if kind == "L":
        return rand_line(rng, other if other is not None and other.geom_type == "Polygon" else None)
    return rand_polygon(rng)


def main():
    out_path = sys.argv[1]
    rng = random.Random(2024)
    kinds = ["P", "L", "A"]
    rows = []
    while len(rows) < ROWS:
        ka, kb = rng.choice(kinds), rng.choice(kinds)
        a = random_geometry(rng, ka)
        b = random_geometry(rng, kb, a)
        matrix = a.relate(b)
        ta, ca = encode(a)
        tb, cb = encode(b)
        rows.append(f"{ta};{ca};{tb};{cb};{matrix}")
    with open(out_path, "w", encoding="utf-8", newline="\n") as f:
        f.write("# kind_a;coords_a;kind_b;coords_b;de9im (GEOS via shapely)\n")
        f.write("\n".join(rows) + "\n")
    print("wrote", len(rows), "rows to", out_path)


if __name__ == "__main__":
    main()
