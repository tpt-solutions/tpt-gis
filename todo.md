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
- [x] `CODE_OF_CONDUCT.md` (placeholder contact email — update before going public)
- [x] `SECURITY.md` (placeholder contact email — update before going public)
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
- [x] Geodesic distance & bearing calculations (Vincenty; Karney not implemented —
      Vincenty is sufficient for the accuracy target and fails to converge only for
      a small set of near-antipodal points, see `geodesic.rs`)
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
- [ ] `buffer` operation — still deferred, needs a general polygon-offsetting
      algorithm (see crate-level doc comment)
- [ ] `union` operation — still deferred, needs a general polygon-clipping
      algorithm (Weiler-Atherton or Martinez-Rueda)
- [ ] `difference` operation — same blocker as `union`
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
- [ ] GeoPackage reader/writer
- [x] Streaming / zero-copy design for large files (`ShpGeometryIter` for shapefiles)

### tpt-gis-index (spatial indexing)
- [x] R-Tree: bulk loading
- [x] R-Tree: incremental insert
- [x] Quadtree implementation
- [x] Decide H3 strategy: pure-Rust reimplementation vs wrapping an existing pure-Rust crate (zero-FFI principle)
- [x] H3 hexagonal grid integration
- [ ] S2 grid integration
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

- [ ] Grid/raster data structures (typed arrays)
- [ ] Map algebra operations (add, subtract, reclassify, focal)
- [ ] GeoTIFF parser (IFD/tag parsing, tiling)
- [ ] DEFLATE / LZW decompression support
- [ ] Cloud Optimized GeoTIFF (COG) support (overviews, tiling validation)
- [ ] HTTP Range-request streaming reader (async, feature-gated)
- [ ] Resampling (nearest / bilinear)
- [ ] Nodata handling

### Phase 3 Milestone
- [ ] Parity with basic GDAL raster operations verified
- [ ] Streaming read of a large remote COG demonstrated without full download
- [ ] Streaming read performance benchmarked

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
      `tpt-gis-index`, `drone-geofence-demo`; `tpt-gis-raster` is still an empty stub)
- [x] Property-based tests (proptest) for geometry/geodesic invariants
- [x] Golden / reference-vector correctness tests (Vincenty test vector; UTM
      central-meridian provable invariant)
- [x] Benchmark harness (criterion) — added for the MVP 2 geofence check;
      not yet applied workspace-wide as a regression-tracking suite
- [x] Fuzzing harness (cargo-fuzz) for `tpt-gis-io` parsers (geojson, wkb, wkt, shapefile)
- [ ] Fuzzing harness (cargo-fuzz) for `tpt-gis-raster` parser (crate is still a stub)
- [ ] Rustdoc coverage across all crates (currently compliant for `tpt-gis-core`
      and `tpt-gis-geom` — `missing_docs` warns with zero violations — but the
      stub crates have no real API surface yet to document)
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
