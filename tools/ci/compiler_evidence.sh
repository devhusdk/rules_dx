#!/usr/bin/env bash
# Appends one JSON line of compiler evidence for a compiler row to a file.
# usage: compiler_evidence.sh OUT PLATFORM COMPILER IDENTITY_TEST ROW_TEST...
# Run from the workspace root after the tests, with bazel-bin and bazel-testlogs in place.
set -euo pipefail

out="$1"; platform="$2"; compiler="$3"; identity="$4"
shift 4

outcome_of() {
  local path="${1#//}"
  local xml="bazel-testlogs/${path/:/\/}/test.xml"
  if [ ! -f "$xml" ]; then
    echo skipped
  elif grep -Eq '<(failure|error)' "$xml"; then
    echo failed
  else
    echo passed
  fi
}

outcome="$(outcome_of "$identity")"
for label in "$@"; do
  case "$(outcome_of "$label")" in
    failed) outcome=failed ;;
    skipped) [ "$outcome" = passed ] && outcome=skipped ;;
  esac
done

path="${identity#//}"
manifest="bazel-bin/${path/:/\/}_starlark_manifest.json"
probe=""
if [ -f "$manifest" ]; then
  probe="$(jq -r '.. | strings | select(contains("field cc.compiler="))' "$manifest")"
fi
probe_field() {
  printf '%s\n' "$probe" | sed -n "s/^field cc\.$1=//p" | head -n 1
}

jq -cn \
  --arg platform "$platform" \
  --arg flag "$compiler" \
  --arg sha "${GITHUB_SHA:-}" \
  --arg compiler "$(probe_field compiler)" \
  --arg target "$(probe_field target)" \
  --arg bazel "$(bazel --version | tr -d '\r')" \
  --arg job "${GITHUB_SERVER_URL:-}/${GITHUB_REPOSITORY:-}/actions/runs/${GITHUB_RUN_ID:-}" \
  --arg outcome "$outcome" \
  '{platform: $platform, compiler_flag: $flag, sha: $sha, compiler: $compiler, target: $target, bazel: $bazel, job: $job, outcome: $outcome}' >> "$out"
