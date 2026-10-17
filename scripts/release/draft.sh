#!/usr/bin/env bash
# Creates the draft GitHub release for one app version, before any build
# runs, so every build job uploads into the same release. Idempotent: a
# re-run finds the existing release (draft or not) and reuses it.
#
#   scripts/release/draft.sh <app> <tag> <version>
#
# Prints the release id; also writes `<app>_release_id` to $GITHUB_OUTPUT.
# Needs GH_TOKEN and GITHUB_REPOSITORY.
set -euo pipefail

app=${1:?usage: draft.sh <app> <tag> <version>}
tag=${2:?missing tag}
version=${3:?missing version}
here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=apps.sh
source "$here/apps.sh"
app_meta "$app"

# /releases/tags/<tag> can't see drafts — list and filter instead.
find_id() {
  gh api --paginate "repos/$GITHUB_REPOSITORY/releases?per_page=100" \
    --jq ".[] | select(.tag_name == \"$tag\") | .id" | head -n1
}

id=$(find_id)
if [ -n "$id" ]; then
  echo "release for $tag already exists (id $id) — reusing it" >&2
else
  notes=$(mktemp)
  app_install_notes "$app" "$version" >"$notes"
  # Changelog: PRs since this app's previous tag (sort -V: 0.10 > 0.9).
  prev=$(git tag -l "beagle-$app-v*" | grep -vx "$tag" | sort -V | tail -n1 || true)
  gen=(-f tag_name="$tag")
  [ -n "$prev" ] && gen+=(-f previous_tag_name="$prev")
  { echo; gh api "repos/$GITHUB_REPOSITORY/releases/generate-notes" "${gen[@]}" --jq .body; } >>"$notes"
  # Create through the API: it returns the id directly. Listing right after
  # `gh release create` can miss a brand-new draft (eventual consistency).
  id=$(gh api -X POST "repos/$GITHUB_REPOSITORY/releases" \
    -f tag_name="$tag" -f name="Beagle $APP_TITLE v$version" \
    -F draft=true -f make_latest=false -F body=@"$notes" --jq .id)
  [ -n "$id" ] || { echo "created $tag but got no release id" >&2; exit 1; }
  echo "created draft release $tag (id $id)" >&2
fi

echo "$id"
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "${app}_release_id=$id" >>"$GITHUB_OUTPUT"
fi
