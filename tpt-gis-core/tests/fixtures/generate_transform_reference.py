# Generates proj_transform.csv. Run: python generate_transform_reference.py proj_transform.csv (needs rasterio).
import sys, random
from rasterio.warp import transform
OUT = sys.argv[1]
rng = random.Random(33)
codes = [4326, 3857, 32633, 32618, 32755, 2154, 5070]
# Geographic extent each CRS is meant for, as (lat range, lon range).
extent = {
    4326: ((-80, 80), (-170, 170)), 3857: ((-80, 80), (-170, 170)),
    32633: ((5, 70), (12, 18)), 32618: ((5, 70), (-78, -72)), 32755: ((-80, -5), (144, 150)),
    2154: ((42, 51), (-5, 9)), 5070: ((25, 49), (-124, -67)),
}
rows = []
for _ in range(400):
    src, dst = rng.sample(codes, 2)
    # Pick a point inside both extents (use the intersection of the two boxes).
    (la0, la1), (lo0, lo1) = extent[src]
    (lb0, lb1), (mo0, mo1) = extent[dst]
    lat0, lat1 = max(la0, lb0), min(la1, lb1)
    lon0, lon1 = max(lo0, mo0), min(lo1, mo1)
    if lat0 >= lat1 or lon0 >= lon1:
        continue
    lat = rng.uniform(lat0, lat1); lon = rng.uniform(lon0, lon1)
    sx, sy = transform("EPSG:4326", f"EPSG:{src}", [lon], [lat])
    dx, dy = transform(f"EPSG:{src}", f"EPSG:{dst}", sx, sy)
    rows.append(f"{src};{dst};{sx[0]:.9f};{sy[0]:.9f};{dx[0]:.9f};{dy[0]:.9f}")
with open(OUT, "w", newline="\n") as f:
    f.write("# src_epsg;dst_epsg;src_x;src_y;dst_x;dst_y  (PROJ via rasterio; src from EPSG:4326)\n")
    f.write("\n".join(rows) + "\n")
print("rows", len(rows))
