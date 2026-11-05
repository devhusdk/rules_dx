#!/usr/bin/env bash
set -euo pipefail

list="tools/testing/nested_smoke.txt"
query='attr(tags, "resources:nested_bazel", tests(//...)) except attr(name, ".*_upstream$", tests(//...))'
cap=8

family_query() {
  local expr="$1"
  shift
  local module dir out
  for module in families/*/MODULE.bazel; do
    [ -f "$module" ] || continue
    dir="$(dirname "$module")"
    if ! out="$(cd "$dir" && bazel query --noshow_progress --output=label "$@" "$expr")"; then
      echo "the query found no nested Bazel tests in $dir" >&2
      exit 2
    fi
    printf '%s\n' "$out" | grep . | sed "s|^//|$dir//|" || true
  done
}

listed="$(grep -v '^#' "$list" | grep -v '^$' | LC_ALL=C sort -u)"
queried="$(bazel query --noshow_progress --output=label "$@" "$query" | LC_ALL=C sort -u)"
if [ -z "$queried" ]; then
  echo "the query found no nested Bazel tests; the guard query is broken" >&2
  exit 2
fi
queried="$(printf '%s\n%s\n' "$queried" "$(family_query "$query" "$@")" | grep . | LC_ALL=C sort -u)"

status=0
count="$(printf '%s\n' "$listed" | grep -c . || true)"
if [ "$count" -gt "$cap" ]; then
  echo "$list lists $count tests; the cap is $cap" >&2
  status=1
fi
added="$(LC_ALL=C comm -23 <(printf '%s\n' "$queried") <(printf '%s\n' "$listed"))"
if [ -n "$added" ]; then
  echo "nested Bazel tests missing from $list (test the logic directly instead):" >&2
  printf '%s\n' "$added" | sed 's/^/  /' >&2
  status=1
fi
enormous="$(bazel query --noshow_progress --output=label "$@" 'attr(size, enormous, tests(//...))' | LC_ALL=C sort -u)"
enormous="$(printf '%s\n%s\n' "$enormous" "$(family_query 'attr(size, enormous, tests(//...))' "$@")" | grep . || true)"
if [ -n "$enormous" ]; then
  echo "enormous tests: $(printf '%s\n' "$enormous" | paste -sd' ' -)" >&2
  status=1
fi
stale="$(LC_ALL=C comm -13 <(printf '%s\n' "$queried") <(printf '%s\n' "$listed"))"
if [ -n "$stale" ]; then
  echo "entries in $list with no nested Bazel test (delete the line):" >&2
  printf '%s\n' "$stale" | sed 's/^/  /' >&2
  status=1
fi
[ "$status" -eq 0 ] && echo "$list matches all $(printf '%s\n' "$queried" | wc -l) nested Bazel tests"
exit "$status"
