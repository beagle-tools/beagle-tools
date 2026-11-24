#!/bin/sh
# agentd's `gh`, first on the PATH of every run whose manifest does not set
# `allow_github_writes: true`. Agents read GitHub freely but must not change
# it on their own: this refuses `gh pr merge`, `gh pr review --approve` and
# any `gh api` call that is not a read (a -X/--method other than GET, a
# request body via -f/-F/--field/--raw-field/--input, or a GraphQL
# mutation). Everything else goes to the real gh unchanged.
set -u
self_dir=$(cd "$(dirname "$0")" && pwd)

block() {
  echo "agentd: BLOCKED gh $1 — agents under agentd cannot write to GitHub (set allow_github_writes: true in the job's manifest to opt out)" >&2
  exit 1
}

# The subcommand words, skipping gh's global -R/--repo flag.
cmd=""
sub=""
skip=0
for arg in "$@"; do
  if [ "$skip" = 1 ]; then skip=0; continue; fi
  case "$arg" in
    -R|--repo) skip=1; continue ;;
    -R*|--repo=*) continue ;;
  esac
  if [ -z "$cmd" ]; then cmd=$arg; continue; fi
  if [ -z "$sub" ]; then sub=$arg; break; fi
done

case "$cmd" in
  pr)
    case "$sub" in
      merge) block "pr merge" ;;
      review)
        for arg in "$@"; do
          case "$arg" in -a|--approve) block "pr review --approve" ;; esac
        done ;;
    esac ;;
  api)
    method=""
    body=0
    graphql=0
    next=""
    for arg in "$@"; do
      if [ "$next" = method ]; then method=$arg; next=""; continue; fi
      case "$arg" in
        -X|--method) next=method ;;
        -X*) method=${arg#-X} ;;
        --method=*) method=${arg#--method=} ;;
        -f|-F|--field|--raw-field|--input|-f*|-F*|--field=*|--raw-field=*|--input=*) body=1 ;;
        graphql) graphql=1 ;;
      esac
      case $(printf '%s' "$arg" | tr '[:upper:]' '[:lower:]') in
        *mutation*) [ "$graphql" = 1 ] && block "api graphql mutation" ;;
      esac
    done
    method=$(printf '%s' "$method" | tr '[:lower:]' '[:upper:]')
    if [ -n "$method" ]; then
      [ "$method" = GET ] || [ "$method" = HEAD ] || block "api -X $method"
    elif [ "$body" = 1 ] && [ "$graphql" = 0 ]; then
      block "api with a request body (POST)"
    fi ;;
esac

# The real gh: the first one on PATH that is not this shim.
real=""
IFS=:
for d in $PATH; do
  [ "$d" = "$self_dir" ] && continue
  [ -x "$d/gh" ] && { real="$d/gh"; break; }
done
unset IFS
[ -n "$real" ] || { echo "agentd gh shim: no real gh on PATH" >&2; exit 127; }
exec "$real" "$@"
