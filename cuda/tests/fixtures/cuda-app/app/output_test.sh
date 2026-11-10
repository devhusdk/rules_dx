#!/bin/sh
set -u
bin="$1"
want="cuda-app n=4096 add=25159680 stencil=75466755"
out=$("$bin")
code=$?
if [ "$code" != 0 ] || [ "$out" != "$want" ]; then
  echo "FAIL: $bin -> $code '$out' (want 0 '$want')"
  exit 1
fi
