#!/usr/bin/env bash
# Upgrades a throwaway consumer from the second-latest BCR release of rules_dx
# to the latest and back, running tests after each step.
# Usage: upgrade_e2e.sh [<previous> <latest>]
# With "local" as <latest>, the upgrade target is this checkout (through
# local_path_override) at the version in the root MODULE.bazel.
set -euo pipefail

bcr_url="${BCR_URL:-https://bcr.bazel.build}"

last_two_versions() {
  jq -r '(.versions - (.yanked_versions // {} | keys)) | .[-2:] | join(" ")'
}

if [ "$#" -eq 2 ]; then
  from=$1
  to=$2
else
  versions=$(curl -fsSL "$bcr_url/modules/rules_dx/metadata.json" | last_two_versions)
  read -r from to <<<"$versions" || true
  if [ -z "${to:-}" ]; then
    echo "upgrade-e2e: fewer than two rules_dx versions on the BCR"
    case "${GITHUB_EVENT_NAME:-}" in
      workflow_dispatch | release) exit 1 ;;
    esac
    exit 0
  fi
fi

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
use_local=0
if [ "$to" = local ]; then
  use_local=1
  to=$(sed -n '/^module(/,/^)/s/^ *version = "\([^"]*\)".*/\1/p' "$repo_root/MODULE.bazel" | head -n 1)
  [ -n "$to" ] || { echo "upgrade-e2e: no version in $repo_root/MODULE.bazel"; exit 1; }
fi
echo "upgrade-e2e: $from -> $to -> $from"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"

cp "$repo_root/.bazelversion" .bazelversion
cat >MODULE.bazel <<MODULE
module(name = "upgrade_e2e")

bazel_dep(name = "rules_dx", version = "$from")
MODULE

dx() { bazel run @rules_dx//:dx -- "$@"; }

expect_like_fresh() {
  local fresh="$work/fresh-$1"
  rm -rf "$fresh"
  mkdir -p "$fresh"
  cp "$repo_root/.bazelversion" "$fresh/.bazelversion"
  printf 'module(name = "upgrade_e2e")\n\nbazel_dep(name = "rules_dx", version = "%s")\n' "$1" >"$fresh/MODULE.bazel"
  (cd "$fresh" && dx init)
  local path
  for path in .github/workflows/ci.yml tools/dx tools/dx.cmd; do
    diff "$fresh/$path" "$path" || { echo "$path differs from a fresh init at $1"; exit 1; }
  done
}

expect_version() {
  grep -q "version = \"$1\"" MODULE.bazel || { echo "MODULE.bazel is not at $1"; exit 1; }
  grep -q "$1" .dx/pins || { echo ".dx/pins is not at $1"; exit 1; }
  grep -q "uses: .*rules_dx" .github/workflows/ci.yml || { echo "ci.yml has no rules_dx uses: ref"; exit 1; }
  [ -f tools/dx ] || { echo "tools/dx is missing"; exit 1; }
  expect_like_fresh "$1"
  dx upgrade --to "$1" || { echo "dx upgrade --to $1 still reports stale files"; exit 1; }
}

dx init
bazel test //...
if [ "$use_local" = 1 ]; then
  printf '\nlocal_path_override(module_name = "rules_dx", path = "%s")\n' "$repo_root" >>MODULE.bazel
fi
dx upgrade --to "$to" --apply
expect_version "$to"
bazel test //...
dx upgrade --to "$from" --apply
expect_version "$from"
bazel test //...
echo "upgrade-e2e: ok"
