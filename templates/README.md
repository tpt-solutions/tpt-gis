# Project templates

Starting points for new projects that use tpt-gis. Each is a
[`cargo-generate`](https://github.com/cargo-generate/cargo-generate) template:

```sh
cargo install --locked cargo-generate
cargo generate --git https://github.com/tpt-solutions/tpt-gis templates/no-std-geofence
cargo generate --git https://github.com/tpt-solutions/tpt-gis templates/cli-tool
```

| Template | What it is |
| -------- | ---------- |
| `no-std-geofence` | A `no_std` point-in-zone check over `const` polygon data, for firmware |
| `cli-tool` | A command-line program that prints the WGS84 geodesic distance and bearing between two points |

The templates depend on the repository, since the crates are not on crates.io yet. While
developing against a local checkout, change the dependency to a `path` dependency.

These templates are not workspace members, so `cargo build --workspace` does not build them.
They were checked by filling in the placeholders in a scratch copy and running `cargo test`
(`no-std-geofence`) and `cargo run` (`cli-tool`).
