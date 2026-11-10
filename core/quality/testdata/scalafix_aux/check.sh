#!/bin/sh
set -eu
dir=core/quality/testdata/scalafix_aux
check() {
  result="$dir/$1-real-lint-scala.pb"
  grep -aq "$dir/$2" "$result" || { echo "$1: $2 result missing"; exit 1; }
  grep -aq "ExplicitResultTypes\|Patch" "$result" || { echo "$1: ExplicitResultTypes patch missing"; exit 1; }
  grep -aq ": $3" "$result" || { echo "$1: patch does not name $3"; exit 1; }
  if grep -aq "unexpected error\|parse error\|command line error\|SemanticDB not found\|stale SemanticDB\|no files to check" "$result"; then
    echo "$1: scalafix failed"
    exit 1
  fi
}
check uses_api UsesApi.scala Api
check core_test CoreTest.scala Box

result="$dir/unused_lib-real-lint-scala.pb"
grep -aq "$dir/Unused.scala" "$result" || { echo "unused_lib: Unused.scala result missing"; exit 1; }
grep -aq "Patch" "$result" || { echo "unused_lib: patch missing"; exit 1; }

result="$dir/unused3_lib-real-lint-scala.pb"
grep -aq "$dir/Unused3.scala" "$result" || { echo "unused3_lib: Unused3.scala result missing"; exit 1; }
grep -aq "Patch" "$result" || { echo "unused3_lib: RemoveUnused patch missing on Scala 3"; exit 1; }
