#!/usr/bin/env bash
set -euo pipefail
declare -F dx_bootstrap >/dev/null 2>&1 || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"

dx_test_init

if declare -F dx_bootstrap >/dev/null 2>&1; then
  ok "bootstrap available"
else
  bad "dx_bootstrap unavailable"
fi

if [[ -n "${DX_HARNESS_SRC:-}" ]]; then
  ok "harness payload advertised"
elif [[ -z "${DX_BOOTSTRAP:-}" ]]; then
  ok "direct source-tree run without harness"
else
  bad "DX_HARNESS_SRC unset"
fi

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

entry="$(dx_resolve_runfile "tools/sh/entry.sh")"
dx_mkscratch fake
mkdir -p "$fake/tree/tools/sh" "$fake/bin"
cp "$(dx_resolve_runfile "tools/sh/bootstrap.sh")" "$fake/tree/tools/sh/bootstrap.sh"
cp "$(dx_resolve_runfile "tools/sh/lib.sh")" "$fake/tree/tools/sh/lib.sh"
cat >"$fake/tree/probe.sh" <<'EOF'
#!/usr/bin/env bash
declare -F dx_resolve_runfile >/dev/null 2>&1 || exit 3
echo "probe sourced with args: $*"
EOF
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "%s"\n' "$fake/tree" >"$fake/bin/git"
chmod +x "$fake/bin/git"

if env -i PATH="$fake/bin:/usr/bin:/bin" DX_HARNESS_SRC="probe.sh" bash "$entry" hello world >"$fake/entry.out" 2>"$fake/entry.err"; then
  if dx_grep_contains "$fake/entry.out" "probe sourced with args: hello world" >/dev/null 2>&1; then
    ok "source-tree fallback loads helper"
  else
    bad "source-tree fallback sourced wrong payload"
  fi
else
  bad "source-tree fallback failed"
  cat "$fake/entry.err" >&2 || true
fi

dx_test_summary "bootstrap helper"
