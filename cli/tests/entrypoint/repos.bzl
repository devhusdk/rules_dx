"""A consumer repository outside this module that runs the public dx launcher."""

load("//cli/prebuilt:pins.bzl", "DX_VERSION")

_BUILD_BAZEL = '''genrule(
    name = "consumer_dx_probe",
    outs = [
        "crate_help.txt",
        "public_completion.txt",
        "public_help.txt",
        "public_status.txt",
        "public_version.txt",
        "workspace/.dx/pins",
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
echo 'bazel_dep(name = "rules_dx", version = "@@DX_VERSION@@")' >> "$(location workspace/MODULE.bazel)"
echo '@@DX_VERSION@@' > "$(location workspace/.dx/pins)"
ws="$$(dirname "$(location workspace/MODULE.bazel)")"
"$$public" status --workspace "$$ws" > "$(location public_status.txt)"
"$$public" --version > "$(location public_version.txt)"
"$$public" completion bash --workspace "$$ws" > "$(location public_completion.txt)"
""",
    tools = [
        "@rules_dx//:dx",
        "@rules_dx//cli/cli:dx",
    ],
    visibility = ["//visibility:public"],
)

genrule(
    name = "consumer_status_probe",
    outs = [
        "status_bare.txt",
        "status_corrupt_tool.txt",
        "status_healthy.txt",
        "status_missing_tool.txt",
        "status_pin.txt",
        "status_skew.txt",
        "status_unselected.txt",
    ],
    cmd = """
set -eu
export MSYS=winsymlinks:nativestrict
public="$(location @rules_dx//:dx)"
out="$$(dirname "$(location status_healthy.txt)")"
root="$$(mktemp -d)"
trap 'rm -rf "$$root"' EXIT
env=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
gen=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
setup=3b047baa3764c0439825877a47aabce13030571807c14b34ba597ca402021770
workspace() {
  mkdir -p "$$root/$$1/.dx"
  echo "$$2" > "$$root/$$1/.dx/pins"
  echo 'module(name = "consumer_status", version = "0.0.0")' > "$$root/$$1/MODULE.bazel"
  if [ -n "$$3" ]; then
    echo 'bazel_dep(name = "rules_dx", version = "'"$$3"'")' >> "$$root/$$1/MODULE.bazel"
  fi
}
select_tool() {
  ws="$$root/$$1"
  mkdir -p "$$ws/.dx/local/environments/$$env/artifacts" "$$ws/.dx/local/generated/$$gen" "$$ws/.dx/local/setups/$$setup"
  ln -s "../../environments/$$env/" "$$ws/.dx/local/setups/$$setup/environment"
  ln -s "../../generated/$$gen/" "$$ws/.dx/local/setups/$$setup/generated"
  ln -s "$$setup" "$$ws/.dx/local/setups/current"
  mkdir -p "$$ws/bazel-out"
  echo '#!/bin/sh' > "$$ws/bazel-out/tool"
  chmod +x "$$ws/bazel-out/tool"
  ln -s "$$ws/bazel-out/tool" "$$ws/.dx/local/environments/$$env/artifacts/tool"
  echo '{"tool":"v1"}' > "$$ws/.dx/local/environments/$$env/values.json"
}
status() {
  set +e
  "$$public" status --output=json --capabilities --workspace "$$root/$$1" > "$$out/status_$$1.txt" 2> "$$root/$$1.err"
  code=$$?
  set -e
  echo "exit=$$code" >> "$$out/status_$$1.txt"
  cat "$$root/$$1.err" >> "$$out/status_$$1.txt"
  if [ "$$code" != "$$2" ]; then
    echo "dx status $$1 exited $$code, want $$2" >&2
    cat "$$out/status_$$1.txt" >&2
    exit 1
  fi
}
workspace healthy @@DX_VERSION@@ @@DX_VERSION@@
select_tool healthy
(cd "$$root/healthy" && find . | LC_ALL=C sort) > "$$root/healthy.before"
status healthy 0
(cd "$$root/healthy" && find . | LC_ALL=C sort) > "$$root/healthy.after"
cmp "$$root/healthy.before" "$$root/healthy.after"
workspace unselected @@DX_VERSION@@ @@DX_VERSION@@
status unselected 0
workspace bare @@DX_VERSION@@ ""
status bare 1
workspace skew @@DX_VERSION@@ 9.9.9
status skew 1
workspace missing_tool @@DX_VERSION@@ @@DX_VERSION@@
select_tool missing_tool
rm "$$root/missing_tool/bazel-out/tool"
status missing_tool 1
workspace corrupt_tool @@DX_VERSION@@ @@DX_VERSION@@
select_tool corrupt_tool
rm "$$root/corrupt_tool/.dx/local/environments/$$env/artifacts/tool"
echo 'not the pinned tool' > "$$root/corrupt_tool/.dx/local/environments/$$env/artifacts/tool"
status corrupt_tool 1
workspace pin 9.9.9 @@DX_VERSION@@
status pin 0
""",
    tools = ["@rules_dx//:dx"],
    visibility = ["//visibility:public"],
)
'''

def _consumer_dx_repo_impl(ctx):
    """Writes the consumer repository that depends on the public dx label."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL.replace("@@DX_VERSION@@", DX_VERSION))
    return ctx.repo_metadata(reproducible = True)

consumer_dx_repo = repository_rule(
    implementation = _consumer_dx_repo_impl,
)
