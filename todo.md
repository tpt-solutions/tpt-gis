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
- [ ] Six untracked `CHANGELOG.md` files (one per crate) plus
      `tpt-gis-core/examples/geodesic_demo.rs` and
      `tpt-gis-geom/examples/geometry_demo.rs` sit in the working tree from a prior
      session but aren't committed — `cargo package`/`cargo publish` only includes
      VCS-tracked files by default, so these need `git add`/commit before any
      crates.io publish
