# drone-geofence-demo

MVP 2 for the [`tpt-gis`](https://crates.io/crates/tpt-gis) engine: a real-time,
`no_std`-capable drone geofence / navigation engine — breach detection plus an
escape-vector heading on breach.

This is an example/demo binary (not published to crates.io). Run it with:

```sh
cargo run -p drone-geofence-demo
```

It ingests a simulated GPS coordinate stream, checks each position against a
pre-loaded set of no-fly-zone polygons (stored as `const`/`static` data with
zero heap allocation), and reports in/out-of-zone status plus the bearing that
escapes the nearest zone when breached.

## License

Licensed under either of [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](https://opensource.org/licenses/MIT) at your option.
