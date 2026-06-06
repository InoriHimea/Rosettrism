#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/check-plan-requirement.sh [--staged | --base <ref>]

Checks that feature-sensitive changes are accompanied by a development plan
update under .plan/ and/or an update to requirement.md.

Options:
  --staged      Check staged changes (useful for pre-commit hooks).
  --base <ref>  Check changes against a git ref. Defaults to HEAD~1 when present,
                otherwise HEAD.
  -h, --help    Show this help message.
USAGE
}

mode="base"
base_ref=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --staged)
      mode="staged"
      shift
      ;;
    --base)
      if [[ $# -lt 2 ]]; then
        echo "error: --base requires a ref" >&2
        exit 2
      fi
      mode="base"
      base_ref="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$mode" == "staged" ]]; then
  mapfile -t changed_files < <(git diff --cached --name-only --diff-filter=ACMRT)
else
  if [[ -z "$base_ref" ]]; then
    if git rev-parse --verify HEAD~1 >/dev/null 2>&1; then
      base_ref="HEAD~1"
    else
      base_ref="HEAD"
    fi
  fi
  mapfile -t changed_files < <(
    {
      git diff --name-only --diff-filter=ACMRT "$base_ref" --
      git ls-files --others --exclude-standard
    } | sort -u
  )
fi

if [[ ${#changed_files[@]} -eq 0 ]]; then
  echo "plan/requirement check: no changed files"
  exit 0
fi

plan_or_requirement_changed=false
feature_sensitive_changed=false
feature_files=()

for file in "${changed_files[@]}"; do
  case "$file" in
    requirement.md|.plan/*.md)
      plan_or_requirement_changed=true
      ;;
  esac

  case "$file" in
    src/*|tests/*|schema/*|scripts/*|frontend/src/*|frontend/index.html|frontend/package.json|frontend/package-lock.json|Cargo.toml|Cargo.lock|Dockerfile)
      feature_sensitive_changed=true
      feature_files+=("$file")
      ;;
  esac
done

if [[ "$feature_sensitive_changed" == "false" ]]; then
  echo "plan/requirement check: no feature-sensitive files changed"
  exit 0
fi

if [[ "$plan_or_requirement_changed" == "true" ]]; then
  echo "plan/requirement check: ok"
  exit 0
fi

{
  echo "plan/requirement check: failed"
  echo "Feature-sensitive files changed without updating requirement.md or .plan/:"
  printf '  - %s\n' "${feature_files[@]}"
  echo
  echo "Update requirement.md and/or add a .plan/YYYY-MM-DD-topic.md entry before committing."
} >&2
exit 1
