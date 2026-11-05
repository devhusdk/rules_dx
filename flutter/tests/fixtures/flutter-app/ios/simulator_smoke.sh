#!/bin/sh
set -eu
while [ "$#" -gt 0 ]; do
  DX_SIMCTL_OP="$1" "$2"
  shift 2
done
