#!/usr/bin/env bash
set -euo pipefail

api="tools/docs/site/user/api.bzl"
query='attr(visibility, "//visibility:public", kind("^(rust_library|_rust_forward) rule$", //...)) except filter("(^|/)(fixtures?|tests?|examples?)(/|:)", //...)'

listed="$(sed -n '/^PUBLIC_RUST_CRATES = \[/,/^\]/p' "$api" | grep -o '"//[^"]*"' | tr -d '"' | sort -u)"
if [ -z "$listed" ]; then
  echo "no PUBLIC_RUST_CRATES entries found in $api" >&2
  exit 2
fi

queried="$(bazel query --noshow_progress --output=label "$@" "$query" | sort -u)"
if [ -z "$queried" ]; then
  echo "the query found no public rust libraries; the guard query is broken" >&2
  exit 2
fi

missing="$(comm -23 <(printf '%s\n' "$queried") <(printf '%s\n' "$listed"))"
if [ -n "$missing" ]; then
  echo "public rust_library targets missing from PUBLIC_RUST_CRATES in $api:" >&2
  printf '%s\n' "$missing" | sed 's/^/  /' >&2
  exit 1
fi
echo "PUBLIC_RUST_CRATES covers all $(printf '%s\n' "$queried" | wc -l) public rust libraries"
