# Generates geos_pairs.csv. Run: python generate_geos_pairs.py geos_pairs.csv (needs shapely).
import sys, math, random
import shapely
from shapely.geometry import Polygon
OUT = sys.argv[1]
rng = random.Random(42)

def star(cx, cy, n, rmin, rmax):
    angles = sorted(rng.uniform(0, 2 * math.pi) for _ in range(n))
    pts = []
    for a in angles:
        r = rng.uniform(rmin, rmax)
        pts.append((round(cx + r * math.cos(a), 1), round(cy + r * math.sin(a), 1)))
    return Polygon(pts)

def rand_poly():
    while True:
        p = star(rng.uniform(20, 80), rng.uniform(20, 80), rng.randint(5, 12), 4, 25)
        if p.is_valid and p.area > 1:
            return p

def with_hole():
    while True:
        outer = star(rng.uniform(30, 70), rng.uniform(30, 70), rng.randint(6, 10), 15, 30)
        hole = Polygon([(c[0] * 0.4 + outer.centroid.x * 0.6, c[1] * 0.4 + outer.centroid.y * 0.6)
                        for c in outer.exterior.coords[:-1]])
        p = Polygon(outer.exterior.coords, [hole.exterior.coords])
        if p.is_valid and p.area > 1:
            return p

def pair():
    kind = rng.random()
    a = with_hole() if rng.random() < 0.25 else rand_poly()
    if kind < 0.25:
        b = rand_poly()
    elif kind < 0.5:
        dx, dy = rng.choice([-3, -1.5, 0, 1.5, 3]), rng.choice([-3, -1.5, 0, 1.5, 3])
        b = shapely.affinity.translate(a, dx, dy)
    elif kind < 0.7:
        b = shapely.affinity.scale(a, 0.5, 0.5, origin=a.centroid)
    elif kind < 0.85:
        b = shapely.affinity.scale(a, 1.6, 1.6, origin=a.centroid)
    else:
        b = with_hole()
    return a, b

def enc(poly):
    rings = [poly.exterior] + list(poly.interiors)
    out = []
    for r in rings:
        out.append(" ".join(f"{x:.1f},{y:.1f}" for x, y in r.coords[:-1]))
    return "|".join(out)

def area(g):
    return 0.0 if g.is_empty else g.area

import shapely.affinity  # noqa: E402
def round_poly(poly):
    # Round every ring to 0.1, the precision the CSV keeps, so GEOS sees the same geometry.
    rr = lambda ring: [(round(x, 1), round(y, 1)) for x, y in ring.coords[:-1]]
    return Polygon(rr(poly.exterior), [rr(h) for h in poly.interiors])

rows = []
while len(rows) < 400:
    a, b = pair()
    a, b = round_poly(a), round_poly(b)
    if not (a.is_valid and b.is_valid) or a.area <= 0 or b.area <= 0:
        continue
    inter = a.intersection(b)
    uni = a.union(b)
    diff = a.difference(b)
    rows.append(";".join([
        enc(a), enc(b),
        f"{area(a):.10f}", f"{area(b):.10f}",
        f"{a.convex_hull.area:.10f}",
        f"{a.centroid.x:.10f}", f"{a.centroid.y:.10f}",
        str(int(a.intersects(b))), str(int(a.covers(b))),
        str(int(a.touches(b))), str(int(a.overlaps(b))),
        f"{area(inter):.10f}", f"{area(uni):.10f}", f"{area(diff):.10f}",
    ]))
with open(OUT, "w", newline="\n") as f:
    f.write("# poly_a;poly_b;area_a;area_b;hull_area_a;centroid_a_x;centroid_a_y;intersects;covers_ab;touches;overlaps;intersection_area;union_area;difference_area\n")
    f.write("\n".join(rows) + "\n")
print("rows", len(rows))
