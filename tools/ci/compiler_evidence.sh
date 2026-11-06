#!/usr/bin/env bash
# Appends one JSON line of compiler evidence per compiler row of the support matrix facts.
# usage: compiler_evidence.sh OUT PLATFORM FACTS
# Run after the tests. Reads bazel-bin and bazel-testlogs, or DX_BIN and DX_TESTLOGS when set.
# With DX_PRODUCERS set to producers_bin, it also records the compiler signatures found in the row's test executables
# and marks a passing row mismatch when none of them carries the expected compiler and version,
# or when it is the msvc row and its executables carry no Rich header at all.
# The probe is the row's identity test for PLATFORM from identity.platform_probes, else identity.probe.
# A row is failed if any test failed, passed if its probe (when it has one) and at least one test passed, else skipped.
# A passed or skipped row the facts list in compiler_blockers for PLATFORM is blocked instead, keeping the
# original outcome in tested_outcome and the reason in blocker.
# Each line also carries the test command (DX_EVIDENCE_COMMAND) and the sha256 of MODULE.bazel.lock
# (or DX_LOCKFILE), which pins every downloaded compiler archive.
set -euo pipefail

out="$1"; platform="$2"; facts="$3"
bin="${DX_BIN:-bazel-bin}"
testlogs="${DX_TESTLOGS:-bazel-testlogs}"
bazel_version="${DX_BAZEL_VERSION:-$(bazel --version | tr -d '\r')}"
lockfile="${DX_LOCKFILE:-MODULE.bazel.lock}"
lockfile_sha256=""
if [ -f "$lockfile" ]; then
  if command -v sha256sum >/dev/null; then
    lockfile_sha256="$(sha256sum "$lockfile" | cut -d' ' -f1)"
  else
    lockfile_sha256="$(shasum -a 256 "$lockfile" | cut -d' ' -f1)"
  fi
fi

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
  probe="$(jq -r --arg platform "$platform" '.identity.platform_probes[$platform] // .identity.probe' <<<"$row" | tr -d '\r')"

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

  observed='[]'
  found=0
  if [ -n "${DX_PRODUCERS:-}" ]; then
    files=()
    while IFS= read -r label; do
      [ -n "$label" ] || continue
      path="${label#//}"; path="${bin}/${path/:/\/}"
      for candidate in "$path" "$path.exe"; do
        [ ! -f "$candidate" ] || files+=("$candidate")
      done
    done < <(jq -r '.ci_tests[]' <<<"$row" | tr -d '\r')
    found="${#files[@]}"
    if [ "$found" -gt 0 ]; then
      observed="$("$DX_PRODUCERS" "${files[@]}" | tr -d '\r' | jq -c '[.[][]] | unique')"
    fi
  fi
  marker=""
  case "$flag" in gcc) marker="GCC" ;; llvm) marker="clang" ;; msvc) marker="msvc " ;; esac
  matched="$(jq -r --arg marker "$marker" --arg version "$version" \
    'any(.[]; contains($marker) and contains($version))' <<<"$observed")"

  if [ "$failed" = 1 ]; then
    outcome=failed
  elif [ "$ran" -gt 0 ] && { [ -z "$probe" ] || [ "$probe_outcome" = passed ]; }; then
    outcome=passed
  else
    outcome=skipped
  fi
  if [ "$outcome" = passed ] && [ "$observed" != '[]' ] && [ "$matched" != true ]; then
    outcome=mismatch
  fi
  if [ "$flag" = msvc ] && [ "$outcome" = passed ] && [ "$found" -gt 0 ] && [ "$observed" = '[]' ]; then
    outcome=mismatch
  fi
  blocker="$(jq -r --arg row "$name" --arg platform "$platform" '.compiler_blockers[$row][$platform] // ""' "$facts" | tr -d '\r')"
  tested_outcome="$outcome"
  if [ -n "$blocker" ] && { [ "$outcome" = passed ] || [ "$outcome" = skipped ]; }; then
    outcome=blocked
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
    --arg command "${DX_EVIDENCE_COMMAND:-}" \
    --arg lockfile_sha256 "$lockfile_sha256" \
    --arg job "${GITHUB_SERVER_URL:-}/${GITHUB_REPOSITORY:-}/actions/runs/${GITHUB_RUN_ID:-}" \
    --arg probe "$probe" \
    --arg probe_outcome "$probe_outcome" \
    --argjson tests "$tests" \
    --argjson ran "$ran" \
    --argjson total "$total" \
    --arg outcome "$outcome" \
    --arg tested_outcome "$tested_outcome" \
    --arg blocker "$blocker" \
    --argjson observed "$observed" \
    '{platform: $platform, row: $row, compiler_flag: $flag, version: $version, sha: $sha, compiler: $compiler, target: $target, bazel: $bazel, command: $command, lockfile_sha256: $lockfile_sha256, job: $job, probe: $probe, probe_outcome: $probe_outcome, tests: $tests, ran: $ran, total: $total, observed_producers: $observed, outcome: $outcome, tested_outcome: $tested_outcome, blocker: $blocker}' >> "$out"
done < <(jq -c '.rows[] | select(.section == "compilers")' "$facts" | tr -d '\r')
