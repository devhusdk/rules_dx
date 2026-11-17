#!/usr/bin/env bash
set -euo pipefail
set +e; f=bazel_tools/tools/bash/runfiles/runfiles.bash
# shellcheck disable=SC1090
source "${RUNFILES_DIR:-/dev/null}/$f" 2>/dev/null || \
  source "$(grep -sm1 "^$f " "${RUNFILES_MANIFEST_FILE:-/dev/null}" | cut -f2- -d' ')" 2>/dev/null || \
  source "$0.runfiles/$f" 2>/dev/null || \
  { echo >&2 "ERROR: cannot find $f"; exit 1; }; f=; set -e
runfiles_export_envvars
launcher=""
for launcher_rel in dotnet/private/quality/tools/fsharplint/runner/net10.0/runner.dll.{sh,bat}; do
  for repo in _main rules_dx+; do
    launcher="$(rlocation "$repo/$launcher_rel" 2>/dev/null || true)"
    [[ -n "$launcher" && -e "$launcher" ]] && break 2
  done
done
if [[ -z "$launcher" || ! -e "$launcher" ]]; then
  echo >&2 "fsharplint: cannot find the runner launcher in runfiles"
  exit 2
fi
FSHARPLINT_CWD="$PWD"
export FSHARPLINT_CWD
cd "${launcher%/"$launcher_rel"}/.."
exec "$launcher" "$@"
