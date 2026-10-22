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
if [ "$code" -eq 0 ] || [ "$code" -eq 1 ]; then
  echo "parse error exited $code, expected a failure outside 0 and 1" >&2
  exit 1
fi

"$scalafmt" --config "$bad" --list "$work/Fine.scala" >"$work/cfg.out" 2>"$work/cfg.err"
code=$?
if [ "$code" -eq 0 ] || [ "$code" -eq 1 ]; then
  echo "config error exited $code, expected a failure outside 0 and 1" >&2
  exit 1
fi
