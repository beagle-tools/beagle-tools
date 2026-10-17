#!/usr/bin/env bash
# Publishes one app's draft release once every build uploaded, then mirrors
# its stable-named assets onto the rolling `beagle-<app>-latest` release —
# the URLs the in-app updater, install scripts and casks read.
#
#   scripts/release/publish.sh <app> <tag> <version>
#
# make_latest=false always: no app owns the repo-wide "latest" release.
# Needs GH_TOKEN, GITHUB_REPOSITORY and GITHUB_SHA.
set -euo pipefail

app=${1:?usage: publish.sh <app> <tag> <version>}
tag=${2:?missing tag}
version=${3:?missing version}
here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=apps.sh
source "$here/apps.sh"
app_meta "$app"
export GH_REPO="$GITHUB_REPOSITORY"

# /releases/tags/<tag> can't see drafts — list and filter instead.
id=$(gh api --paginate "repos/$GITHUB_REPOSITORY/releases?per_page=100" \
  --jq ".[] | select(.tag_name == \"$tag\") | .id" | head -n1)
[ -n "$id" ] || { echo "no release found for $tag" >&2; exit 1; }
gh api -X PATCH "repos/$GITHUB_REPOSITORY/releases/$id" \
  -F draft=false -f make_latest=false >/dev/null
echo "published $tag"

stage=$(mktemp -d)
patterns=(--pattern "$APP_PRODUCT-*" --pattern 'latest.json')
if [ -n "$APP_FORMULA" ]; then
  patterns+=(--pattern "$APP_FORMULA-*")
fi
gh release download "$tag" --dir "$stage" "${patterns[@]}"
if [ ! -f "$stage/latest.json" ]; then
  # Without it the in-app updater keeps seeing the previous version.
  echo "::warning::$tag has no latest.json — is createUpdaterArtifacts on in tauri.conf.json?"
fi
ls -l "$stage"

if ! gh release view "$APP_MIRROR" >/dev/null 2>&1; then
  gh release create "$APP_MIRROR" --target "$GITHUB_SHA" --latest=false \
    --title "Beagle $APP_TITLE (latest)" --notes "Rolling mirror."
fi
gh release upload "$APP_MIRROR" "$stage"/* --clobber
gh release edit "$APP_MIRROR" --latest=false --notes "$(
  echo "Rolling mirror of the newest \`beagle-$app-v*\` release — currently [v$version]($REPO_URL/releases/tag/$tag)."
  echo "Assets are replaced on every release; use these URLs for stable downloads."
  echo
  app_install_notes "$app" "$version"
)" >/dev/null
echo "$APP_MIRROR now mirrors $tag"
