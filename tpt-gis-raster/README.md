# tpt-gis-raster

Grid data, map algebra, resampling, and Cloud Optimized GeoTIFF (COG) parsing for
the [`tpt-gis`](https://crates.io/crates/tpt-gis) engine.

The crate is built from four layers, each usable on its own:

- **`grid`** — `Grid<T>`, a dense row-major array of cells of any `CellType`.
- **`band`** — `Band<T>` (alias `Raster<T>`), a `Grid` plus georeferencing and a
  nodata value. `AnyBand` is the dynamically-typed form a file reader returns.
- **`algebra` / `resample`** — nodata-aware map algebra and nearest-neighbour /
  bilinear / average resampling, generic over the cell type.
- **`geotiff` / `cog`** — a from-scratch (Geo)TIFF reader and a COG layout
  validator.

## Installation

```sh
cargo add tpt-gis-raster
# optional: async HTTP Range-request streaming reader for remote COGs
cargo add tpt-gis-raster --features http
```

## Example

```rust
use tpt_gis_raster::{algebra, Band, GeoTransform, Grid};

let grid = Grid::new(2, 2, vec![1i16, 2, -9999, 4]).unwrap();
let band = Band::new(grid, GeoTransform::new(0.0, 10.0, 1.0, 1.0)).with_nodata(-9999);

let doubled = algebra::add(&band, &band).unwrap();
assert_eq!(doubled.get(0, 0), Some(2));
// Nodata in, nodata out — not -19998.
assert_eq!(doubled.get(0, 1), Some(-9999));
```

## Features

- `std` (default) — enables the `(Geo)TIFF` / `COG` readers.
- `http` (off by default) — the async HTTP Range-request streaming reader for
  remote COGs (pulls in `reqwest` over rustls, no OpenSSL FFI).

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
