# tpt-gis-raster

Grid data, map algebra, resampling, and GeoTIFF / Cloud Optimized GeoTIFF (COG) reading
and writing for the [`tpt-gis`](https://github.com/tpt-solutions/tpt-gis) engine.

> **Status:** not yet published to crates.io. Depend on it by path or git, for example
> `tpt-gis-raster = { git = "https://github.com/tpt-solutions/tpt-gis" }`.

## Why

The raster layer is built from four layers, each usable on its own:

- **`grid`** — `Grid<T>`, a dense row-major array of cells of any `CellType`.
- **`band`** — `Band<T>` (alias `Raster<T>`), a `Grid` plus georeferencing and a nodata value.
  `AnyBand` is the dynamically-typed form a file reader returns.
- **`algebra` / `resample`** — nodata-aware map algebra and nearest-neighbour, bilinear and
  average resampling, generic over the cell type.
- **`geotiff` / `cog`** — a from-scratch (Geo)TIFF reader and writer, and a COG layout validator.

The thread running through all of it is **nodata**: every operation that can see a nodata
cell propagates it rather than silently treating the sentinel (typically `-9999` or `NaN`)
as real data.

## Installation

```toml
[dependencies]
tpt-gis-raster = { git = "https://github.com/tpt-solutions/tpt-gis" }
# optional: async HTTP Range-request streaming reader for remote COGs
# tpt-gis-raster = { git = "https://github.com/tpt-solutions/tpt-gis", features = ["http"] }
```

## Features

| Feature | Default | Description                                                       |
| ------- | ------- | ----------------------------------------------------------------- |
| `std`   | on      | Enables the GeoTIFF and COG readers and writers (need `std::io`). |
| `http`  | off     | Async HTTP Range-request COG reader (`reqwest` over rustls).      |

The `grid`, `algebra`, and `resample` layers are `no_std` + `alloc` and stay available
without `std`, so a grid can be manipulated on an embedded target; only the file readers
and writers need `std`.

## Reading and writing

- `geotiff::GeoTiff::parse` and `read` — read a single-band image; `read_band` and
  `GeoTiff::read_image_band` select a band of a multi-band file. ZSTD, DEFLATE, LZW and
  uncompressed strips and tiles are supported. `PixelIsPoint` georeferencing is honoured.
- `geotiff::write` and `write_bands` — uncompressed single-strip GeoTIFF, single or multi-band.
- `geotiff::write_cog` with `CogOptions` — a tiled Cloud Optimized GeoTIFF with optional DEFLATE
  and chained overviews.
- `cog::validate` — checks a file against the COG layout rules and reports issues.

## Examples

### Build a band and run map algebra

```rust
use tpt_gis_raster::{algebra, Band, GeoTransform, Grid};

// A 3x3 elevation band with a nodata hole in the centre.
let grid = Grid::new(3, 3, vec![1i16, 2, 3, 4, -9999, 6, 7, 8, 9]).unwrap();
let dem = Band::new(grid, GeoTransform::new(0.0, 0.0, 1.0, 1.0)).with_nodata(-9999);

// Local op: nodata in, nodata out — not -19998.
let doubled = algebra::add(&dem, &dem).unwrap();
assert_eq!(doubled.get(1, 1), Some(-9999));

// A focal mean fills the hole from its (valid) neighbours.
let smoothed = algebra::focal_mean(&dem, 1).unwrap();
assert_eq!(smoothed.get(1, 1), Some(5)); // mean of 1,2,3,4,6,7,8,9
```

### Resample / build overviews

```rust
use tpt_gis_raster::{resample, Band, GeoTransform, Grid, Resampling};

let cells: Vec<f32> = (0..16).map(|i| i as f32).collect();
let band = Band::new(Grid::new(4, 4, cells).unwrap(), GeoTransform::new(0.0, 4.0, 1.0, 1.0));

// Halve the resolution: each output cell averages a 2x2 source block.
let overview = resample::downsample(&band, 2, Resampling::Average).unwrap();
assert_eq!(overview.width(), 2);
assert_eq!(overview.get(0, 0), Some(2.5)); // mean of 0, 1, 4, 5
```

### Read a GeoTIFF / validate a COG

```rust,no_run
use tpt_gis_raster::cog::validate;
use tpt_gis_raster::geotiff::GeoTiff;

let bytes = std::fs::read("example.tif").unwrap();
let tiff = GeoTiff::parse(&bytes).unwrap();
let band = tiff.read().unwrap(); // AnyBand
println!("decoded {}x{} band", band.width(), band.height());

let report = validate(&bytes).unwrap();
if report.is_valid() {
    println!("valid COG with {} overview level(s)", report.overviews.len());
}
```

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
