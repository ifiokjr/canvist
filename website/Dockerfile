# canvist website: a static Astro build served by Caddy.
#
# Build from the repository root, because the site reads the docs, the brand
# assets and the v1 engine from the rest of the repo:
#
#   docker build -f website/Dockerfile .
#
# On Railway the `website` service sets `dockerfilePath` to website/Dockerfile
# and builds from the repository root (Railway retired railway.json in favour
# of a project-wide .railway/railway.ts, which is not set up yet).

FROM denoland/deno:2.8.3 AS build
WORKDIR /repo
COPY docs/src docs/src
COPY assets/brand assets/brand
COPY packages/canvist/src packages/canvist/src
COPY packages/canvist/wasm packages/canvist/wasm
COPY website/package.json website/deno.json website/deno.lock website/
WORKDIR /repo/website
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
