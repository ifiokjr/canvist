# canvist Cloud: the `canvist-cloud` binary on a small Debian base.
#
# Build from the repository root, which the workspace needs:
#
#   docker build -f services/cloud/Dockerfile -t canvist-cloud .
#
# Railway builds this for both the `sync` and `convert` services; they
# differ only in `CANVIST_ROLE`.

FROM rust:1.96.0-slim-trixie AS build
WORKDIR /src
# The image's toolchain is the one `rust-toolchain.toml` pins; naming it
# stops rustup from installing the toolchain file's editor components.
ENV RUSTUP_TOOLCHAIN=1.96.0 \
	CARGO_TERM_COLOR=never \
	CARGO_INCREMENTAL=0
COPY . .
RUN cargo build --release --locked -p canvist_cloud --bin canvist-cloud \
	&& cp target/release/canvist-cloud /usr/local/bin/canvist-cloud \
	&& rm -rf target

FROM debian:trixie-slim
# Root certificates for webhooks and Stripe; nothing else is needed.
RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates \
	&& rm -rf /var/lib/apt/lists/* \
	&& useradd --system --uid 10001 --no-create-home canvist
COPY --from=build /usr/local/bin/canvist-cloud /usr/local/bin/canvist-cloud
USER canvist
ENV CANVIST_LOG_FORMAT=json \
	PORT=8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/canvist-cloud"]
