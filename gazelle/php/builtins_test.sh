#!/usr/bin/env bash
set -euo pipefail
dump="$1"
list="$(realpath "$2")"
exec "$dump" --check "$list"
