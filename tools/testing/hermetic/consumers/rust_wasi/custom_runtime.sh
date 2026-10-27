#!/bin/sh
subcommand="$1"
shift
echo "runtime=custom $subcommand $*" >&2
exec "$DX_WASI_MANAGED_RUNTIME" "$subcommand" "$@"
