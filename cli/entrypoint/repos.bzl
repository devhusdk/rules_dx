"""A consumer repository outside this module that runs the public dx launcher."""

_BUILD_BAZEL = '''genrule(
    name = "consumer_dx_probe",
    outs = [
        "crate_help.txt",
        "public_completion.txt",
        "public_drift.txt",
        "public_help.txt",
        "public_status.txt",
        "public_version.txt",
        "run_ci_dryrun.txt",
        "workspace/.dx/version",
        "workspace/MODULE.bazel",
    ],
    cmd = """
set -eu
public="$(location @rules_dx//:dx)"
crate="$(location @rules_dx//cli/cli:dx)"
"$$public" --help > "$(location public_help.txt)"
"$$crate" --help > "$(location crate_help.txt)"
cmp "$(location public_help.txt)" "$(location crate_help.txt)"
echo 'module(name = "consumer", version = "0.0.0")' > "$(location workspace/MODULE.bazel)"
echo '0.0.0' > "$(location workspace/.dx/version)"
ws="$$(dirname "$(location workspace/MODULE.bazel)")"
"$$public" status --workspace "$$ws" > "$(location public_status.txt)"
"$$public" version --workspace "$$ws" > "$(location public_version.txt)"
"$$public" completion bash --workspace "$$ws" > "$(location public_completion.txt)"
CI=true "$$public" run --dry-run //app:bin --workspace "$$ws" > "$(location run_ci_dryrun.txt)" 2>&1
grep -q "Running run for //app:bin" "$(location run_ci_dryrun.txt)"
echo '9.9.9' > "$(location workspace/.dx/version)"
if "$$public" version --check --workspace "$$ws" > "$(location public_drift.txt)" 2>&1; then
  echo "dx version --check accepted a drifted pin" >&2
  exit 1
fi
grep -q status_pin_mismatch "$(location public_drift.txt)"
""",
    tools = [
        "@rules_dx//:dx",
        "@rules_dx//cli/cli:dx",
    ],
    visibility = ["//visibility:public"],
)
'''

def _consumer_dx_repo_impl(ctx):
    """Writes the consumer repository that depends on the public dx label."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)

consumer_dx_repo = repository_rule(
    implementation = _consumer_dx_repo_impl,
)

_SETS_BUILD_BAZEL = '''genrule(
    name = "consumer_sets_probe",
    outs = [
        "bad_kind.txt",
        "bump_unsupported.txt",
        "license_json.txt",
        "nolock_license.txt",
        "overlap_dryrun.txt",
        "security_json.txt",
        "update_check_dryrun.txt",
        "update_dryrun.txt",
        "update_dryrun_one.txt",
        "sets_ok/dx.toml",
        "sets_ok/MODULE.bazel",
        "sets_ok/.dx/version",
        "sets_ok/apps/frontend/pyproject.toml",
        "sets_ok/apps/frontend/uv.lock",
        "sets_ok/services/worker/pyproject.toml",
        "sets_ok/services/worker/uv.lock",
        "sets_overlap/dx.toml",
        "sets_overlap/MODULE.bazel",
        "sets_overlap/.dx/version",
        "sets_overlap/apps/left/pyproject.toml",
        "sets_overlap/apps/left/uv.lock",
        "sets_overlap/apps/right/pyproject.toml",
        "sets_overlap/apps/right/uv.lock",
        "sets_bad/dx.toml",
        "sets_bad/MODULE.bazel",
        "sets_bad/.dx/version",
        "sets_nolock/dx.toml",
        "sets_nolock/MODULE.bazel",
        "sets_nolock/.dx/version",
        "sets_nolock/apps/frontend/pyproject.toml",
    ],
    cmd = """
set -eu
public="$(location @rules_dx//:dx)"
ok="$$(dirname "$(location sets_ok/MODULE.bazel)")"
overlap="$$(dirname "$(location sets_overlap/MODULE.bazel)")"
bad="$$(dirname "$(location sets_bad/MODULE.bazel)")"
nolock="$$(dirname "$(location sets_nolock/MODULE.bazel)")"
for ws in "$$ok" "$$overlap" "$$bad" "$$nolock"; do
  echo 'module(name = "consumer_sets", version = "0.0.0")' > "$$ws/MODULE.bazel"
  mkdir -p "$$ws/.dx"
  echo '0.0.0' > "$$ws/.dx/version"
done
cat > "$(location sets_ok/dx.toml)" <<'EOF'
schema_version = 1
[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]
[[dependency_set]]
name = "worker"
ecosystem = "uv"
manifests = ["services/worker/pyproject.toml"]
locks = ["services/worker/uv.lock"]
scopes = ["services/worker"]
EOF
cat > "$(location sets_overlap/dx.toml)" <<'EOF'
schema_version = 1
[[dependency_set]]
name = "left"
ecosystem = "uv"
manifests = ["apps/left/pyproject.toml"]
locks = ["apps/left/uv.lock"]
scopes = ["apps/shared"]
[[dependency_set]]
name = "right"
ecosystem = "uv"
manifests = ["apps/right/pyproject.toml"]
locks = ["apps/right/uv.lock"]
scopes = ["apps/shared"]
EOF
cat > "$(location sets_bad/dx.toml)" <<'EOF'
schema_version = 1
[[dependency_set]]
name = "frontend"
ecosystem = "pip"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]
EOF
cat > "$(location sets_nolock/dx.toml)" <<'EOF'
schema_version = 1
[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]
EOF
for project in "$(location sets_ok/apps/frontend/pyproject.toml)" "$(location sets_ok/services/worker/pyproject.toml)" "$(location sets_overlap/apps/left/pyproject.toml)" "$(location sets_overlap/apps/right/pyproject.toml)" "$(location sets_nolock/apps/frontend/pyproject.toml)"; do
  cat > "$$project" <<'EOF'
[project]
name = "project"
version = "0.1.0"
requires-python = ">=3.9"
dependencies = ["anyio>=4"]
EOF
done
for lock in "$(location sets_ok/apps/frontend/uv.lock)" "$(location sets_ok/services/worker/uv.lock)" "$(location sets_overlap/apps/left/uv.lock)" "$(location sets_overlap/apps/right/uv.lock)"; do
  cat > "$$lock" <<'EOF'
version = 1
requires-python = ">=3.9"
[[package]]
name = "anyio"
version = "4.0.0"
source = { registry = "https://pypi.org/simple" }
EOF
done
cp "$(location sets_ok/apps/frontend/uv.lock)" /tmp/consumer_sets_frontend_uv_lock.orig
cp "$(location sets_ok/services/worker/uv.lock)" /tmp/consumer_sets_worker_uv_lock.orig
"$$public" update --dry-run --verbose --workspace "$$ok" > "$(location update_dryrun.txt)"
grep -q "Running update for frontend, worker" "$(location update_dryrun.txt)"
grep -q "Would update frontend: uv lock --directory apps/frontend" "$(location update_dryrun.txt)"
grep -q "Would update worker: uv lock --directory services/worker" "$(location update_dryrun.txt)"
"$$public" update --dry-run --verbose --workspace "$$ok" worker > "$(location update_dryrun_one.txt)"
grep -q "Running update for worker" "$(location update_dryrun_one.txt)"
if grep -q frontend "$(location update_dryrun_one.txt)"; then
  echo "selective dry-run leaked the unselected set" >&2
  exit 1
fi
"$$public" update --check --dry-run --verbose --workspace "$$ok" > "$(location update_check_dryrun.txt)"
grep -q "Would check frontend: uv lock --check --directory apps/frontend" "$(location update_check_dryrun.txt)"
if "$$public" security --output=json --workspace "$$ok" > "$(location security_json.txt)" 2>/dev/null; then
  echo "note: security passed with the hermetic gitleaks tool present" >&2
fi
grep -q "no advisory coverage for frontend, worker" "$(location security_json.txt)"
"$$public" license --output=json --workspace "$$ok" > "$(location license_json.txt)"
grep -q audit_license_clean "$(location license_json.txt)"
if "$$public" bump frontend 4.1.0 --workspace "$$ok" > "$(location bump_unsupported.txt)" 2>&1; then
  echo "bump unexpectedly widened a configured uv set" >&2
  exit 1
fi
grep -q "bump is not supported for ecosystem uv set frontend" "$(location bump_unsupported.txt)"
"$$public" update --dry-run --verbose --workspace "$$overlap" apps/shared/thing > "$(location overlap_dryrun.txt)"
grep -q "Running update for left, right" "$(location overlap_dryrun.txt)"
if "$$public" update --dry-run --workspace "$$bad" > "$(location bad_kind.txt)" 2>&1; then
  echo "update accepted an unknown ecosystem" >&2
  exit 1
fi
grep -q "unknown ecosystem" "$(location bad_kind.txt)"
if "$$public" license --output=json --workspace "$$nolock" > "$(location nolock_license.txt)" 2>&1; then
  echo "license passed with a missing lockfile" >&2
  exit 1
fi
grep -q "failed to assess frontend" "$(location nolock_license.txt)"
grep -q "apps/frontend/uv.lock" "$(location nolock_license.txt)"
cmp /tmp/consumer_sets_frontend_uv_lock.orig "$(location sets_ok/apps/frontend/uv.lock)"
cmp /tmp/consumer_sets_worker_uv_lock.orig "$(location sets_ok/services/worker/uv.lock)"
""",
    tools = [
        "@rules_dx//:dx",
    ],
    visibility = ["//visibility:public"],
)
'''

def _consumer_sets_repo_impl(ctx):
    """Writes the consumer repository that declares its own dependency sets."""
    ctx.file("BUILD.bazel", _SETS_BUILD_BAZEL)

consumer_sets_repo = repository_rule(
    implementation = _consumer_sets_repo_impl,
)
