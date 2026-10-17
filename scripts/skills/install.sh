#!/usr/bin/env bash
# Installs every skill in the repo's skills/ directory for coding agents.
#
#   Claude Code: ~/.claude/skills/<name>     -> skills/<name>   (symlinked dir)
#   Codex:       ~/.codex/prompts/<name>.md  <- skills/<name>/SKILL.md body
#
# The paths match rca/cli/src/skill.rs, so `beagle update` and this script
# agree on where a skill lives. Claude gets a symlink (or a copy with
# --copy) so installs track the checkout. Codex always gets a copy without
# the YAML frontmatter, exactly what `beagle update` writes: Codex injects
# prompts verbatim, and a symlink there would let `beagle update` (which
# writes through links) strip the frontmatter from the repo's SKILL.md.
#
# Idempotent. Never replaces a real file/dir it didn't create unless --force.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/skills/install.sh [--agent claude|codex|all] [--copy]
                                 [--uninstall] [--force] [--dry-run] [skill...]

Installs skills from skills/ (all of them, or only the named ones).

  --agent A     claude, codex or all (default: all)
  --copy        copy Claude skills instead of symlinking (Codex always copies)
  --uninstall   remove installed skills (modified copies only with --force)
  --force       replace/remove existing non-symlink files
  --dry-run     print what would change, change nothing
EOF
}

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SKILLS_DIR="$REPO_ROOT/skills"

agent="all"
mode="symlink"
action="install"
force=0
dry_run=0
selected=()

while [ $# -gt 0 ]; do
  case "$1" in
    --agent)
      [ $# -ge 2 ] || { echo "error: --agent needs a value" >&2; exit 2; }
      agent="$2"
      shift 2
      ;;
    --agent=*) agent="${1#--agent=}"; shift ;;
    --copy) mode="copy"; shift ;;
    --uninstall) action="uninstall"; shift ;;
    --force) force=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h | --help) usage; exit 0 ;;
    -*) echo "error: unknown option $1" >&2; usage >&2; exit 2 ;;
    *) selected+=("$1"); shift ;;
  esac
done

case "$agent" in
  claude | codex | all) ;;
  *) echo "error: --agent must be claude, codex or all (got '$agent')" >&2; exit 2 ;;
esac

[ -n "${HOME:-}" ] || { echo "error: HOME is not set" >&2; exit 2; }

run() {
  if [ "$dry_run" -eq 1 ]; then
    echo "  would run: $*"
  else
    "$@"
  fi
}

# Skill names: every skills/<name>/SKILL.md, or the ones named on the CLI.
skill_names() {
  if [ "${#selected[@]}" -gt 0 ]; then
    printf '%s\n' "${selected[@]}"
    return
  fi
  local dir
  for dir in "$SKILLS_DIR"/*/; do
    [ -f "${dir}SKILL.md" ] && basename "$dir"
  done
}

# Prints SKILL.md without its leading YAML frontmatter block.
strip_frontmatter() {
  awk 'NR == 1 && $0 == "---" { infm = 1; next }
       infm && $0 == "---" { infm = 0; started = 1; next }
       infm { next }
       started && !body && $0 == "" { next }
       { body = 1; print }' "$1"
}

# Removes `dest` so it can be replaced. Symlinks always go; real files and
# directories only with --force. Returns 1 if it refused.
clear_dest() {
  local dest="$1"
  if [ -L "$dest" ]; then
    run rm -f "$dest"
  elif [ -e "$dest" ]; then
    if [ "$force" -eq 1 ]; then
      run rm -rf "$dest"
    else
      echo "  skip: $dest exists and isn't ours to replace (use --force)" >&2
      return 1
    fi
  fi
}

# install_one <src> <dest> <kind>; kind is claude-dir or codex-file.
install_one() {
  local src="$1" dest="$2" kind="$3" mode="$mode"
  [ "$kind" = "codex-file" ] && mode="copy"

  if [ "$mode" = "symlink" ] && [ -L "$dest" ] &&
    [ "$(readlink "$dest")" = "$src" ]; then
    echo "  ok:   $dest (already linked)"
    return 0
  fi
  if [ "$mode" = "copy" ] && [ ! -L "$dest" ] && [ -e "$dest" ]; then
    if copy_is_current "$src" "$dest" "$kind"; then
      echo "  ok:   $dest (already current)"
      return 0
    fi
  fi

  clear_dest "$dest" || return 1
  run mkdir -p "$(dirname "$dest")"
  if [ "$mode" = "symlink" ]; then
    run ln -s "$src" "$dest"
  elif [ "$kind" = "claude-dir" ]; then
    run cp -R "$src" "$dest"
  elif [ "$dry_run" -eq 1 ]; then
    echo "  would write: $dest (SKILL.md body without frontmatter)"
  else
    strip_frontmatter "$src" >"$dest"
  fi
  echo "  $mode: $dest"
}

copy_is_current() {
  local src="$1" dest="$2" kind="$3"
  if [ "$kind" = "claude-dir" ]; then
    diff -rq "$src" "$dest" >/dev/null 2>&1
  else
    strip_frontmatter "$src" | cmp -s - "$dest"
  fi
}

# uninstall_one <src> <dest> <kind>. An unmodified copy of ours counts as
# ours (a Codex install is always a copy); anything else needs --force.
uninstall_one() {
  local src="$1" dest="$2" kind="$3"
  if [ -L "$dest" ] ||
    { [ -e "$dest" ] && copy_is_current "$src" "$dest" "$kind"; }; then
    if [ -L "$dest" ]; then run rm -f "$dest" || return 1; else run rm -rf "$dest" || return 1; fi
    echo "  removed: $dest"
  elif [ -e "$dest" ]; then
    if [ "$force" -eq 1 ]; then
      run rm -rf "$dest" || return 1
      echo "  removed: $dest"
    else
      echo "  skip: $dest was modified or not installed by us (use --force)" >&2
      return 1
    fi
  fi
}

# Paths per agent; must match rca/cli/src/skill.rs (Agent::skill_path).
claude_dest() { echo "$HOME/.claude/skills/$1"; }
codex_dest() { echo "$HOME/.codex/prompts/$1.md"; }

agents=()
case "$agent" in
  all) agents=(claude codex) ;;
  *) agents=("$agent") ;;
esac

status=0
names="$(skill_names)"
[ -n "$names" ] || { echo "error: no skills found in $SKILLS_DIR" >&2; exit 1; }

while IFS= read -r name; do
  if [ ! -f "$SKILLS_DIR/$name/SKILL.md" ]; then
    echo "error: no skill '$name' (expected $SKILLS_DIR/$name/SKILL.md)" >&2
    status=1
    continue
  fi
  echo "$name"
  for a in "${agents[@]}"; do
    if [ "$a" = "claude" ]; then
      dest="$(claude_dest "$name")"
      src="$SKILLS_DIR/$name"
      kind="claude-dir"
    else
      dest="$(codex_dest "$name")"
      src="$SKILLS_DIR/$name/SKILL.md"
      kind="codex-file"
    fi
    if [ "$action" = "install" ]; then
      install_one "$src" "$dest" "$kind" || status=1
    else
      uninstall_one "$src" "$dest" "$kind" || status=1
    fi
  done
done <<<"$names"

if [ "$status" -ne 0 ]; then
  echo "finished with skipped items (see above)" >&2
fi
exit "$status"
