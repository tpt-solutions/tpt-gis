# The `tptgis` command-line tool in a small runtime image.
#
#   docker build -t tptgis .
#   docker run --rm -v "$PWD:/data" -w /data tptgis info cities.geojson
#
# The image has no GDAL, PROJ or GEOS: the engine is pure Rust.

FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
# The root rust-toolchain.toml pins the toolchain channel; rustup installs it here.
RUN cargo build --release --locked -p tpt-gis-cli

FROM debian:bookworm-slim
COPY --from=build /src/target/release/tptgis /usr/local/bin/tptgis
ENTRYPOINT ["tptgis"]
CMD ["--help"]
