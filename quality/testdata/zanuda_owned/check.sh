#!/bin/sh
set -eu
dir=quality/testdata/zanuda_owned
dirty="$dir/dirty-real-lint-ocaml.pb"
clean="$dir/clean-real-lint-ocaml.pb"
grep -aq "$dir/dirty.ml" "$dirty" || { echo "dirty: dirty.ml result missing"; exit 1; }
grep -aq "if_bool" "$dirty" || { echo "dirty: typed if_bool finding missing"; exit 1; }
grep -aq "list_length_comparisons" "$dirty" || { echo "dirty: list_length_comparisons finding missing"; exit 1; }
grep -aq "$dir/clean.ml" "$clean" || { echo "clean: clean.ml result missing"; exit 1; }
if grep -aq "if_bool\|list_length_comparisons\|Fatal error\|no such file" "$clean"; then
  echo "clean: unexpected finding or failure"
  exit 1
fi
