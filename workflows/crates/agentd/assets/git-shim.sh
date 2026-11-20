#!/bin/sh
# agentd's `git`, first on every run's PATH. It pins core.hooksPath to
# agentd's hook dispatcher (see githook.sh) for every git call — a run that
# unsets the env var still gets it — and refuses the two ways around a
# hook: `--no-verify` on a push, and overriding core.hooksPath with -c.
# Everything else goes to the real git unchanged.
set -u
self_dir=$(cd "$(dirname "$0")" && pwd)

push=0
for arg in "$@"; do
  case "$arg" in push) push=1 ;; esac
done
for arg in "$@"; do
  case $(printf '%s' "$arg" | tr '[:upper:]' '[:lower:]') in
    --no-verify)
      [ "$push" = 1 ] && { echo "agentd: BLOCKED git push --no-verify — agent pushes always run the push guard" >&2; exit 1; } ;;
    core.hookspath=*|-ccore.hookspath=*|--config-env=core.hookspath*)
      echo "agentd: BLOCKED overriding core.hooksPath — agent git runs always use agentd's hooks" >&2; exit 1 ;;
  esac
done
case $(printf '%s' "${GIT_CONFIG_PARAMETERS:-}" | tr '[:upper:]' '[:lower:]') in
  *core.hookspath*) echo "agentd: BLOCKED overriding core.hooksPath via GIT_CONFIG_PARAMETERS" >&2; exit 1 ;;
esac

# The real git: the first one on PATH that is not this shim.
real=""
IFS=:
for d in $PATH; do
  [ "$d" = "$self_dir" ] && continue
  [ -x "$d/git" ] && { real="$d/git"; break; }
done
unset IFS
[ -n "$real" ] || { echo "agentd git shim: no real git on PATH" >&2; exit 127; }

# Appended after the caller's own GIT_CONFIG_* pairs (never replacing
# them); the last value of a key wins, so this hooksPath always applies.
n=${GIT_CONFIG_COUNT:-0}
export "GIT_CONFIG_KEY_$n=core.hooksPath" "GIT_CONFIG_VALUE_$n=$(dirname "$self_dir")/githooks"
export GIT_CONFIG_COUNT=$((n + 1))
exec "$real" "$@"
