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

# Rules the no-config defaults run; standard keeps them on.
unused="UnusedImports UnusedParameter UnusedPrivateMember UnusedPrivateProperty UnreachableCode"
# Rules strict leaves off: formatting-only rules that fight ktfmt and ktlint, and
# AbsentOrWrongFileLicense, which needs a license template file.
skipped="AbsentOrWrongFileLicense MaxLineLength NewLineAtEndOfFile WildcardImport TrailingWhitespace NoTabs SpacingBetweenPackageAndImports"

rewrite() {
  awk -v mode="$1" -v unused="$unused" -v skipped="$skipped" '
    BEGIN {
      n = split(unused, a, " "); for (i = 1; i <= n; i++) force[a[i]] = 1
      n = split(skipped, b, " "); for (i = 1; i <= n; i++) skip[b[i]] = 1
    }
    /^  [A-Za-z]+:/ { rule = $1; sub(/:$/, "", rule) }
    /^[a-z-]+:/ { rule = "" }
    /^    active: / && rule != "" {
      if (mode == "standard" && (rule in force)) { print "    active: true"; next }
      if (mode == "strict") { print "    active: " ((rule in skip) ? "false" : "true"); next }
    }
    { print }
  ' "$work/detekt.yml"
}

rewrite standard >"$here/standard.yml"
{
  echo "# Every rule active, except these: formatting-only rules that fight ktfmt and ktlint,"
  echo "# and AbsentOrWrongFileLicense, which needs a license template file."
  echo "# $skipped"
  rewrite strict
} >"$here/strict.yml"
