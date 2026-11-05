#!/usr/bin/env bash
# Upgrades a throwaway consumer from the second-latest BCR release of rules_dx
# to the latest and back, running tests after each step.
# Usage: upgrade_e2e.sh [<previous> <latest>]
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
echo "upgrade-e2e: $from -> $to -> $from"

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"

cp "$repo_root/.bazelversion" .bazelversion
cat >MODULE.bazel <<MODULE
module(name = "upgrade_e2e")

bazel_dep(name = "rules_dx", version = "$from")
MODULE

dx() { bazel run @rules_dx//:dx -- "$@"; }

expect_version() {
  grep -q "version = \"$1\"" MODULE.bazel || { echo "MODULE.bazel is not at $1"; exit 1; }
  grep -q "$1" .dx/pins || { echo ".dx/pins is not at $1"; exit 1; }
  grep -q "uses: .*rules_dx" .github/workflows/ci.yml || { echo "ci.yml has no rules_dx uses: ref"; exit 1; }
  [ -f tools/dx ] || { echo "tools/dx is missing"; exit 1; }
  dx upgrade --to "$1" || { echo "dx upgrade --to $1 still reports stale files"; exit 1; }
}

dx init
bazel test //...
dx upgrade --to "$to" --apply
expect_version "$to"
bazel test //...
dx upgrade --to "$from" --apply
expect_version "$from"
bazel test //...
echo "upgrade-e2e: ok"
