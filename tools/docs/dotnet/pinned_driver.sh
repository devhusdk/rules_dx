#!/usr/bin/env bash
# Starts a pinned .NET driver from the root of its runfiles, where the dotnet host finds the dependencies.
# The driver changes back to DX_DRIVER_CWD once it is running.
set -euo pipefail
set +e; f=bazel_tools/tools/bash/runfiles/runfiles.bash
# shellcheck disable=SC1090
source "${RUNFILES_DIR:-/dev/null}/$f" 2>/dev/null || \
  source "$(grep -sm1 "^$f " "${RUNFILES_MANIFEST_FILE:-/dev/null}" | cut -f2- -d' ')" 2>/dev/null || \
  source "$0.runfiles/$f" 2>/dev/null || \
  { echo >&2 "ERROR: cannot find $f"; exit 1; }; f=; set -e
runfiles_export_envvars
name="$(basename "$0")"
name="${name%.exe}"
language="${name%_api_pinned}"
DX_DOTNET_LAUNCHER="tools/docs/dotnet/$language/${language}_api_upstream/net10.0/${language}_api_upstream.dll.sh"
launcher="$(rlocation "_main/$DX_DOTNET_LAUNCHER" 2>/dev/null || true)"
root="_main"
if [[ -z "$launcher" || ! -e "$launcher" ]]; then
  launcher="$(rlocation "rules_dx+/$DX_DOTNET_LAUNCHER")"
  root="rules_dx+"
fi
DX_DRIVER_CWD="$PWD"
export DX_DRIVER_CWD
cd "${launcher%/"$DX_DOTNET_LAUNCHER"}/.."
exec "$launcher" "$@"
