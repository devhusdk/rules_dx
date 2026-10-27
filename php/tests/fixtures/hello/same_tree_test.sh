#!/usr/bin/env bash
set -euo pipefail
diff -r "$1" "$2"
