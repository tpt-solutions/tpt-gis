# tpt-gis — Project Todo

Pure Rust, planetary-scale GIS engine. License: **Dual MIT / Apache-2.0**. Copyright: **TPT Solutions**.
Source design doc: `spec.txt`.

---

## Phase 0 — Project Setup & Governance

- [ ] Init git repository, initial commit
- [ ] Cargo workspace scaffolding: root `Cargo.toml` with 5 member crates as stub libs
  - [ ] `tpt-gis-core`
  - [ ] `tpt-gis-geom`
  - [ ] `tpt-gis-raster`
  - [ ] `tpt-gis-io`
  - [ ] `tpt-gis-index`
- [ ] `LICENSE-MIT` file (copyright TPT Solutions)
- [ ] `LICENSE-APACHE` file (copyright TPT Solutions)
- [ ] Set `license = "MIT OR Apache-2.0"` in every crate's `Cargo.toml`
- [ ] `README.md` (project overview, from spec Executive Summary)
- [ ] `CONTRIBUTING.md`
- [ ] `CODE_OF_CONDUCT.md`
- [ ] `SECURITY.md`
- [ ] `.gitignore`
- [ ] `rust-toolchain.toml` + MSRV decision
- [ ] `rustfmt.toml` + clippy lint config
- [ ] Issue templates / PR template
- [ ] Decide crates.io naming / namespace reservation strategy

---

## Phase 1 (Months 1–3) — `tpt-gis-core` & `tpt-gis-geom`

### tpt-gis-core (replacement for PROJ)
- [ ] `no_std` crate setup with optional `std` feature
- [ ] CRS (Coordinate Reference System) type definitions
- [ ] EPSG code registry / lookup
- [ ] Ellipsoid models (WGS84, GRS80, etc.)
- [ ] Geodesic distance & bearing calculations (Vincenty/Karney)
- [ ] WGS84 ↔ Web Mercator transform
- [ ] WGS84 ↔ UTM / state-plane transforms
- [ ] Datum transformation pipeline

### tpt-gis-geom (replacement for GEOS)
- [ ] `no_std` OGC Simple Features primitives (Point, LineString, Polygon, Multi*)
- [ ] Bounding box / envelope calculation
- [ ] Point-in-polygon test (zero-allocation)
- [ ] `intersects` predicate
- [ ] `contains` predicate
- [ ] `distance` operation
- [ ] `buffer` operation
- [ ] `union` operation
- [ ] `difference` operation
- [ ] Floating-point tolerance handling for robust predicates

### MVP 2 — Real-Time Drone Geofence & Navigation Engine
- [ ] `no_std` GPS coordinate stream ingestion
- [ ] Pre-loaded no-fly-zone polygon loader (const/static data, zero heap alloc)
- [ ] Real-time in/out-of-zone breach check
- [ ] Escape-vector heading calculation on breach
- [ ] Demo build for Raspberry Pi / flight-controller target
- [ ] Latency benchmark on embedded target
- [ ] Publish demo/example + write-up

### Phase 1 Milestone
- [ ] `no_std` build verified on embedded target
- [ ] Geodesic accuracy validated against known reference vectors
- [ ] Drone Geofence Engine demo released

---

## Phase 2 (Months 4–6) — `tpt-gis-io` & `tpt-gis-index`

### tpt-gis-io (data interchange layer)
- [ ] GeoJSON reader/writer (serde-based)
- [ ] WKB (Well-Known Binary) reader/writer
- [ ] WKT (Well-Known Text) reader/writer
- [ ] Shapefile (.shp/.shx/.dbf) reader
- [ ] GeoPackage reader/writer
- [ ] Streaming / zero-copy design for large files

### tpt-gis-index (spatial indexing)
- [ ] R-Tree: bulk loading
- [ ] R-Tree: incremental insert
- [ ] Quadtree implementation
- [ ] Decide H3 strategy: pure-Rust reimplementation vs wrapping an existing pure-Rust crate (zero-FFI principle)
- [ ] H3 hexagonal grid integration
- [ ] S2 grid integration
- [ ] Spatial-join algorithm built on R-Tree

### MVP 1 — Planetary-Scale Spatial Join Engine
- [ ] CLI scaffold (clap)
- [ ] Test fixture generation (10M points / 50k polygons)
- [ ] Reprojection pipeline (via `tpt-gis-core`)
- [ ] Spatial join execution (points-in-polygons via R-Tree)
- [ ] Output writer (GeoJSON/CSV)
- [ ] Throughput benchmark vs PostGIS / GDAL `ogr2ogr`
- [ ] Publish CLI binary + benchmark report

### Phase 2 Milestone
- [ ] Spatial Join Engine CLI released
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

- [ ] CI: std targets (Linux/macOS/Windows)
- [ ] CI: `no_std` embedded target
- [ ] CI: `wasm32-unknown-unknown` target
- [ ] CI: `wasm32-wasi` target
- [ ] Unit tests per crate
- [ ] Property-based tests (proptest) for geometry/geodesic invariants
- [ ] Golden / reference-vector correctness tests
- [ ] Benchmark harness (criterion) with regression tracking
- [ ] Fuzzing harness (cargo-fuzz) for `tpt-gis-io` parsers
- [ ] Fuzzing harness (cargo-fuzz) for `tpt-gis-raster` parser
- [ ] Rustdoc coverage across all crates
- [ ] docs.rs configuration
- [ ] User-guide site
- [ ] Semver policy documented
- [ ] `CHANGELOG.md` + release automation
- [ ] crates.io publishing checklist per crate
- [ ] Correctness oracle: comparison tests vs GDAL/PROJ/GEOS outputs
- [ ] `cargo-deny` / `cargo-audit` in CI enforcing zero GPL/LGPL dependencies
