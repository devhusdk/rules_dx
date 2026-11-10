#!/bin/sh
set -eu
while [ "$#" -gt 2 ]; do shift; done
in="$1"
out="$2"
{ printf 'DX-RESOURCES-DEMO-V1\n'; cat "$in"; } > "$out"
