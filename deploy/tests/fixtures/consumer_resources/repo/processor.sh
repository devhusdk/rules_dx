#!/bin/sh
set -eu
while [ "$#" -gt 2 ]; do shift; done
in="$1"
out="$2"
{ cat "$in"; printf 'FOOTER-dx-consumer-v1\n'; } > "$out"
