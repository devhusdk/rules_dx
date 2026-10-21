#!/bin/sh
set -eu
app=quality/testdata/spotbugs_owned/app-real-lint-jvm.pb
dep=quality/testdata/spotbugs_owned/dep-real-lint-jvm.pb
grep -aq "NP_LOAD_OF_KNOWN_NULL_VALUE" "$dep" || { echo "dep finding missing"; exit 1; }
if grep -aq "NP_LOAD_OF_KNOWN_NULL_VALUE\|Dirty.java" "$app"; then
  echo "app result contains the dependency finding"
  exit 1
fi
grep -aq "Clean.java" "$app" || { echo "app result missing"; exit 1; }
