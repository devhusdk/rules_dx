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
grep -aq "SF_SWITCH_FALLTHROUGH" "$dep" || { echo "multiline finding missing"; exit 1; }
LC_ALL=C grep -aqP 'SF_SWITCH_FALLTHROUGH\*\*quality/testdata/spotbugs_owned/Dirty\.java0\xbe\x02\x38\xe9\x02' "$dep" || { echo "multiline finding not placed on lines 17 to 19"; exit 1; }
