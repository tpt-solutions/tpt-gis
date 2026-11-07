# tpt-gis — Project Todo

Pure Rust, planetary-scale GIS engine. License: **Dual MIT / Apache-2.0**. Copyright: **TPT Solutions**.
Source design doc: `spec.txt`.

---

## Phase 0 — Project Setup & Governance

- [x] Init git repository, initial commit
- [x] Cargo workspace scaffolding: root `Cargo.toml` with 5 member crates as stub libs
  - [x] `tpt-gis-core`
  - [x] `tpt-gis-geom`
  - [x] `tpt-gis-raster`
  - [x] `tpt-gis-io`
  - [x] `tpt-gis-index`
- [x] `LICENSE-MIT` file (copyright TPT Solutions)
- [x] `LICENSE-APACHE` file (copyright TPT Solutions)
- [x] Set `license = "MIT OR Apache-2.0"` in every crate's `Cargo.toml`
- [x] `README.md` (project overview, from spec Executive Summary)
- [x] `CONTRIBUTING.md`
- [x] ~~`CODE_OF_CONDUCT.md`~~ / ~~`SECURITY.md`~~ — removed (project is issues-only; no
      placeholder contacts to maintain)
- [x] `.gitignore`
- [x] `rust-toolchain.toml` (channel = stable) + MSRV decision (1.81, in `Cargo.toml`)
- [x] `rustfmt.toml` + clippy lint config (`clippy::all`; `pedantic` deliberately **not**
      enabled — too aggressive for math-heavy geodesy code with conventional short
      variable names like `a`, `b`, `e2`, `f`)
- [x] Issue templates / PR template
- [x] Decide crates.io naming / namespace reservation strategy (documented in
      `PUBLISHING.md`: `tpt-gis-*` prefix with `tpt-gis-*-rs` fallback)

---

## Phase 1 (Months 1–3) — `tpt-gis-core` & `tpt-gis-geom`

### tpt-gis-core (replacement for PROJ)
- [x] `no_std` crate setup with optional `std` feature
- [x] CRS (Coordinate Reference System) type definitions
- [x] EPSG code registry / lookup (small curated set — WGS84, Web Mercator, all UTM
      zones computed formulaically; **not** the full EPSG database, see `epsg.rs`)
- [x] Ellipsoid models (WGS84, GRS80)
- [x] Geodesic distance & bearing calculations (Karney's algorithm, a port of
      GeographicLib; converges for every pair, including nearly antipodal ones; checked
      against GeographicLib on 613 problems. Vincenty was replaced.)
- [x] WGS84 ↔ Web Mercator transform
- [x] WGS84 ↔ UTM transform (state-plane not implemented)
- [x] Datum transformation pipeline (geodetic↔geocentric conversion + Helmert
      7-parameter/Bursa-Wolf transform)
- [x] Local tangent-plane (East-North-Up) approximation — added beyond the original
      plan; needed to fix a real nearest-edge-selection bug in the MVP 2 demo (see
      `projection/local_tangent_plane.rs`)

### tpt-gis-geom (replacement for GEOS)
- [x] `no_std` OGC Simple Features primitives — `Point`, `LineString`, `Polygon`
      done; `MultiPoint`/`MultiLineString`/`MultiPolygon` **not yet implemented**
- [x] Bounding box / envelope calculation
- [x] Point-in-polygon test (zero-allocation)
- [x] `intersects` predicate (segment/segment, polygon/polygon)
- [x] `contains` predicate (point-in-polygon, polygon-in-polygon)
- [x] `distance` operation (point-point, point-segment, point-linestring, plus
      nearest-point-on-boundary for polygons)
- [x] `buffer` operation — points, lines, and polygons with round joins; holes shrink
      (`buffer` module, `alloc` feature). Erosion of the exterior is not supported yet.
- [x] `union` operation — Martinez–Rueda–Feito sweep (`clip` module, `alloc` feature)
- [x] `difference` operation — same clipper (`clip::difference`)
- [x] Floating-point tolerance handling for robust predicates (`predicates::EPSILON`)

### MVP 2 — Real-Time Drone Geofence & Navigation Engine
- [x] `no_std` GPS coordinate stream ingestion (simulated flight path in the demo)
- [x] Pre-loaded no-fly-zone polygon loader (const/static data, zero heap alloc)
- [x] Real-time in/out-of-zone breach check
- [x] Escape-vector heading calculation on breach (fixed a real bug during
      implementation: comparing raw longitude/latitude degrees as if planar picks
      the wrong nearest edge, since a degree of longitude is shorter than a degree
      of latitude away from the equator — fixed via the local tangent-plane module
      above; see the regression test in `drone-geofence-demo`)
- [ ] Demo build for real hardware — target board is ESP32 / ESP32-C3 / ESP32-S3
      (available to the maintainer), not a Raspberry Pi as `spec.txt` originally
      suggested. What's verified so far (no hardware flashing yet — deliberately
      deferred, see below):
  - [x] `tpt-gis-core`/`tpt-gis-geom` cross-compile cleanly for
        `riscv32imc-unknown-none-elf` (the **ESP32-C3**'s target — mainline Rust
        support, no toolchain fork needed) and `thumbv7em-none-eabihf` (a generic
        Cortex-M stand-in) and `wasm32-unknown-unknown`
  - [x] `drone-geofence-demo`'s own `check_position` logic (not just the two
        library crates) also builds `no_std` for `riscv32imc-unknown-none-elf`
        (via `cargo build -p drone-geofence-demo --lib --no-default-features
        --target riscv32imc-unknown-none-elf`), so it's ready to drop into a real
        `esp-hal`-based firmware project without changes
  - [ ] **ESP32 / ESP32-S3 (Xtensa) targets are not yet buildable** — `rustup
        target add xtensa-esp32-none-elf` fails on stock stable Rust ("no
        prebuilt artifacts"); these need the community `esp-rs` toolchain fork,
        installed via `espup install` (a large one-time download, not done in
        this environment — left for whoever sets up the actual flashing
        workflow)
  - [ ] No actual firmware binary (HAL init, entry point, panic handler,
        flashing via `espflash`/`probe-rs`) has been built or flashed — this is
        real hardware validation the maintainer will do directly
- [ ] Latency benchmark on embedded target — **not done** on real hardware; a
      host-side criterion benchmark exists instead (`benches/geofence_check.rs`,
      ~350ns/check on a desktop x86 CPU) as a stand-in showing the check itself is
      cheap (no allocation, a handful of trig calls). True on-ESP32 timing (via a
      hardware timer once flashed) remains a follow-up.
- [x] Publish demo/example + write-up (`examples/drone-geofence-demo`, runnable via
      `cargo run -p drone-geofence-demo`; no external publishing/blog write-up done)

### Phase 1 Milestone
- [x] `no_std` build verified for `thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf`
      (ESP32-C3), and WASM (`wasm32-unknown-unknown`) for both `tpt-gis-core` and
      `tpt-gis-geom` — Xtensa (ESP32/ESP32-S3) still pending the `esp-rs` toolchain,
      see MVP 2 notes above
- [x] Geodesic accuracy validated against a published reference vector (Vincenty's
      Flinders Peak → Buninyong test case, cross-checked against
      movable-type.co.uk and Wikipedia)
- [ ] Drone Geofence Engine demo released — built and tested locally; not yet
      published externally (see MVP 2 notes above)

---

## Phase 2 (Months 4–6) — `tpt-gis-io` & `tpt-gis-index`

### tpt-gis-io (data interchange layer)
- [x] GeoJSON reader/writer (serde-based)
- [x] WKB (Well-Known Binary) reader/writer
- [x] WKT (Well-Known Text) reader/writer
- [x] Shapefile (.shp/.shx/.dbf) reader
- [x] GeoPackage reader/writer (pure-Rust, zero-FFI SQLite file format in `tpt-gis-io/src/geopackage/`; leaf+interior read, single-leaf-page write)
- [x] Streaming / zero-copy design for large files (`ShpGeometryIter` for shapefiles)

### tpt-gis-index (spatial indexing)
- [x] R-Tree: bulk loading
- [x] R-Tree: incremental insert
- [x] Quadtree implementation
- [x] Decide H3 strategy: pure-Rust reimplementation vs wrapping an existing pure-Rust crate (zero-FFI principle)
- [x] H3 hexagonal grid integration
- [x] S2 grid integration — implemented as a Z-order/Morton curve (`s2.rs`), not
      byte-compatible with Google's Hilbert-curve S2; see the deferred
      byte-compatibility item under "Security Audit & Correctness Fixes" below
- [x] Spatial-join algorithm built on R-Tree

### MVP 1 — Planetary-Scale Spatial Join Engine
- [x] CLI scaffold (clap)
- [x] Test fixture generation (10M points / 50k polygons) — `generate-fixtures` subcommand
- [x] Reprojection pipeline (via `tpt-gis-core`)
- [x] Spatial join execution (points-in-polygons via R-Tree)
- [x] Output writer (GeoJSON/CSV)
- [x] Throughput benchmark (criterion; baseline ~4ms for 10k points / 500 polygons)
- [ ] Publish CLI binary + benchmark report

### Phase 2 Milestone
- [x] Spatial Join Engine CLI released (local build + test; external publish pending)
- [ ] Throughput benchmark report published

---

## Phase 3 (Months 7–12) — `tpt-gis-raster` & COG Streaming

- [x] Grid/raster data structures (typed arrays) — `Grid<T>` + `CellType`/`CellKind`
      (`src/grid.rs`, `src/cell.rs`, `src/band.rs`)
- [x] Map algebra operations (add, subtract, reclassify, focal) — local, unary,
      reclassify, and neighbourhood stats (mean/min/max/sum/range/stddev/median) +
      `focal_kernel` convolution, all nodata-aware (`src/algebra.rs`)
- [x] GeoTIFF parser (IFD/tag parsing, tiling) — classic + BigTIFF, strips +
      tiles, single-band, GDAL geo-keys → `GeoTransform`/`epsg`/`nodata`
      (`src/geotiff.rs`)
- [x] GeoTIFF writer — `geotiff::write` emits a classic little-endian, uncompressed,
      single-band, single-strip file with the transform, EPSG (GeoKeys) and nodata
      (GDAL tag 42113) written; read back by the parser and checked with rasterio
      (`src/geotiff.rs`)
- [x] DEFLATE / LZW decompression support — `flate2` (rust_backend, no C) for
      DEFLATE, `weezl` for LZW, plus PackBits; horizontal (2) and floating-point (3)
      predictors (`src/geotiff.rs`)
- [x] Cloud Optimized GeoTIFF (COG) support (overviews, tiling validation) —
      `cog::validate` walks the IFD/SubIFD pyramid and reports tiling, internal
      ordering, data-range, and georeferencing issues (`src/cog.rs`)
- [x] HTTP Range-request streaming reader (async, feature-gated) — `http` module
      with a pluggable `RangeTransport` trait; `CogRangeReader::read_region` fetches
      only the tiles a window needs (`src/http.rs`). `MemoryTransport` lets it run
      against an in-memory buffer, so the tile-by-tile path is testable and
      benchmarkable without a server.
- [x] Resampling (nearest / bilinear) — plus average (overview-grade downsampling);
      strict about nodata for bilinear, skipping for nearest/average (`src/resample.rs`)
- [x] Nodata handling — sentinel propagated through every grid/algebra/resample
      operation; `NaN` sentinels match via `CellType::same_value` (`src/band.rs`)

### Phase 3 Milestone
- [x] Parity with basic GDAL raster operations verified — `tests/gdal_parity.rs`
      cross-checks `add`/`subtract`/`multiply`/`focal_{mean,min,max,sum,range,median,
      stddev}`/average-downsample against an independent naive reference (including
      nodata propagation and edge handling). A bug in the tiled `decode_window` row
      stride was found and fixed during this verification.
- [x] Streaming read of a large remote COG demonstrated without full download —
      `http::tests::streams_only_the_tiles_a_region_needs` reads a 2048×2048 tiled
      COG (≈4 MiB) region with a 1 MiB prefix and asserts only the prefix + the one
      needed tile are fetched (a `MemoryTransport` records the requested ranges). A
      COG fixture generator (`geotiff::testsupport::make_cog`) and a COG-validator
      test (`cog` module) were added to support this.
- [x] Streaming read performance benchmarked — `benches/cog_stream.rs` (criterion)
      compares a single-tile region read (~1.18 ms) against a full decode (~3.0 ms)
      on a 1024×1024 COG, showing the streaming path is ~2.5× cheaper. Run with
      `cargo bench -p tpt-gis-raster --features http --bench cog_stream`.

---

## Security Audit & Correctness Fixes (2026-08 review)

Found during a full project review (security audit + stub/todo sweep). See
`CLAUDE.md`'s crate table and the sections below for context — several of
these are things a previous pass marked `[x]` done that don't actually work.

### Security fixes (untrusted-input parsers)
- [x] GeoTIFF allocation/decompression bomb — `decode_window` allocates
      `width*height*bytes_per_sample` unbounded from IFD tags; `decompress`'s
      `expected` size cap is computed but never used (`tpt-gis-raster/src/geotiff.rs`)
- [x] HTTP range-response validation — `HttpTransport::fetch_range` accepts any
      2xx status and any response length with no cap; `len == 0` underflows a
      `u64` subtraction (`tpt-gis-raster/src/http.rs`)
- [x] Shapefile allocation bomb — `num_parts`/`num_points`/`num_records` used
      to pre-allocate before validation against remaining buffer length
      (`tpt-gis-io/src/shapefile.rs`)
- [x] Unbounded recursion on `Multi*`/`GeometryCollection` nesting in
      `tpt-gis-io/src/wkb.rs`, `wkt.rs`, and `geojson.rs` — add a shared
      max-nesting-depth guard to all three
- [x] Reject non-finite (`NaN`/`Infinity`) coordinates at the WKB/WKT/GeoJSON
      parser boundary, mirroring `tpt-gis-index/src/h3.rs`'s existing check —
      closes a panic path in `tpt-gis-index/src/rtree.rs`

### Correctness fixes (marked done, don't work)
- [x] `ShpGeometryIter` discards every parsed geometry and returns `None`
      immediately — streaming shapefile reader is non-functional
      (`tpt-gis-io/src/shapefile.rs`)
- [x] `tpt-gis-index/src/s2.rs` fails its own unit tests and doctest — three
      independent bit-math bugs (`ij()`/`from_face_ij()` transposition, missing
      inverse quadratic transform in `from_lat_lon`, wrong axis formulas in
      `face_uv_from_xyz` on faces 3/4/5); also soften the "same ids/tokens as
      Google's S2" doc claim (it's a Z-order curve, not Hilbert — not
      bit-compatible) rather than fabricate a reference test vector
  - [ ] (deferred, separate future pass) real Hilbert-curve/orientation-table
        S2 compatibility + variable-length token format, if byte-compatibility
        with Google's S2 is ever actually needed
- [x] `tpt-gis-io` fuzz harness doesn't compile — wrong dependency
      (`cargo-fuzz` instead of `libfuzzer-sys`) and two targets call
      nonexistent functions (`parse_wkb`/`parse_wkt` vs. real `parse_geometry`);
      not wired into CI either
- [x] No fuzz target exists for `tpt-gis-raster`'s GeoTIFF/COG parser — added one

### Documentation reconciliation
- [x] `CLAUDE.md` crate table is stale (calls `tpt-gis-io`/`-index`/`-raster`
      stubs) and misstates the lint config (claims `clippy::pedantic` is
      enforced; only `clippy::all` is)
- [x] `todo.md` (this file) self-contradicts: Cross-Cutting section below still
      says "`tpt-gis-raster` is still an empty stub" while Phase 3 above
      correctly describes it as substantially complete — fix once the raster
      fixes above land
- [x] `CHANGELOG.md`'s `[0.1.0]` link points at a release tag that was never
      pushed (404)
- [x] `.gitignore` doesn't exclude `*.log` — stray `build.log`/`test_*.log`
      files sit untracked at repo root

### Adoption tooling
- [x] README doesn't link `examples/spatial-join-cli` or
      `examples/drone-geofence-demo` — no path from README to either
- [x] No crate sets `keywords`/`categories` in `Cargo.toml` (hurts crates.io
      discoverability); not on `PUBLISHING.md`'s checklist either
- [x] `examples/spatial-join-cli` has no crate-local `README.md`
- [x] `examples/spatial-join-cli/src/main.rs` uses `.expect()` throughout for
      file I/O/parsing — raw panics instead of clean CLI error exit
- [x] `tpt-gis-io/src/shapefile.rs`'s `trimmed_ascii` silently turns non-UTF-8
      `.dbf` Character fields (common in the wild) into an empty string
      instead of lossily decoding them
- [x] CI's `no_std` job builds a `wasm32-wasi` target that no longer exists on
      current stable Rust (renamed to `wasm32-wasip1`/`wasm32-wasip2`) — fix in
      `rust-toolchain.toml` and `.github/workflows/ci.yml`

### New features
- [x] `tpt-gis` facade crate — single-dependency, feature-gated re-export of
      all five `tpt-gis-*` crates, `http` opt-in (pulls in reqwest/tokio),
      published last per `PUBLISHING.md`
- [x] Browser WASM demo (`examples/wasm-geofence-demo`) — reuses
      `examples/drone-geofence-demo`'s existing `check_position` logic via a
      `wasm-bindgen` layer + a dependency-free static `index.html`/canvas
      harness; linked from the root README

---

## Cross-Cutting / Infrastructure

- [x] CI: std targets (Linux/macOS/Windows) — workflow configured
      (`.github/workflows/ci.yml`); not yet exercised by an actual GitHub Actions
      run (no remote pushed yet). Local-equivalent commands verified on Windows.
- [x] CI: `no_std` embedded targets — configured + locally verified
      (`thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf` for ESP32-C3)
- [x] CI: `wasm32-unknown-unknown` target — configured + locally verified
- [x] CI: `wasm32-wasi` target — added
- [ ] CI: Xtensa targets (`xtensa-esp32-none-elf`, `xtensa-esp32s3-none-elf`) for
      ESP32/ESP32-S3 — blocked on the `esp-rs` toolchain fork (`espup`), which
      GitHub Actions runners don't have preinstalled; needs a dedicated setup step
      if added
- [x] Unit tests per crate (`tpt-gis-core`, `tpt-gis-geom`, `tpt-gis-io`,
      `tpt-gis-index`, `drone-geofence-demo`;        `tpt-gis-raster` is still an empty stub — this is now obsolete: Phase 3
       above correctly describes it as substantially complete)
- [x] Property-based tests (proptest) for geometry/geodesic invariants
- [x] Golden / reference-vector correctness tests (Vincenty test vector; UTM
      central-meridian provable invariant)
- [x] Benchmark harness (criterion) — added for the MVP 2 geofence check;
      not yet applied workspace-wide as a regression-tracking suite
- [x] Fuzzing harness (cargo-fuzz) for `tpt-gis-io` parsers (geojson, wkb, wkt, shapefile)
- [x] Fuzzing harness (cargo-fuzz) for `tpt-gis-raster` parser (GeoTIFF/COG) — added
      under `tpt-gis-raster/fuzz`, wired into CI
- [x] Rustdoc coverage across all crates — `missing_docs` warns with zero
      violations on `tpt-gis-core`, `tpt-gis-geom`, `tpt-gis-io`, `tpt-gis-index`,
      `tpt-gis-raster`, and the `tpt-gis` facade (verified via `cargo doc`)
- [x] docs.rs configuration
- [x] User-guide site (`docs/` directory with getting-started and CLI usage guides)
- [x] Semver policy documented
- [x] `CHANGELOG.md` + release automation (changelog added)
- [x] crates.io publishing checklist per crate
- [x] Correctness oracle: comparison tests vs reference values (WebMercator, UTM, Vincenty)
- [x] `cargo-deny` in CI enforcing zero GPL/LGPL dependencies (`deny.toml`,
      verified locally against the full dependency tree including dev-dependencies —
      exit code 0); no separate `cargo-audit` added, but `cargo-deny`'s
      `[advisories]` check covers the same RUSTSEC advisory database

---

## Security Hardening & Adoption Tooling (2026-08-20 review)

Found during a follow-up full project review (security audit + stub/todo sweep +
adoption-tooling assessment). No literal stubs/`TODO`s remain in `src/` anywhere in
the workspace — the items below are new hardening/ergonomics work, not unfinished
features from an earlier pass.

### COG HTTP / raster hardening (security)
- [x] `HttpTransport::fetch_range` (`tpt-gis-raster/src/http.rs`) accepts any 2xx
      status, not just `206 Partial Content` — a Range-unaware origin returning
      `200 OK` with the full body makes the reader silently buffer the entire file
      before truncating, defeating the module's own "read without downloading the
      whole file" promise. Extract a pure `validate_range_response` helper that
      hard-errors on `200`/anything but `206`, plus a defense-in-depth check that
      a `206`'s `Content-Length` doesn't exceed the requested range.
- [x] `CogRangeReader::new` builds `reqwest::Client::new()` with no timeout — a
      hung/slow-loris origin blocks forever. Add `.timeout(30s)`/`.connect_timeout(10s)`
      via `Client::builder()`.
- [x] `decode_window` (`tpt-gis-raster/src/geotiff.rs`) calls `self.fetch(offset,
      byte_count)` with `byte_count` taken directly from the untrusted
      `StripByteCounts`/`TileByteCounts` IFD tags, before any size check (only the
      *decompressed* size is capped, via `MAX_DECODE_BYTES`). A crafted COG can
      declare a huge per-tile byte count, reaching a fetch (and, over HTTP, a real
      Range request) before rejection. Add a `MAX_RAW_CHUNK_BYTES` cap validated in
      `image_info_from` right after `chunks` is built (closes both the local
      `decode_window` path and the HTTP `read_region` prefetch-loop path, which
      calls `transport.fetch_range` directly with the same untrusted count).
- [x] No fuzz target covers the hand-rolled GeoPackage/SQLite b-tree reader
      (`tpt-gis-io/src/geopackage/`) or the Shapefile DBF reader (`read_dbf`) — add
      `tpt-gis-io/fuzz/fuzz_targets/{geopackage,dbf}.rs`.
- [ ] (deferred, lower severity, noted but not blocking) `decode_lzw` decodes the
      full LZW stream via `weezl` before checking the result against
      `MAX_DECODE_BYTES`, so a pathological stream can force a large intermediate
      allocation before the existing post-decode check fires; and
      `last_data_offset`'s `offset + count` sum is unchecked (partially mitigated
      by the `MAX_RAW_CHUNK_BYTES` cap above, but `offset` itself is still
      attacker-controlled). The `offset + count` overflow was closed in
      `image_info_from` via `checked_add` (saturating to `u64::MAX`), so it can no
      longer panic; the `decode_lzw` full-stream-before-check ordering remains.

### Adoption ergonomics
- [x] Root `README.md` quickstart code block + CI/license/MSRV badges
- [x] `prelude` module on the `tpt-gis` facade crate for one-line ergonomic imports
- [x] Promote `examples/spatial-join-cli` to a real installable `tpt-gis-cli` crate
      (`cargo install tpt-gis-cli`, binary name `tptgis`), leaving its criterion
      benchmark behind so the publishable crate stays lean; update `PUBLISHING.md`'s
      publish order and root `Cargo.toml` members accordingly
- [x] Python bindings (`bindings/tpt-gis-py`, pyo3 + maturin) — v0.1 scope:
      `Geometry` (WKT/WKB/GeoJSON round-trip + `contains_point`), shapefile read,
      `GeoPoint`/geodesic distance from `tpt-gis-core`. Deferred: raster/COG, the
       index crate, GeoPackage, projections, numpy interop. A matrixed 3-OS CI job
       (`maturin develop` + `pytest`) and a workspace `exclude` entry (builds via
       maturin, not plain `cargo build`, same as the wasm demo) are both in place, and
       the extension builds + all 10 pytest cases pass locally. Discoverability is
       wired up: the root README links the bindings (PyPI badge + quickstart), a
       `docs/python-bindings.md` guide is linked from `docs/getting-started.md`, and a
       `publish-python` CI job builds abi3 wheels (Python 3.8+, Linux/macOS/Windows)
       and publishes them to PyPI on `v*` tags via the `PYPI_API_TOKEN` secret. The
       crate is built as an abi3 extension.

### Housekeeping (flagged, not actioned)
- [x] Six untracked `CHANGELOG.md` files (one per crate) plus
      `tpt-gis-core/examples/geodesic_demo.rs` and
      `tpt-gis-geom/examples/geometry_demo.rs` — all now committed (checked with
      `git ls-files`), so `cargo publish` will include them

---

## Phase 4 — Platform review follow-ups (2026-11)

Source: static review of all crates + CI/docs. Findings are **unverified until a
failing test exists** — add the regression test first, then fix. Root cause for many
raster/io bugs: `tpt-gis-raster/tests/gdal_parity.rs` compares against a naive copy of
the same logic, not real GDAL/QGIS/libtiff output; add third-party fixtures.

### 4.1 Correctness bugs — wrong results
- [x] `tpt-gis-index` S2: `face_uv_to_xyz` face 5 returned `(-u,-v,1)` instead of
      `(u,v,-1)`; regression tests added
- [x] S2: reject invalid ids (face >= 6, `level()` underflow on id 0,
      `from_lat_lon` level > 30, NaN/out-of-range lat/lon) instead of panicking
- [x] S2: docs claim Google-S2 compatibility but ordering is Morton, tokens are fixed
      16 hex digits — implement Hilbert + trimmed tokens, or fix `lib.rs` docs
- [x] `polygon_contains_polygon`: ignores outer-polygon holes; rejects polygons that
      touch the outer boundary (OGC `contains` allows boundary contact)
- [x] `polygons_intersect`: hole edges never tested
- [x] `orientation`: the cross-product test was already relative to the edge lengths.
      The segment bounding-box and point-on-boundary tests now use `tolerance_for`, a
      floor of `EPSILON` that grows to 1e-14 × the coordinate magnitude (about 45 ulps),
      so near-touches at 1e9 scale are found. `point_in_ring`'s boundary rule is defined
      and tested (left and bottom edges inside, right and top outside). Shewchuk-style
      adaptive predicates are not implemented.
- [x] GeoPackage blob header: flags and envelope sizes checked against a GDAL-written
      `.gpkg` (pyogrio, GDAL 3.12), with envelopes and an empty point; the fixture is in
      `tpt-gis-io/tests/fixtures/`. The check found a real bug instead: an
      `INTEGER PRIMARY KEY` column must have a NULL slot in the record header, and our
      reader skipped it and our writer omitted it, so other readers saw shifted columns.
      Both are fixed, with a layout test.
- [x] GeoPackage writer: set `application_id`/`user_version`, add srs rows 0 and -1,
      sort/dedupe rowids, lift single-4KiB-page limit (>~250 vertices → `WriteTooLarge`)
- [x] GeoPackage reader: tolerate rows with fewer columns than schema
      (`ALTER TABLE ADD COLUMN`); don't swallow geometry decode errors
- [x] TIFF predictor 2 (per-sample, honour byte order/bit depth) and predictor 3
      (libtiff byte-plane algorithm)
- [x] TIFF short last strip (height not multiple of `rows_per_strip`); windowed read
      origin-Y when no geotransform
- [x] HTTP COG reader: files < 1 MiB fail (`cache.get(0, PREFIX)`); IFD beyond 1 MiB
      fails; cap chunked bodies; redirect/scheme limits; custom headers
- [x] CLI `spatial-join`: honour polygon holes; handle/warn on Multi* geometries
- [x] CLI `reproject`: Multi* / GeometryCollection pass through unprojected
- [x] CLI: missing `format!` in unsupported-CRS error; `--format` should be a
      `ValueEnum`; broken-pipe panic on stdout
- [x] Local tangent plane: wrap antimeridian, guard poles, use ellipsoidal north scale
- [x] UTM: Norway (32V) / Svalbard exceptions, longitude wrapping, `Zone::try_new` and
      `try_containing` reject bad input; Web Mercator wraps longitude and clamps latitude.
      `Zone::new` stays unchecked by design (use `try_new` for untrusted input).
- [x] `geocentric_to_geodetic` wrong at the poles
- [x] Quadtree depth reset on `subdivide`

### 4.2 Robustness — panics, hangs, DoS on untrusted input
- [x] `geodesic::direct`: unbounded loop on NaN/inf — iteration cap + finite checks;
      normalise output longitude
- [x] SQLite reader: cycle/depth guard in `read_btree`; page number 0 underflow;
      untrusted `payload_len` → `with_capacity`; unchecked `pos + len`
- [x] `parse_create_table`: non-ASCII quoted name slice panic; quoted names with spaces
- [x] TIFF: IFD chain cycle (explicit cycle error, plus a length cap); zero width or
      height rejected; out-of-range tag values rejected by `u16_tag`; `decode_lzw` now
      decodes in bounded chunks and stops once the output exceeds the declared size.
      Tests cover each.
- [x] Shapefile: 0-part polygon yields `rings: []` (later panics); swallowed header error
- [x] R-tree `str_pack` panics on NaN bboxes — return `Result` or filter
- [ ] Extend fuzz targets after each parser fix; run (not just compile) fuzzers on a schedule

### 4.3 Hygiene
- [x] Remove or implement unused `serde` / `alloc` features in core and geom
- [x] Fix `tpt-gis-core/src/lib.rs` `std` doc comment (libm is used unconditionally)
- [x] Verify MSRV 1.81 for `no_std` float methods (`abs` etc.). Found and fixed: float
      arithmetic in `const fn` (`Ellipsoid::from_inverse_flattening`, `GeoTransform::to_gdal`
      are now plain `fn`; the built-in ellipsoids are `const` struct literals). Dependencies
      needing a newer Rust were pinned in `Cargo.lock`, `h3o` moved to 0.6 (0.8+ needs
      Rust 1.82+), and `proptest`/`tempfile` moved to older releases. `cargo +1.81 test
      --workspace` passes. CI has an `msrv` job.
- [x] Commit `Cargo.lock` (repo ships a binary + Python extension) — tracked. The lock is
      pinned so the workspace builds on Rust 1.81 (see the MSRV item below).
- [ ] Add real GDAL/QGIS/libtiff-produced fixtures to raster/io tests
      - Done: GDAL-written GeoTIFFs (tiled uint16 with deflate and predictor 2; striped
        float32, three bands, deflate and predictor 3; single-band float32 predictor 3) in
        `tpt-gis-raster/tests/fixtures/`, checked cell by cell against GDAL's arrays. A
        GDAL-written GeoPackage in `tpt-gis-io/tests/fixtures/`. These found the zlib,
        predictor-3 and rowid-alias bugs.
      - Open: QGIS- and libtiff-written files.

### 4.4 Missing features (by adoption value)
- [x] `Multi*` + `GeometryCollection` in `tpt-gis-geom`
- [ ] Geometry ops: area, centroid, convex hull, simplify, `is_valid`, segment
      intersection point, touches/crosses/within/overlaps
      - Done: area, centroid, convex hull, simplify, `is_valid` (basic), segment
        intersection point, within, polygon touches/overlaps (`relate`, not DE-9IM).
      - Open: `crosses`, line-polygon relations, full DE-9IM.
      - Fixed: `clip` split nearly vertical edges in lexicographic order; a cut could fall
        before its edge's own endpoint, so union, intersection and difference were wrong
        for some inputs (union returned nothing). Found by the GEOS differential test.
- [ ] `buffer` / `union` / `difference` / `intersection`
      - Decision (recommended option, followed): Martinez–Rueda–Feito sweep.
      - Done: `union`, `intersection`, `difference`, `symmetric_difference`
        (`clip`, `alloc` feature), tested against a point-membership oracle.
      - Done since: buffers of polygons with holes (holes shrink).
      - Done since: erosion (negative distances) is polygon minus the boundary buffer.
      - Open: a balanced-tree merge for buffers
        of very large inputs.
- [ ] CRS: Lambert Conformal Conic, Albers, generic Transverse Mercator, NAD83 /
      ETRS89 / BNG / Lambert-93 registry entries, general `transform(from, to)`
      - Done: LCC (with Lambert-93), Albers, generic Transverse Mercator (UTM is a
        special case), Web Mercator longitude wrapping.
      - Open: EPSG registry entries, BNG (needs the OSTN15 grid), general transform.
- [x] Karney geodesics (fixes Vincenty antipodal failure) — inverse and direct, checked
      against GeographicLib; not done: geodesic area (`S12`) and line sampling
- [ ] I/O: ISO WKB Z/M + EWKB; WKT `EMPTY`/`Z`/`SRID=`; bare GeoJSON Feature/Geometry,
      `properties: null`; shapefile `.prj`/`.cpg`/Z types + writer; GeoPackage
      WITHOUT ROWID + R-tree tables
      - Done: ISO and EWKB Z/M/SRID in WKB (extra ordinates are read and dropped), WKT
        `Z`/`M`/`ZM` tags and `SRID=` prefixes, bare GeoJSON Feature and geometry objects,
        and `properties: null`. Z and M values are not kept: the model is 2D.
      - Also done: shapefile Z and M types read as 2D; `info` reports a shapefile's `.prj`.
      - Also done: `.cpg` encodings; `.dbf` attributes flow through `convert`, `info`, and
        `validate`.
      - Also done: a shapefile writer (`.shp`, `.shx`, `.dbf`, `.cpg`), checked by GDAL
        (read through pyogrio).
      - Also done: `.prj` for shapefiles written from GeoJSON (WGS84) or shapefiles.
      - Open: WKT `EMPTY` (the model has no empty geometries), GeoPackage
        WITHOUT ROWID and R-tree tables.
- [ ] Raster: overview IFDs, COG writer (multi-band read and write, and the single-band GeoTIFF writer, are done)
      - Done: PixelIsPoint georeferencing (the origin moves half a pixel); ZSTD decoding
        (`ruzstd`, decode only). Tests cover raw-block frames; real compressed streams are
        exercised by the decoder's own tests, not by ours.
- [ ] Index: R-tree remove + k-NN; polygon-polygon and nearest spatial joins;
      H3/S2 polygon covering
      - Done: `RTree::remove`, `RTree::nearest` / `nearest_iter`,
        `polygon_intersection_join`, `nearest_polygon_join`.
      - Open: H3/S2 polygon covering.

### 4.5 Innovation
- [x] **Geofence compiler**: `tptgis geofence-compile in.geojson > fence.rs` emitting a
      `const` `no_std` zero-alloc polygon + bbox table for drone/IoT firmware
- [ ] Browser/WASM playground (GitHub Pages): load GeoJSON, reproject, join client-side
- [x] `tptgis validate` / `doctor` for COG layout, GeoPackage, GeoJSON
- [ ] `tptgis cog-tiles`: XYZ tile server over HTTP-range COGs
- [x] Differential tests against stored GDAL/PROJ/GEOS-generated fixtures
      - GEOS (shapely): `tpt-gis-geom/tests/geos_differential.rs` checks 400 random polygon
        pairs (touching, nested, holed) for area, centroid, hull, intersects, covers,
        touches, overlaps, and intersection, union and difference areas. The clip checks
        found a real bug (see the clip item below), now fixed.
      - PROJ (rasterio): `tpt-gis-core/tests/proj_differential.rs` checks 500 points for UTM
        (north and south), Lambert-93, Conus Albers and Web Mercator. Max errors: UTM about
        1 mm, Lambert-93 and Albers about 7 µm, Web Mercator about 34 µm.
      - GDAL: the GeoTIFF and GeoPackage fixtures above.
      - Generators are in the `tests/fixtures/` folders next to the data.

### 4.6 CLI & Python usability
- [ ] CLI subcommands: `info`, `convert` (GeoJSON/WKT/WKB/SHP/GPKG), raster/COG info;
      stdin `-` support, `--verbose`, shell completions, `EPSG:xxxx` CRS args,
      CSV header row + unquoted string values, file path in parse errors
- [x] CLI: `run_with_args` should not call `process::exit` (testable); add
      `tpt-gis-cli/tests`
- [ ] Python: `.pyi` stubs + `py.typed` (done: the native module is
      `tpt_gis_py._core`, stubs are checked by `mypy.stubtest` in CI), `__geo_interface__`,
      bounds/area/length, `__eq__`/`__hash__`/pickle, and the exception hierarchy (done).
      Open: `allow_threads`, numpy interop, zipped shapefile geometry+attributes, wheel
      matrix (aarch64, musllinux, universal2), classifiers

### 4.7 Docs & adoption
- [ ] `docs/coming-from-gdal-proj-geos.md`: parity table + side-by-side with
      shapely / geopandas / rasterio / `geo`
- [x] Cookbook: reproject shapefile→GeoJSON, COG window read, spatial join, embedded
      geofence, geodesic distance
- [x] Fix `docs/getting-started.md` (use facade crate; cover raster, `http`, CLI, wasm)
- [ ] Feature-flag table in crate READMEs; `[package.metadata.docs.rs]` on all crates
- [ ] Supported CRS / format / operation matrix + FAQ; mdBook site on GitHub Pages
- [ ] Re-run benchmarks in **release mode**; publish 10M-point comparison vs
      GDAL/PostGIS/geopandas (current number is a debug build)
- [ ] `cargo generate` templates (no_std geofence, wasm module, CLI tool); Dockerfile;
      `.devcontainer`; `wasm-pack` npm package
- [x] Remove now-unused `.github/PULL_REQUEST_TEMPLATE.md` (PRs not accepted) and add
      `ISSUE_TEMPLATE/config.yml` with a Q&A/discussion link

### 4.8 CI & release automation
- [ ] `.github/dependabot.yml`; `concurrency` group; `Swatinem/rust-cache`; SHA-pin actions
      - Done: dependabot, `concurrency` group, SHA-pinned actions (dependabot keeps them current).
      - Open: `Swatinem/rust-cache`, MSRV job, `cargo doc -D warnings`, feature powerset.
- [ ] Jobs: MSRV 1.81, `cargo-hack` feature powerset, `cargo doc -D warnings`,
      doctests, `http` feature, `cargo-semver-checks`, coverage (llvm-cov → Codecov),
      scheduled `cargo audit` / `cargo deny` advisories, scheduled fuzz runs
      - Done in `ci.yml`: `msrv` (Rust 1.81 test), `docs` (`RUSTDOCFLAGS=-D warnings`),
        `feature-powerset` (`cargo hack check --feature-powerset`; passes locally across
        81 combinations), and `Swatinem/rust-cache` on the build jobs. Doc warnings were
        fixed so the docs job passes.
      - Open: doctests as a separate job, the `http` feature in CI, semver checks,
        coverage, scheduled audit and fuzz runs.
- [ ] `release.yml`: ordered `cargo publish`, GitHub Release with changelog extraction,
      prebuilt `tptgis` binaries, `cargo-binstall` metadata, Homebrew formula
- [ ] Publish crates + wheels and tag a first release (nothing is on crates.io/PyPI yet;
      README PyPI badge is dead until then)
