# Generates geographiclib_sample.csv from GeographicLib (WGS84) Line positions.
# Run: python generate_line_sample_reference.py geographiclib_sample.csv (needs geographiclib).
import sys, random
from geographiclib.geodesic import Geodesic
OUT = sys.argv[1]
g = Geodesic.WGS84
rng = random.Random(31)
pairs = [(0.0, 0.0, 10.0, 170.0), (51.5, -0.1, 48.85, 2.35), (-37.8, 144.9, -33.9, 151.2), (0.5, 0.0, -0.5, 179.7), (89.0, 0.0, 88.0, 180.0)]
for _ in range(45):
    pairs.append((rng.uniform(-80, 80), rng.uniform(-180, 180), rng.uniform(-80, 80), rng.uniform(-180, 180)))
steps = 8
rows = []
for lat1, lon1, lat2, lon2 in pairs:
    inv = g.Inverse(lat1, lon1, lat2, lon2)
    line = g.InverseLine(lat1, lon1, lat2, lon2)
    pts = []
    for k in range(steps + 1):
        s = inv["s12"] * k / steps
        pos = line.Position(s)
        pts.append((pos["lat2"], pos["lon2"]))
    rows.append(";".join([f"{lat1:.10f}", f"{lon1:.10f}", f"{lat2:.10f}", f"{lon2:.10f}", str(steps)] + [f"{a:.10f};{b:.10f}" for a, b in pts]))
with open(OUT, "w", newline="\n") as f:
    f.write("# lat1;lon1;lat2;lon2;steps;then steps+1 lat;lon pairs  (GeographicLib 2.1 InverseLine, WGS84)\n")
    f.write("\n".join(rows) + "\n")
print("rows", len(rows))
