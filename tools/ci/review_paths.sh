#!/usr/bin/env bash
# Prints the changed files on stdin that start with a prefix in .github/review-paths.txt.
# Usage: git diff --name-only origin/main...HEAD | tools/ci/review_paths.sh [prefix-file]
set -euo pipefail

list="${1:-$(dirname "$0")/../../.github/review-paths.txt}"
prefixes=$(grep -v '^[[:space:]]*\(#\|$\)' "$list")

while IFS= read -r file; do
  while IFS= read -r prefix; do
    case "$file" in
      "$prefix"*)
        printf '%s\n' "$file"
        break
        ;;
    esac
  done <<<"$prefixes"
done
