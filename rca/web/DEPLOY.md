# Deploying the Beagle RCA postmortem site

The site is a static build, so you can deploy it three ways — as raw
files, as a container, or on Kubernetes — and gate it with GitHub SSO
whenever you want. Content (published RCAs) is baked in at build time;
rebuild to publish new incidents.

## 1. Static files (simplest)

```sh
cd rca/web
corepack pnpm install --frozen-lockfile
BEAGLE_SITE_URL=https://postmortems.example.com corepack pnpm build
# → rca/web/dist/  — upload anywhere: Vercel, Netlify, Pages, S3, nginx, …
```

By default the build reads the repo's example store (`../examples/rcas`).
Point `BEAGLE_RCAS_DIR` at your own `rcas/` directory to publish your
incidents (only RCAs marked with `beagle rca publish` are included).

## 2. Docker

Build from the `rca/` directory. The image bakes in the RCAs under
`rca/examples/rcas/` (published ones only), plus the repo's shared
`standards/` as a named build context:

```sh
cd rca
docker build --build-context standards=../standards \
  --build-arg BEAGLE_SITE_URL=https://postmortems.example.com \
  -f web/Dockerfile -t beagle-web .
docker run -p 8080:8080 beagle-web        # → http://localhost:8080
```

To ship your own incidents in the image, put your published RCA folders in
`rca/examples/rcas/` of the build context (or build a static `dist/` as in
section 1 and serve that instead).

Or run the image with Compose. Its `build:` section does not pass the
`standards` context, so build the image with the command above first, then
start it without `--build`. **Public:**

```sh
cd rca/web
docker compose up                         # → http://localhost:8080
```

**Gated by GitHub SSO** (bring your own OAuth app):

```sh
cd rca/web
cp .env.example .env                       # fill in your GitHub OAuth app
docker compose --profile auth up           # → http://localhost:4180
```

With auth on, reach the site through the proxy on `:4180`; do not expose
the un-gated web container (`:8080`) to the internet.

### Creating the GitHub OAuth app

GitHub → Settings → Developer settings → **OAuth Apps** → New OAuth App.
Set the **Authorization callback URL** to
`https://<your-host>/oauth2/callback` (or
`http://localhost:4180/oauth2/callback` locally). Put the client ID and
secret in `.env`, and generate a cookie secret with
`openssl rand -base64 32`. Restrict who can view with `GITHUB_ORG` /
`GITHUB_TEAM`.

## 3. Kubernetes (Helm)

The chart lives at `rca/deploy/helm/beagle-web`. It deploys the image (built
above and pushed to a registry), an ingress, and — optionally — the
GitHub-SSO proxy.

**Public:**

```sh
helm install postmortems rca/deploy/helm/beagle-web \
  --set image.repository=ghcr.io/you/beagle-web --set image.tag=0.1.0 \
  --set ingress.enabled=true --set ingress.host=postmortems.example.com
```

Or use the released image (example content only, see below):
`--set image.repository=ghcr.io/beagle-tools/beagle-rca-web --set image.tag=<version>`.

**Gated by GitHub SSO:**

```sh
helm install postmortems rca/deploy/helm/beagle-web \
  --set image.repository=ghcr.io/you/beagle-web --set image.tag=0.1.0 \
  --set ingress.enabled=true --set ingress.host=postmortems.example.com \
  --set ingress.tlsSecret=postmortems-tls \
  --set auth.enabled=true \
  --set auth.github.clientId=<client-id> \
  --set auth.github.org=<your-org> \
  --set auth.redirectUrl=https://postmortems.example.com/oauth2/callback \
  --set auth.clientSecret=<client-secret> \
  --set auth.cookieSecret=$(openssl rand -base64 32)
```

When `auth.enabled=true` the ingress routes to oauth2-proxy, which
authenticates against GitHub and forwards to the site. Prefer keeping
secrets out of `--set`: put `client-secret` and `cookie-secret` in your
own Secret and pass `--set auth.existingSecret=<name>`. See
`rca/deploy/helm/beagle-web/values.yaml` for every knob.

## Publishing the image (GHCR)

Every rca release (`beagle-rca-vX.Y.Z`, cut from `main` by the repo's
release automation) publishes the image as
`ghcr.io/beagle-tools/beagle-rca-web:X.Y.Z` and `:latest`, alongside the CLI
and desktop builds of the same version. That image bakes in the repo's
example store, so build your own image to publish your incidents.
