# canvist website: a static Astro build served by Caddy.
#
# Build from the repository root, because the site reads the docs, the brand
# assets and the canvist packages from the rest of the repo:
#
#   docker build -f website/Dockerfile .
#
# On Railway the `website` service sets `dockerfilePath` to website/Dockerfile
# and builds from the repository root (Railway retired railway.json in favour
# of a project-wide .railway/railway.ts, which is not set up yet).

# The landing page embeds the live editor (`@canvist/web`) and renders its
# document at build time (`@canvist/headless/ssr`). Both are compiled from
# the crates here, with the packages' own build scripts, as `web.yml` does.
FROM node:24.14.0-trixie-slim AS node
FROM rust:1.96.0-slim-trixie AS packages
# The image's toolchain is the one `rust-toolchain.toml` pins; naming it
# stops rustup from installing the toolchain file's editor components.
ENV RUSTUP_TOOLCHAIN=1.96.0 \
	CARGO_TERM_COLOR=never \
	CARGO_INCREMENTAL=0
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules/npm /usr/local/lib/node_modules/npm
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
	&& rustup target add wasm32-unknown-unknown \
	&& cargo install --locked wasm-pack@0.14.0
WORKDIR /repo
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
COPY packages/canvist-headless packages/canvist-headless
COPY packages/canvist-web packages/canvist-web
# The site uses each package's `dist` and WebAssembly; their own
# dependencies are build tools, so `node_modules` goes once they're built.
RUN cd packages/canvist-headless \
	&& npm ci \
	&& npm run build:wasm \
	&& npm run build \
	&& cd ../canvist-web \
	&& npm ci \
	&& npm run build:wasm \
	&& npm run build \
	&& rm -rf ../canvist-headless/node_modules node_modules /repo/target

FROM denoland/deno:2.8.3 AS build
WORKDIR /repo
# What the site reads from the rest of the repo: the book, the brand assets,
# crate and package readmes (for /docs), and the packages built above, over
# their sources. The context is trimmed by Dockerfile.dockerignore.
COPY docs/src docs/src
COPY assets/brand assets/brand
COPY crates crates
COPY packages packages
COPY --from=packages /repo/packages/canvist-headless packages/canvist-headless
COPY --from=packages /repo/packages/canvist-web packages/canvist-web
WORKDIR /repo/website
COPY website/package.json website/deno.json website/deno.lock ./
RUN deno install --frozen
COPY website .
# Railway passes the service's public domain as a build variable, which sets
# the canonical URL; SITE_URL overrides it.
ARG RAILWAY_PUBLIC_DOMAIN
ARG SITE_URL
RUN deno task build

FROM caddy:2.11.6-alpine
COPY website/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /repo/website/dist /srv
EXPOSE 8080
