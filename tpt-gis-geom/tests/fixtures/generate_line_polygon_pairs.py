# Generates geos_line_polygon.csv. Run: python generate_line_polygon_pairs.py geos_line_polygon.csv (needs shapely).
import sys, math, random
import shapely
from shapely.geometry import Polygon, LineString
OUT = sys.argv[1]
rng = random.Random(99)

def star(cx, cy, n, rmin, rmax):
    angles = sorted(rng.uniform(0, 2 * math.pi) for _ in range(n))
    pts = [(round(cx + rng.uniform(rmin, rmax) * math.cos(a), 1), round(cy + rng.uniform(rmin, rmax) * math.sin(a), 1)) for a in angles]
    return Polygon(pts)

def rand_poly():
    while True:
        p = star(rng.uniform(20, 80), rng.uniform(20, 80), rng.randint(4, 10), 6, 25)
        if p.is_valid and p.area > 1:
            return p

def rand_line():
    n = rng.randint(2, 6)
    return [(round(rng.uniform(0, 100), 1), round(rng.uniform(0, 100), 1)) for _ in range(n)]

def enc_ring(coords):
    return " ".join(f"{x:.1f},{y:.1f}" for x, y in coords)

rows = []
while len(rows) < 500:
    poly = rand_poly()
    kind = rng.random()
    if kind < 0.3:
        # A line through the polygon's vertices and edge midpoints: boundary contact cases.
        ring = list(poly.exterior.coords[:-1])
        pts = [rng.choice(ring) for _ in range(rng.randint(2, 4))]
        pts = [(round(x, 1), round(y, 1)) for x, y in pts]
        if len(set(pts)) < 2:
            continue
    else:
        pts = rand_line()
    line = LineString(pts)
    if line.length == 0:
        continue
    rows.append(";".join([
        enc_ring(poly.exterior.coords[:-1]),
        " ".join(f"{x:.1f},{y:.1f}" for x, y in pts),
        str(int(line.intersects(poly))),
        str(int(line.crosses(poly))),
        str(int(line.within(poly))),
    ]))
with open(OUT, "w", newline="\n") as f:
    f.write("# polygon_exterior;line_points;intersects;crosses;within  (GEOS via shapely)\n")
    f.write("\n".join(rows) + "\n")
print("rows", len(rows), "crosses", sum(r.endswith(";1") for r in rows), "intersects", sum(r.split(";")[2] == "1" for r in rows))
