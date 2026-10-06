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
