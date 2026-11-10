#!/bin/sh
set -eu
app=core/quality/testdata/spotbugs_owned/app-real-lint-jvm.pb
dep=core/quality/testdata/spotbugs_owned/dep-real-lint-jvm.pb
grep -aq "NP_ALWAYS_NULL" "$dep" || { echo "dep finding missing"; exit 1; }
if grep -aq "NP_ALWAYS_NULL\|Dirty.java" "$app"; then
  echo "app result contains the dependency finding"
  exit 1
fi
grep -aq "Clean.java" "$app" || { echo "app result missing"; exit 1; }
grep -aq "SF_DEAD_STORE_DUE_TO_SWITCH_FALLTHROUGH" "$dep" || { echo "multiline finding missing"; exit 1; }
placed=$(printf 'SF_DEAD_STORE_DUE_TO_SWITCH_FALLTHROUGH*/core/quality/testdata/spotbugs_owned/Dirty.java0\333\0028\333\002')
LC_ALL=C grep -aqF "$placed" "$dep" || { echo "multiline finding not placed"; exit 1; }
test_result=core/quality/testdata/spotbugs_owned/app_test-real-lint-jvm.pb
grep -aq "CleanTest.java" "$test_result" || { echo "test result missing"; exit 1; }
implicit=core/quality/testdata/spotbugs_owned/implicit_test-real-lint-jvm.pb
grep -aq "ImplicitTest.java" "$implicit" || { echo "implicit junit test result missing"; exit 1; }
uses_api=core/quality/testdata/spotbugs_owned/uses_api-real-lint-jvm.pb
grep -aq "UsesApi.java" "$uses_api" || { echo "compile-only dep result missing"; exit 1; }
collide=core/quality/testdata/spotbugs_owned/collide-real-lint-jvm.pb
grep -aq "Collide.java" "$collide" || { echo "same-named compile-only dep result missing"; exit 1; }
