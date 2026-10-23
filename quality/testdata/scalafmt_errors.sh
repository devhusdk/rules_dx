#!/bin/bash
set -u
scalafmt="$1"
good="$2"
bad="$3"
work="${TEST_TMPDIR:-/tmp}/scalafmt_errors"
rm -rf "$work"
mkdir -p "$work"
printf 'package fixtures.scalafmt\n\nobject Sample {\n  def greet = (\n}\n' >"$work/Broken.scala"
printf 'package fixtures.scalafmt\n\nobject Sample {\n  def greet = 1\n}\n' >"$work/Fine.scala"

"$scalafmt" --config "$good" --list "$work/Broken.scala" >"$work/parse.out" 2>"$work/parse.err"
code=$?
if [ "$code" -ne 2 ] || ! grep -q "Broken.scala:5: error" "$work/parse.err"; then
  echo "parse error exited $code, expected 2 naming Broken.scala" >&2
  cat "$work/parse.err" >&2
  exit 1
fi

"$scalafmt" --config "$bad" --list "$work/Fine.scala" >"$work/cfg.out" 2>"$work/cfg.err"
code=$?
if [ "$code" -ne 8 ] || ! grep -q "noSuchOption" "$work/cfg.err"; then
  echo "config error exited $code, expected 8 naming noSuchOption" >&2
  cat "$work/cfg.err" >&2
  exit 1
fi
