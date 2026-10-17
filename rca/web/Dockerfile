# syntax=docker/dockerfile:1

# Build the static postmortem site. Build context is the rca/ directory:
# the build reads web/ and the published RCAs under rcas/. Only incidents
# flagged `published = true` — and only their client-safe sections —
# reach the output, so the image is safe to ship publicly.
#
#   cd rca && docker build --build-context standards=../standards \
#     -f web/Dockerfile -t beagle-web .
#
# The named `standards` context supplies the monorepo's shared
# tsconfig.base.json, which web/tsconfig.json extends (../../standards from
# /src/web resolves to /standards).
#
# Content is baked in at build time; rebuild to publish new incidents.
FROM node:24-alpine AS build
# pnpm comes from corepack at the version package.json pins.
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
WORKDIR /src
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./web/
RUN cd web && corepack pnpm install --frozen-lockfile
COPY --from=standards tsconfig.base.json /standards/tsconfig.base.json
COPY web/ ./web/
COPY rcas/ ./rcas/
ARG BEAGLE_SITE_URL=""
ENV BEAGLE_SITE_URL=${BEAGLE_SITE_URL}
RUN cd web && corepack pnpm build

# Serve the static output with nginx on :8080 (unprivileged port).
FROM nginx:1.27-alpine AS serve
COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/web/dist /usr/share/nginx/html
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s \
  CMD wget -q -O /dev/null http://localhost:8080/ || exit 1
