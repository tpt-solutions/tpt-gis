# Generates geos_line_line.csv. Run: python generate_line_line_pairs.py geos_line_line.csv (needs shapely).
import sys, random
from shapely.geometry import LineString
OUT = sys.argv[1]
rng = random.Random(5)
# Small grid, so that lines share vertices and cross at grid points.
grid = [(float(x), float(y)) for x in range(0, 6) for y in range(0, 6)]
def rand_line():
    n = rng.randint(2, 4)
    pts = []
    while len(pts) < n:
        p = rng.choice(grid)
        if not pts or p != pts[-1]:
            pts.append(p)
    return pts
def enc(pts):
    return " ".join(f"{x:.1f},{y:.1f}" for x, y in pts)
rows = []
while len(rows) < 600:
    a, b = rand_line(), rand_line()
    la, lb = LineString(a), LineString(b)
    if la.length == 0 or lb.length == 0:
        continue
    if rng.random() < 0.2:
        # Make b share a stretch with a, to exercise overlaps.
        b = [a[0], a[-1]] if rng.random() < 0.5 else a[: max(2, len(a) - 1)]
        lb = LineString(b)
        if lb.length == 0:
            continue
    rows.append(";".join([enc(a), enc(b), str(int(la.crosses(lb))), str(int(la.within(lb)))]))
with open(OUT, "w", newline="\n") as f:
    f.write("# line_a;line_b;crosses  (GEOS via shapely)\n")
    f.write("\n".join(rows) + "\n")
print("rows", len(rows), "crosses", sum(r.endswith(";1") for r in rows))
