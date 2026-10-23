#!/bin/sh
set -u
dart="$1"
src=examples/dart-app/diagnostics
failures=0

fail() {
  echo "FAIL: $*"
  failures=$((failures + 1))
}

run() {
  "$dart" --suppress-analytics "$@" >"$TEST_TMPDIR/out" 2>&1
  code=$?
}

expect() {
  want="$1"
  what="$2"
  [ "$code" = "$want" ] || fail "$what exited $code, want $want: $(cat "$TEST_TMPDIR/out")"
}

expect_text() {
  grep -qF -- "$1" "$TEST_TMPDIR/out" || fail "$2 did not print '$1': $(cat "$TEST_TMPDIR/out")"
}

compile() {
  run compile kernel -o "$TEST_TMPDIR/$(basename "$1").dill" "$src/$1"
}

analyze() {
  run analyze --format=machine --fatal-infos "$src/$1"
}

format() {
  run format --output=none --set-exit-if-changed "$src/$1"
}

compile clean.dart
expect 0 "compile clean.dart"
format clean.dart
expect 0 "format clean.dart"
analyze clean.dart
expect 0 "analyze clean.dart"

compile syntax_error.dart
[ "$code" != 0 ] || fail "compile syntax_error.dart passed"
expect_text "Can't find ')' to match '('" "compile syntax_error.dart"
format syntax_error.dart
expect 65 "format syntax_error.dart"
analyze syntax_error.dart
expect 3 "analyze syntax_error.dart"
expect_text "ERROR|SYNTACTIC_ERROR|EXPECTED_TOKEN|" "analyze syntax_error.dart"

compile type_error.dart
[ "$code" != 0 ] || fail "compile type_error.dart passed"
expect_text "can't be returned from a function" "compile type_error.dart"
analyze type_error.dart
expect 3 "analyze type_error.dart"
expect_text "|RETURN_OF_INVALID_TYPE|" "analyze type_error.dart"

compile missing_dependency.dart
[ "$code" != 0 ] || fail "compile missing_dependency.dart passed"
expect_text "package:args/args.dart" "compile missing_dependency.dart"
analyze missing_dependency.dart
expect 3 "analyze missing_dependency.dart"
expect_text "|URI_DOES_NOT_EXIST|" "analyze missing_dependency.dart"

analyze lint_finding.dart
expect 1 "analyze lint_finding.dart"
expect_text "|LINT|AVOID_PRINT|" "analyze lint_finding.dart"
expect_text "|LINT|PREFER_SINGLE_QUOTES|" "analyze lint_finding.dart"

before=$(cksum <"$src/unformatted.dart")
format unformatted.dart
expect 1 "format unformatted.dart"
expect_text "Changed " "format unformatted.dart"
[ "$(cksum <"$src/unformatted.dart")" = "$before" ] || fail "format rewrote unformatted.dart"

exit "$failures"
