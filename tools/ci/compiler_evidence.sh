#!/usr/bin/env bash
# Appends one JSON line of compiler evidence per compiler row of the support matrix facts.
# usage: compiler_evidence.sh OUT PLATFORM FACTS
# Run after the tests. Reads bazel-bin and bazel-testlogs, or DX_BIN and DX_TESTLOGS when set.
# A row is failed if any test failed, passed if its probe (when it has one) and at least one test passed, else skipped.
set -euo pipefail

out="$1"; platform="$2"; facts="$3"
bin="${DX_BIN:-bazel-bin}"
testlogs="${DX_TESTLOGS:-bazel-testlogs}"
bazel_version="${DX_BAZEL_VERSION:-$(bazel --version | tr -d '\r')}"

outcome_of() {
  local path="${1#//}"
  local xml="${testlogs}/${path/:/\/}/test.xml"
  if [ ! -f "$xml" ]; then
    echo skipped
  elif grep -Eq '<(failure|error)' "$xml"; then
    echo failed
  else
    echo passed
  fi
}

while IFS= read -r row; do
  name="$(jq -r .name <<<"$row" | tr -d '\r')"
  flag="$(jq -r .identity.compiler <<<"$row" | tr -d '\r')"
  version="$(jq -r .identity.version <<<"$row" | tr -d '\r')"
  probe="$(jq -r .identity.probe <<<"$row" | tr -d '\r')"

  tests='{}'
  ran=0; total=0; failed=0
  while IFS= read -r label; do
    [ -n "$label" ] || continue
    outcome="$(outcome_of "$label")"
    tests="$(jq -c --arg label "$label" --arg outcome "$outcome" '. + {($label): $outcome}' <<<"$tests")"
    total=$((total + 1))
    [ "$outcome" = skipped ] || ran=$((ran + 1))
    [ "$outcome" != failed ] || failed=1
  done < <(jq -r '(.ci_tests + .consumer_tests)[]' <<<"$row" | tr -d '\r')

  probe_outcome=""
  probe_fields=""
  if [ -n "$probe" ]; then
    probe_outcome="$(outcome_of "$probe")"
    [ "$probe_outcome" != failed ] || failed=1
    path="${probe#//}"
    manifest="${bin}/${path/:/\/}_starlark_manifest.json"
    if [ -f "$manifest" ]; then
      probe_fields="$(jq -r '.. | strings | select(contains("field cc.compiler="))' "$manifest")"
    fi
  fi
  probe_field() {
    printf '%s\n' "$probe_fields" | sed -n "s/^field cc\.$1=//p" | head -n 1
  }

  if [ "$failed" = 1 ]; then
    outcome=failed
  elif [ "$ran" -gt 0 ] && { [ -z "$probe" ] || [ "$probe_outcome" = passed ]; }; then
    outcome=passed
  else
    outcome=skipped
  fi

  jq -cn \
    --arg platform "$platform" \
    --arg row "$name" \
    --arg flag "$flag" \
    --arg version "$version" \
    --arg sha "${GITHUB_SHA:-}" \
    --arg compiler "$(probe_field compiler)" \
    --arg target "$(probe_field target)" \
    --arg bazel "$bazel_version" \
    --arg job "${GITHUB_SERVER_URL:-}/${GITHUB_REPOSITORY:-}/actions/runs/${GITHUB_RUN_ID:-}" \
    --arg probe "$probe" \
    --arg probe_outcome "$probe_outcome" \
    --argjson tests "$tests" \
    --argjson ran "$ran" \
    --argjson total "$total" \
    --arg outcome "$outcome" \
    '{platform: $platform, row: $row, compiler_flag: $flag, version: $version, sha: $sha, compiler: $compiler, target: $target, bazel: $bazel, job: $job, probe: $probe, probe_outcome: $probe_outcome, tests: $tests, ran: $ran, total: $total, outcome: $outcome}' >> "$out"
done < <(jq -c '.rows[] | select(.section == "compilers")' "$facts" | tr -d '\r')
