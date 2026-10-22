#!/bin/sh
set -eu
spotbugs=quality/tools/jvm/spotbugs
app=$(ls quality/testdata/spotbugs_owned/*app*.jar | grep -v -e '-hjar' -e header | head -n1)
dep=$(ls quality/testdata/spotbugs_owned/*dep*.jar | grep -v -e '-hjar' -e header | head -n1)
out=$(mktemp -d)
"$spotbugs" -textui -effort:default -sarif "$app" >"$out/missing.sarif"
"$spotbugs" -textui -effort:default -sarif -auxclasspath "$dep" "$app" >"$out/complete.sarif"
tr -d ' \n' <"$out/missing.sarif" | grep -q '"executionSuccessful":false' || { echo "missing dependency run not marked failed"; exit 1; }
tr -d ' \n' <"$out/missing.sarif" | grep -q 'Classesneededforanalysiswere' || { echo "missing class notification absent"; exit 1; }
tr -d ' \n' <"$out/complete.sarif" | grep -q '"executionSuccessful":true' || { echo "complete classpath run not marked successful"; exit 1; }
if tr -d ' \n' <"$out/complete.sarif" | grep -q 'Classesneededforanalysiswere'; then
  echo "complete classpath run reports missing classes"
  exit 1
fi
