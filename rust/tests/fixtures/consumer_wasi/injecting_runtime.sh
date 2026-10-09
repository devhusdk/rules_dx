#!/bin/sh
subcommand="$1"
shift
exec "$DX_WASI_MANAGED_RUNTIME" "$subcommand" --env DX_WASI_WRAPPED=wrapped-value "$@"
