#!/usr/bin/env bash
set -euo pipefail
source "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init

if [[ -n "${DX_BOOTSTRAP:-}" ]]; then
  if [[ -f "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP}" || -f "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP}" ]]; then
    ok "runfiles preload resolves"
  else
    bad "DX_BOOTSTRAP points at missing file: $DX_BOOTSTRAP"
  fi
  if declare -F rlocation >/dev/null 2>&1; then
    ok "runfiles.bash loaded"
  else
    bad "rlocation unavailable under Bazel"
  fi
else
  ok "source-tree fallback reached"
fi

if dx_bootstrap "tools/sh/no_such_file.sh" 2>/dev/null; then
  bad "dx_bootstrap succeeded for missing file"
else
  ok "dx_bootstrap rejects missing file"
fi

dx_mkscratch fake
mkdir -p "$fake/tree/tools/sh" "$fake/bin"
cp "$(dx_resolve_runfile "tools/sh/bootstrap.sh")" "$fake/tree/tools/sh/bootstrap.sh"
cp "$(dx_resolve_runfile "tools/sh/lib.sh")" "$fake/tree/tools/sh/lib.sh"
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "%s"\n' "$fake/tree" >"$fake/bin/git"
chmod +x "$fake/bin/git"

if env -i PATH="$fake/bin:/usr/bin:/bin" bash -c '
set -euo pipefail
source "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
declare -F dx_bootstrap >/dev/null
dx_bootstrap "tools/sh/lib.sh"
declare -F dx_test_init >/dev/null
' >/dev/null 2>&1; then
  ok "source-tree fallback loads helper"
else
  bad "source-tree fallback failed"
fi

dx_test_summary "bootstrap helper"
