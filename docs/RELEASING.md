# Releasing

Releases are fully automatic. **Merging to `main` is releasing**: the
`release` workflow (`.github/workflows/release.yml`) ships every app whose
code changed, and nothing else. There is no release branch, no manual
version bump and no release PR.

## What triggers a release

Every push to `main` runs `release`. For each app (`tandem`, `rca`,
`workflows`, and `cli`, the `beagle` base CLI in `cli-base/`) it diffs the push (`github.event.before..HEAD`) and releases the
app if any **release-relevant** path changed:

| App       | Release-relevant paths                                                           |
| --------- | -------------------------------------------------------------------------------- |
| tandem    | `tandem/**`                                                                      |
| rca       | `rca/**`, `skills/beagle/**`, `skills/beagle-review/**` (embedded in the binary) |
| workflows | `workflows/**`                                                                   |
| cli       | `cli-base/**`                                                                    |
| all       | `utils/**`, `rust-toolchain.toml` (compiled into every app)                      |

Excluded everywhere: `**/*.md` and `<app dir>/docs/**`. Docs-only merges don't
release. The two `skills/` paths above are the one exception, because rca
embeds those `SKILL.md` files. Changes to CI, `scripts/` or this
workflow don't release anything by themselves either.

If the push's previous SHA is unknown (the first push, or a force push),
every app counts as changed.

The rules live in `scripts/release/lib.mjs` and are unit tested in
`scripts/release/plan.test.mjs`. Keep this doc, that file and the tests in
step.

## Versions: semver from PR labels

Each app has its own version line and tags: `beagle-<app>-vX.Y.Z`. For an
app that releases:

1. If `HEAD` already carries a `beagle-<app>-v*` tag, that version is
   released as-is. This covers re-runs and manual re-releases.
2. Else if `main` has moved past this commit (the run waited in the queue),
   it is skipped. The newer push's run releases it.
3. Else if the app has no tag yet, the version comes from its version file:
   `tandem/src-tauri/tauri.conf.json`, `rca/cli/Cargo.toml` `[package]`,
   `workflows/Cargo.toml` or `cli-base/Cargo.toml` `[workspace.package]`.
4. Else the app's highest tag is bumped by the merged PR's label:

| PR label                  | Bump              |
| ------------------------- | ----------------- |
| `release:major`           | `1.4.2` → `2.0.0` |
| `release:minor`           | `1.4.2` → `1.5.0` |
| none (or `release:patch`) | `1.4.2` → `1.4.3` |

One label applies to every app the PR releases.

**First release.** The first push that releases each app tags its current
version-file value: `beagle-tandem-v0.0.1`, `beagle-rca-v0.29.0`,
`beagle-workflows-v0.1.0` and `beagle-cli-v0.1.0`. From then on, bumps come
from labels.

Version files in the repo aren't bumped by commits. Release CI runs
`node scripts/set-version.mjs <X.Y.Z>` in each app dir to stamp every version
location (package.json, tauri.conf.json, Cargo.toml, Cargo.lock), and never
commits the result. So the built binaries and `--version` always report the
tag.

## Pipeline

```
plan ─┬─ tandem-desktop ──────────────────────────► tandem-publish ──► homebrew
      ├─ rca-cli + rca-desktop + rca-web ─────────► rca-publish ─────► homebrew
      ├─ workflows-cli + workflows-desktop ───────► workflows-publish ► homebrew
      └─ cli-build ───────────────────────────────► cli-publish ───────► homebrew (no-op)
```

- **plan** (`scripts/release/plan.mjs`) decides the versions, then pushes the
  tags. It also creates one **draft** release per app
  (`scripts/release/draft.sh`), titled `Beagle Tandem vX.Y.Z` /
  `Beagle RCA vX.Y.Z` / `Beagle Workflows vX.Y.Z` / `Beagle CLI vX.Y.Z`. Each draft has install
  instructions plus generated notes for the PRs since the app's previous tag.
- **build** jobs upload into that draft:
  - `_release-tauri.yml` builds the desktop apps with tauri-action. It signs
    the updater bundles, merges `latest.json`, and adds stable-named copies of
    the installers.
  - `_release-cli.yml` cross-builds the CLIs. It runs a `--version` smoke
    test on native targets, then uploads tarballs plus `.sha256` files.
  - `rca-web` pushes the Docker image.
- **publish** (`scripts/release/publish.sh`) runs only if every build for that
  app succeeded. It un-drafts the release with `make_latest=false`, so no app
  owns the repo-wide "latest". Then it mirrors the stable-named assets and
  `latest.json` onto the app's rolling release. If any build fails, the draft
  stays hidden and the mirror is untouched.
- **homebrew** (`scripts/release/homebrew.sh`) renders the app's formula and
  copies its casks into `beagle-tools/homebrew-tap`, then commits
  `beagle-<app> X.Y.Z` and pushes.

Runs serialize on the `release-main` concurrency group and are never
cancelled.

## Assets

The versioned release (`beagle-<app>-vX.Y.Z`) holds Tauri's versioned
bundles, the updater bundles and signatures, `latest.json`, the stable-named
installers and the CLI tarballs.

The rolling mirror release `beagle-<app>-latest` (title
`Beagle <App> (latest)`) has its assets replaced on every release:

| App       | Mirror assets                                                                                                                                                         |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| tandem    | `Tandem-macos-arm64.dmg`, `Tandem-macos-x64.dmg`, `Tandem-windows-x64-setup.exe`, `Tandem-linux-x86_64.AppImage`, `Tandem-linux-amd64.deb`, `latest.json`             |
| rca       | `Beagle-macos-arm64.dmg`, `Beagle-macos-x64.dmg`, `Beagle-linux-x86_64.AppImage`, `Beagle-linux-amd64.deb`, `latest.json`, `beagle-rca-<target>.tar.gz` (+ `.sha256`) |
| workflows | `Agentdesk-macos-arm64.dmg`, `Agentdesk-macos-x64.dmg`, `latest.json`, `beagle-workflows-<target>.tar.gz` (+ `.sha256`)                                               |
| cli       | `beagle-cli-<target>.tar.gz` (+ `.sha256`)                                                                                                                            |

CLI tarballs hold the binaries at the tarball root:

- `beagle-rca-<target>.tar.gz` holds `beagle` and `beagle-agentd`. Targets:
  `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl`.
- `beagle-workflows-<target>.tar.gz` holds `agentd`, `agentctl` and
  `agenttop`. Targets: the two macOS targets.
- `beagle-cli-<target>.tar.gz` holds `beagle`, the base CLI. Targets: the
  same three as rca. It has no Homebrew formula yet; `beagle update` updates
  it in place. Extension tarballs (`beagle-<app>-ext-<target>.tar.gz`, with an
  `extension.toml` at the root) are what `beagle extensions install` reads;
  apps start publishing them as each ships its extension
  (`cli-base/AGENTS.md`).

rca also pushes `ghcr.io/beagle-tools/beagle-rca-web:<X.Y.Z>` and `:latest`.

Stable URL: `https://github.com/beagle-tools/beagle-tools/releases/download/beagle-<app>-latest/<asset>`.

## In-app updates

Each desktop app's Tauri updater polls
`https://github.com/beagle-tools/beagle-tools/releases/download/beagle-<app>-latest/latest.json`.
It verifies bundles against the shared updater public key, and updates
from the in-app update button. The in-app version list (for rollback)
filters releases by the tag prefix `beagle-<app>-v`.

## Homebrew

```sh
brew install beagle-tools/tap/beagle-rca              # beagle + beagle-agentd (macOS, Linux x86_64)
brew install beagle-tools/tap/beagle-workflows        # agentd, agentctl, agenttop (macOS)
brew install --cask beagle-tools/tap/beagle-tandem    # Tandem.app
brew install --cask beagle-tools/tap/beagle-rca-desktop  # Beagle.app
brew install --cask beagle-tools/tap/beagle-agentdesk    # Agentdesk.app
```

Sources live in `packaging/homebrew/`. Edit them there, never in the tap,
because the next release overwrites the tap.

- **Formulas** (`Formula/*.rb.tmpl`) are versioned. Release CI fills in
  `@VERSION@` and a sha256 per target (`@SHA_<TARGET>@`) from the release's
  `.sha256` assets.
- **Casks** (`Casks/*.rb`) are static: `version :latest` and
  `sha256 :no_check`, pointing at the mirror. They clear the quarantine flag
  after install. After the first install, the apps update themselves.

To render locally, see the header of `scripts/release/homebrew.sh`.

## Manual re-release and rollback

- **Re-run a failed release:** re-run the workflow run. The tag is already
  at that commit, so the same version is rebuilt into the same draft. If
  `main` has moved on since, use the dispatch below instead.
- **Re-release / force a release of the current `main`:** run
  _Actions → release → Run workflow_ with `apps` = `all` or e.g. `rca,tandem`.
  Apps with no tag at HEAD get a new bumped version (patch, unless the PR
  that produced HEAD has a label). To pin the version, first push the tag
  yourself:
  `git tag beagle-rca-v0.30.0 origin/main && git push origin beagle-rca-v0.30.0`.
  Then dispatch.
- **Roll back:** don't delete tags. Prefer fixing forward: revert on `main`,
  and the next release ships the revert under a new version. Users can pick
  an older version from the in-app version list, or download it from its
  versioned release.
- **Pull a broken release:** mark the versioned release as a pre-release, or
  delete it. Then re-point the mirror by re-running the publish job of the
  last good version's workflow run.

## Secrets

| Secret                      | Used by                | Notes                                                                                         |
| --------------------------- | ---------------------- | --------------------------------------------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY` | `_release-tauri.yml`   | Updater signing key, no password. The workflow sets `TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ""`. |
| `HOMEBREW_TAP_DEPLOY_KEY`   | `_release-publish.yml` | SSH deploy key with write access to `beagle-tools/homebrew-tap`. Preferred.                   |
| `HOMEBREW_TAP_TOKEN`        | `_release-publish.yml` | Fallback: a token with contents:write on the tap. Used only when there's no deploy key.       |

If neither tap secret is set, the homebrew job logs a warning and skips. The
release itself still completes. `GITHUB_TOKEN` handles everything else: tags,
releases and GHCR, through `contents: write` and `packages: write`.

## Testing the pipeline locally

```sh
node --test scripts/release/plan.test.mjs
node scripts/release/plan.mjs --dry-run --before <sha> --after <sha> [--labels release:minor]
```

`--dry-run` skips the stale-main check, the GitHub API and the tag push.
It prints which apps would release and at which version.
