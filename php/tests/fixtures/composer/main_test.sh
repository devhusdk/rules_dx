#!/usr/bin/env bash
set -euo pipefail
got="$("$1")"
want="RequestFactoryInterface=loaded deprecation=files"
[[ "$got" == "$want" ]] || { echo "got '$got', want '$want'" >&2; exit 1; }
