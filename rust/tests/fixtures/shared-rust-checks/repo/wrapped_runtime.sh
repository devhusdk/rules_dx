#!/bin/sh
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*"
subcommand="$1"
shift
exec "$DX_WASI_MANAGED_RUNTIME" "$subcommand" --env CONSUMER_RUNTIME=wrapped "$@"
