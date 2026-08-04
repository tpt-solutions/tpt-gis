# Security Policy

## Supported Versions

`tpt-gis` is in Phase 1 (pre-1.0, active design/development — see `todo.md`).
Until a 1.0 release, only the latest published version of each crate receives
security fixes.

## Reporting a Vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Instead, report it privately via GitHub's ["Report a vulnerability"][advisory]
feature on this repository, or email **security@tpt-solutions.dev**
*(placeholder — update before this repository is made public)*.

Please include:

- A description of the vulnerability and its potential impact
- Steps to reproduce, or a proof-of-concept if available
- The affected crate(s) and version(s)

We aim to acknowledge reports within 5 business days. Once a fix is available,
we will coordinate disclosure timing with the reporter and credit them (unless
they prefer to remain anonymous) in the release notes.

## Scope

`tpt-gis-io` and `tpt-gis-raster` parse untrusted, potentially adversarial input
(GeoJSON, WKB, Shapefile, GeoPackage, GeoTIFF/COG) by design — memory-safety
issues, panics-on-malformed-input, or resource-exhaustion bugs in these parsers
are in scope and treated as security issues, not ordinary bugs. Denial-of-service
via extremely large but well-formed inputs is a known, lower-severity class we
track separately in `todo.md` (streaming design) rather than treat as a
vulnerability report.

[advisory]: https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability
