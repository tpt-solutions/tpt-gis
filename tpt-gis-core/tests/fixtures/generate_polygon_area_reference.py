# Generates geographiclib_polygon.csv from GeographicLib (WGS84). Run: python generate_polygon_area_reference.py geographiclib_polygon.csv (needs geographiclib).
import sys, math, random
from geographiclib.geodesic import Geodesic
from geographiclib.polygonarea import PolygonArea
OUT = sys.argv[1]
g = Geodesic.WGS84
rng = random.Random(77)
rows = []
def add(points, tag):
    pa = PolygonArea(g)
    for lat, lon in points:
        pa.AddPoint(lat, lon)
    n, perimeter, area = pa.Compute()
    enc = ";".join(f"{lat:.10f};{lon:.10f}" for lat, lon in points)
    rows.append(f"{tag};{len(points)};{enc};{perimeter:.9f};{area:.6f}")
# Small convex-ish polygons around random centres (counter-clockwise).
for _ in range(120):
    clat, clon = rng.uniform(-70, 70), rng.uniform(-170, 170)
    n = rng.randint(3, 12)
    angles = sorted(rng.uniform(0, 2 * math.pi) for _ in range(n))
    pts = [(clat + rng.uniform(0.05, 2.0) * math.sin(a), clon + rng.uniform(0.05, 2.0) * math.cos(a)) for a in angles]
    pts = [(max(-89.9, min(89.9, la)), lo) for la, lo in pts]
    add(pts, "local")
# Clockwise: the same vertices reversed.
for _ in range(30):
    clat, clon = rng.uniform(-60, 60), rng.uniform(-170, 170)
    pts = [(clat + rng.uniform(-1, 1), clon + rng.uniform(-1, 1)) for _ in range(5)]
    add(list(reversed(pts)), "cw")
# Crossing the antimeridian.
for _ in range(30):
    lat = rng.uniform(-50, 50)
    pts = [(lat - 1, 179.5), (lat + 1, 179.7), (lat + 0.5, -179.6), (lat - 0.5, -179.8), (lat - 1, 179.9)]
    add(pts, "dateline")
# Enclosing a pole.
for _ in range(15):
    lat = rng.uniform(70, 85)
    lons = [i * 60.0 for i in range(6)]
    pts = [(lat + rng.uniform(-0.2, 0.2), lo) for lo in lons]
    add(pts, "pole")
# Long edges across the globe, some self-intersecting.
for _ in range(30):
    pts = [(rng.uniform(-80, 80), rng.uniform(-180, 180)) for _ in range(rng.randint(3, 7))]
    add(pts, "spread")
with open(OUT, "w", newline="\n") as f:
    f.write("# tag;n;lat1;lon1;...;perimeter_m;area_m2  (GeographicLib 2.1 PolygonArea, WGS84)\n")
    f.write("\n".join(rows) + "\n")
print("rows", len(rows))
