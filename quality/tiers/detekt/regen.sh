#!/usr/bin/env bash
# Rebuilds standard.yml and strict.yml from the pinned detekt-cli jar.
# Usage: regen.sh <detekt.jar> [java]
set -euo pipefail

jar=${1:?usage: regen.sh <detekt.jar> [java]}
java=${2:-java}
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

(cd "$work" && "$java" -jar "$jar" --generate-config >/dev/null)

# Rule sets standard turns on.
sets="potential-bugs exceptions performance"
# Rules strict leaves off: formatting-only rules that fight ktfmt, and
# AbsentOrWrongFileLicense, which needs a license template file.
skipped="AbsentOrWrongFileLicense MaxLineLength NewLineAtEndOfFile WildcardImport TrailingWhitespace NoTabs SpacingBetweenPackageAndImports"

rewrite() {
  awk -v mode="$1" -v sets="$sets" -v skipped="$skipped" '
    BEGIN {
      n = split(sets, a, " "); for (i = 1; i <= n; i++) keep[a[i]] = 1
      n = split(skipped, b, " "); for (i = 1; i <= n; i++) skip[b[i]] = 1
    }
    /^  [A-Za-z]+:/ { rule = $1; sub(/:$/, "", rule) }
    /^[a-z-]+:/ { rule = ""; set = $1; sub(/:$/, "", set) }
    /^    active: / && rule != "" {
      if (mode == "standard") { print "    active: " ((set in keep) ? "true" : "false"); next }
      if (mode == "strict") { print "    active: " ((rule in skip) ? "false" : "true"); next }
    }
    { print }
  ' "$work/detekt.yml"
}

rewrite standard >"$here/standard.yml"
{
  echo "# Every rule active, except these: formatting-only rules that fight ktfmt,"
  echo "# and AbsentOrWrongFileLicense, which needs a license template file."
  echo "# $skipped"
  rewrite strict
} >"$here/strict.yml"
