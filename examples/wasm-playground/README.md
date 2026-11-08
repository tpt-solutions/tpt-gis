# wasm-playground

A browser playground for tpt-gis. Load GeoJSON, reproject it, and join points to polygons,
all client-side: the page runs the engine's Rust code compiled to WebAssembly, and no file
leaves the browser.

**Live:** published to GitHub Pages from `master` by
[`.github/workflows/pages.yml`](../../.github/workflows/pages.yml).

This crate is **not** part of the workspace `members` (it builds only for
`wasm32-unknown-unknown`), so it does not affect `cargo build --workspace`. It is the
browser counterpart of [`wasm-geofence-demo`](../wasm-geofence-demo).

## What it does

- **Load**: pick a points file and a polygons file, or load the bundled sample (London
  landmarks and four areas, one with a hole).
- **Reproject**: move a layer to another EPSG code. Supported codes are the ones
  `tpt_gis_core::epsg::lookup` knows: 4326, 3857, UTM zones (326xx/327xx), 2154, 5070, 4277
  and 27700.
- **Join**: keep each point that falls inside a polygon and merge the polygon's properties
  into it (the point's own properties win on a name clash). Both layers must be in the same
  CRS, so reproject one first if they differ. Download the result as GeoJSON.

Only `Point` features take part on the points side, and only `Polygon` and `MultiPolygon`
features on the polygons side. Other features are ignored.

## Exported functions

| Function | Purpose |
| --- | --- |
| `reproject(geojson, from_epsg, to_epsg)` | Transforms every vertex; keeps properties. Returns GeoJSON text. |
| `join(points_geojson, polygons_geojson)` | Points-in-polygons inner join. Returns GeoJSON text. |

Both return a `JsError` for unsupported EPSG codes or input that is not a GeoJSON
`FeatureCollection`.

## Run locally

Needs Rust with the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.126 (the
version pinned in `Cargo.toml`).

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/wasm_playground.wasm
python3 -m http.server 8080
```

Then open <http://localhost:8080/>. Serve over HTTP: the page loads a WebAssembly module,
which browsers refuse to load from `file://`.

Tests run on the host:

```sh
cargo test
```

The tests cover the join (including property merging), Web Mercator reprojection of the
origin, and a London point reprojected to the British National Grid.

## Layout

- `src/lib.rs`: the `wasm-bindgen` surface and the join/reproject logic.
- `index.html`: the dependency-free page. It imports `./pkg/wasm_playground.js`.
- `samples/`: the bundled GeoJSON. Coordinates are hand-written approximations for London,
  not taken from an external dataset.
