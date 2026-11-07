#!/bin/sh
# Clang does not outline atomics on Darwin, so the macOS aarch64 builtins archive
# carries no lse_<pat>_<size>_<model>.o helpers (#9125).
set -eu
members=$(grep -aoE 'lse_[a-z]+_[0-9]+_[0-9]+(\.pic)?\.o' "$1" | sort -u | wc -l)
if [ "$members" -ne 0 ]; then
    echo "$1 has $members lse_*.o members, want 0" >&2
    exit 1
fi
