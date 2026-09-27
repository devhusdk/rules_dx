#!/usr/bin/env bash
set -euo pipefail

source "${TEST_SRCDIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "${RUNFILES_DIR:-/dev/null}/${DX_BOOTSTRAP:-_main/tools/sh/bootstrap.sh}" 2>/dev/null || source "$(git rev-parse --show-toplevel 2>/dev/null)/tools/sh/bootstrap.sh"
dx_bootstrap "tools/sh/lib.sh"
dx_bootstrap "tools/sh/assert.sh"

dx_test_init
dx_mkscratch scratch

cat >"$scratch/result.json" <<'EOF'
{"convergence": "STABLE", "diagnostics": [{"severity": "ERROR", "tool_id": "ruff"}], "targets": ["a", "b"]}
EOF

dx_assert_json_valid "$scratch/result.json"
dx_assert_json_field "$scratch/result.json" "convergence" "STABLE"
dx_assert_json_field "$scratch/result.json" "diagnostics.0.severity" "ERROR"
dx_assert_exit "0" "0" "probe exit"
dx_assert_exit "1" "1" "failing probe exit"

cat >"$scratch/BUILD.bazel" <<'EOF'
rustfmt_config(name = "taplo_config")
rustfmt_config(name = "rustfmt_config")
EOF

dx_assert_targets_contains "$scratch/BUILD.bazel" 'name = "taplo_config"'
dx_assert_targets_absent "$scratch/BUILD.bazel" 'name = "missing_config"'
dx_assert_target_set "$scratch/BUILD.bazel" "rustfmt_config" "taplo_config"

dx_test_summary "assert helper"
