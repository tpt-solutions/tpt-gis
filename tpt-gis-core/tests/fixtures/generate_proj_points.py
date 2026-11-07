# Generates proj_points.csv. Run: python generate_proj_points.py proj_points.csv (needs rasterio, which bundles PROJ).
import sys, random
from rasterio.warp import transform
OUT = sys.argv[1]
rng = random.Random(7)
rows = []  # (crs_tag, epsg, lat, lon)
for _ in range(200):
    zone = rng.randint(1, 60)
    north = rng.random() < 0.6
    cm = zone * 6 - 183
    lon = cm + rng.uniform(-2.9, 2.9)
    lat = rng.uniform(0.5, 83.5) if north else rng.uniform(-79.5, -0.5)
    epsg = (32600 if north else 32700) + zone
    rows.append(("utm", zone if north else -zone, epsg, lat, lon))
for _ in range(100):
    rows.append(("lambert93", 0, 2154, rng.uniform(42, 51), rng.uniform(-5, 9)))
for _ in range(100):
    rows.append(("albers_conus", 0, 5070, rng.uniform(25, 49), rng.uniform(-124, -67)))
for _ in range(100):
    rows.append(("web_mercator", 0, 3857, rng.uniform(-84, 84), rng.uniform(-179.9, 179.9)))

out = []
for tag, arg, epsg, lat, lon in rows:
    xs, ys = transform("EPSG:4326", f"EPSG:{epsg}", [lon], [lat])
    out.append(f"{tag};{arg};{lat:.10f};{lon:.10f};{xs[0]:.9f};{ys[0]:.9f}")
with open(OUT, "w", newline="\n") as f:
    f.write("# crs;arg;lat;lon;x;y  (arg: zone number, negative for south; 0 otherwise)\n")
    f.write("\n".join(out) + "\n")
print("rows", len(out))
