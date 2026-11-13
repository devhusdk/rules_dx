#!/usr/bin/env bash
set -euo pipefail
set +e; f=bazel_tools/tools/bash/runfiles/runfiles.bash
# shellcheck disable=SC1090
source "${RUNFILES_DIR:-/dev/null}/$f" 2>/dev/null || \
  source "$(grep -sm1 "^$f " "${RUNFILES_MANIFEST_FILE:-/dev/null}" | cut -f2- -d' ')" 2>/dev/null || \
  source "$0.runfiles/$f" 2>/dev/null || \
  { echo >&2 "ERROR: cannot find $f"; exit 1; }; f=; set -e
runfiles_export_envvars
runner="$(rlocation _main/dotnet/private/quality/tools/fsharplint/runner.sh 2>/dev/null || true)"
if [[ -z "$runner" || ! -e "$runner" ]]; then
  runner="$(rlocation rules_dx+/dotnet/private/quality/tools/fsharplint/runner.sh)"
fi
FSHARPLINT_CWD="$PWD"
export FSHARPLINT_CWD
cd "${runner%/dotnet/private/quality/tools/fsharplint/runner.sh}"
exec "$runner" "$@"
