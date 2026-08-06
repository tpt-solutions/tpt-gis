# Plan: Pure-Rust GeoPackage reader/writer for `tpt-gis-io`

## Context & decisions
- `tpt-gis-io` (non-`no_std`, heap-OK) currently supports GeoJSON/WKB/WKT/Shapefile. GeoPackage is a documented stub.
- GeoPackage is, by OGC spec, a **SQLite database file**. A correct implementation needs a SQLite file reader/writer.
- Constraint confirmed with the user: **no C FFI** (project is zero-FFI; `rusqlite`/`libsqlite3-sys` is rejected) and **no heavy external engine** (`turtle`/`limbo` rejected).
- `tpt-keystone-db` (user-suggested) was investigated and rejected: it is a Postgres-wire *server* on its own LSM storage, not SQLite-compatible and far too heavy — cannot open `.gpkg` files.
- Decision: **build a minimal, in-repo pure-Rust SQLite file reader/writer** tailored to GeoPackage's needs, living as a module inside `tpt-gis-io`. No new C dependencies → stays license-clean (`deny.toml`) and zero-FFI.

## Architecture / data flow
- New module `tpt-gis-io/src/geopackage/` with:
  - `sqlite.rs` — minimal SQLite 3 file format: DB header, varints, record serial types, leaf + interior table b-tree traversal, cell/payload parsing, plus a minimal *writer* (leaf-only b-trees).
  - `geometry.rs` — GeoPackage geometry **BLOB** decoder: `GP` magic + version + flags + srs_id + envelope + StandardGeoPackageBinary (1-byte endianness + WKB). The WKB tail is fed straight to the existing `tpt_gis_io::wkb::parse_geometry`.
  - `mod.rs` — public API + `GeoPackageError`, metadata-table handling.
- Public API:
  - `read_gpkg(bytes: &[u8]) -> Result<Vec<Feature>, GeoPackageError>` (reuse `geojson::Feature { geometry, properties: Map<String, Value> }` so output matches the GeoJSON/Shapefile readers).
  - `write_gpkg(features: &[Feature], opts) -> Result<Vec<u8>, GeoPackageError>` producing a minimal valid `.gpkg`.
- Attribute mapping: INTEGER→`Number`, REAL→`Number(f64)`, TEXT→`String`, BOOLEAN→`Bool`, DATE/DATETIME→`String`, BLOB→`String` (base64). Document the BLOB base64 choice.

## Implementation steps (ordered)
1. **`GeoPackageError`** enum in `mod.rs` (I/O, parse, unsupported, truncated).
2. **SQLite reader in `sqlite.rs`:**
   - Parse 100-byte DB header; validate magic `SQLite format 3\000`; read page size (offset 16, BE u16; `1` ⇒ 65536), reserved bytes (offset 20), compute usable size = page_size − reserved.
   - Varint reader (BE, 1–9 bytes); record serial-type decoder (0,1–4,5,6,7,8,9, ≥12 even=BLOB, ≥13 odd=TEXT, with overflow-page handling noted).
   - Page walker: page-type byte (`0x0d` leaf table, `0x05` interior table). Cell pointer array (page 1 starts at offset 100). Leaf cell = varint payload len + varint rowid + record. Interior cell = child page ptr + key; recurse.
   - Read `sqlite_master` (rootpage 1) → `(type, name, tbl_name, rootpage, sql)`; parse `CREATE TABLE` SQL for column names/types; detect `INTEGER PRIMARY KEY` (rowid alias, not stored in record).
   - Expose `read_table(rootpage) -> Vec<Vec<Value>>` + `schema()` for feature tables.
3. **GPKG metadata handling in `mod.rs`:** read `gpkg_contents`, `gpkg_geometry_columns`, `gpkg_spatial_ref_sys`; identify feature tables + their geometry column.
4. **Geometry BLOB decoder in `geometry.rs`:** parse `GP` magic (bytes 0–1), version (byte 2), flags (byte 3: envelope code = bits 1–3, header byte order = bit 3, empty flag = bit 4), `srs_id` (int32, header byte order), envelope (envelope_code × 16 bytes), then StandardGeoPackageBinary (1-byte endianness + WKB) → `wkb::parse_geometry`. Return `(Geometry, srs_id)`.
5. **`read_gpkg`:** for each feature table, for each row, decode geometry column via `geometry.rs`, map other columns to `Feature.properties`, collect `Vec<Feature>`.
6. **`write_gpkg` (MVP, leaf-only):** build a SQLite file with pages: DB header (page 1) + `sqlite_master` leaf + `gpkg_spatial_ref_sys`/`gpkg_contents`/`gpkg_geometry_columns` + one feature table. Serialize records (serial types above), GPKG geometry BLOBs (header + WKB via `wkb` encoder — note: a WKB *writer* may not exist yet; if `tpt_gis_io::wkb` has no `write`, add a minimal one or serialize from `Geometry`). Keep tables within leaf pages (no interior splitting / freelist in MVP).
7. **Wire it up:** `pub mod geopackage;` in `tpt-gis-io/src/lib.rs`; update the crate doc comment (remove the "not yet implemented" note).
8. **Docs/update todo:** `CLAUDE.md` io crate table → include GeoPackage; remove AGENTS.md/CLAUDE.md "no GeoPackage" notes; mark `todo.md` Phase-2 GeoPackage item done.

## Risks / scope cuts (explicit)
- **SQLite correctness is the hard part.** MVP read supports leaf + simple interior b-trees and inline (non-overflow) payloads; overflow-page following is best-effort and should be unit-tested but is not required for typical GeoPackage feature tables.
- **Write is intentionally limited:** leaf-only b-trees (assumes tables fit in a few pages); no R-tree spatial index, no tile/attributes extension, no compression. Document this in the module doc.
- Attribute `BLOB` → base64 string (serde_json has no bytes); acceptable for MVP.
- No guaranteed external validator (GDAL/Python) in CI; rely on round-trip + spec-based unit tests.

## Validation
- Unit tests (in `geopackage/`):
  - varint round-trip; serial-type decode; synthetic leaf + interior page parse; `sqlite_master` SQL column parse.
  - GPKG geometry header parse with a **known vector** (hand-built BLOB: little-endian header, envelope code 1, a known WKB point) asserting decoded `(Geometry, srs_id)`.
- Round-trip: `write_gpkg`(small feature set) → `read_gpkg` → assert geometry + properties equal.
- Optional (env-dependent, not CI-blocking): if `python3 -c "import sqlite3"` is available, generate a reference `.gpkg` and assert our reader parses it, and assert our writer's output opens via `sqlite3`.
- `cargo test -p tpt-gis-io`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` all clean.

## Files
- New: `tpt-gis-io/src/geopackage/{mod.rs, sqlite.rs, geometry.rs}`
- Edit: `tpt-gis-io/src/lib.rs`, `CLAUDE.md`, `AGENTS.md`, `todo.md`
- Possibly new: minimal WKB writer in `tpt-gis-io/src/wkb.rs` if none exists.
