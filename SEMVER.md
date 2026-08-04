# Semver Policy

`tpt-gis` follows [Semantic Versioning](https://semver.org/) for all published crates.

## Versions

- **Major (`X.0.0`)**: Breaking API changes. A major bump means that code using the
  previous major version will not compile without changes.
- **Minor (`0.X.0`)**: New functionality in a backward-compatible manner. Existing
  code continues to compile and behave the same.
- **Patch (`0.0.X`)**: Backward-compatible bug fixes only.

## Stability Guarantees

- `no_std` compatibility is a **supported guarantee**, not a feature flag that may
  be removed. Crates will remain `no_std`-compatible across minor versions unless a
  major version bump is performed.
- Internal implementation details (private modules, helper types) may change at any
  time without a version bump, as long as the public API surface is unchanged.
- Public APIs marked `#[unstable]` or behind a `unstable` feature flag are exempt
  from semver guarantees until they are stabilized.

## Pre-release Versions

Pre-release versions (`0.1.0-alpha.1`, `0.1.0-beta.2`, etc.) are not subject to
semver guarantees. Anything may change between pre-releases.
