# wasm-geofence-demo

A browser demo of the same real-time drone geofence check used in
[`drone-geofence-demo`](../drone-geofence-demo), compiled to WebAssembly via
`wasm-bindgen`. It reuses that crate's `no_std`, zero-allocation `check_position`
logic unchanged — only a thin JS-friendly wrapper is added.

This crate is **not** part of the workspace `members` (it builds only for
`wasm32-unknown-unknown`), so it does not affect `cargo build --workspace`.

## Build & run

```sh
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir pkg \
    target/wasm32-unknown-unknown/debug/wasm_geofence_demo.wasm
python3 -m http.server 8080
```

Then open <http://localhost:8080/> and move the cursor over the map. The red
rectangle is a no-fly zone; the dot is your position. When inside, the demo shows
the escape heading and distance to the nearest edge — the same output the embedded
engine produces.

`index.html` is a fully static, dependency-free harness: it imports the
`wasm-bindgen` ES-module glue (`pkg/wasm_geofence_demo.js`) and renders the result
on a `<canvas>`.
