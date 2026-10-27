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
echo 'bazel_dep(name = "rules_dx", version = "0.0.0")' >> "$(location workspace/MODULE.bazel)"
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
  echo "$$2" > "$$root/$$1/.dx/version"
  echo 'module(name = "consumer_status", version = "0.0.0")' > "$$root/$$1/MODULE.bazel"
  if [ -n "$$3" ]; then
    echo 'bazel_dep(name = "rules_dx", version = "'"$$3"'")' >> "$$root/$$1/MODULE.bazel"
  fi
}
select_tool() {
  ws="$$root/$$1"
  mkdir -p "$$ws/.dx/environments/$$env/artifacts" "$$ws/.dx/generated/$$gen" "$$ws/.dx/setups/$$setup"
  ln -s "../../environments/$$env/" "$$ws/.dx/setups/$$setup/environment"
  ln -s "../../generated/$$gen/" "$$ws/.dx/setups/$$setup/generated"
  ln -s "$$setup" "$$ws/.dx/setups/current"
  mkdir -p "$$ws/bazel-out"
  echo '#!/bin/sh' > "$$ws/bazel-out/tool"
  chmod +x "$$ws/bazel-out/tool"
  ln -s "$$ws/bazel-out/tool" "$$ws/.dx/environments/$$env/artifacts/tool"
  echo '{"tool":"v1"}' > "$$ws/.dx/environments/$$env/values.json"
}
status() {
  set +e
  "$$public" status --output=json --verbose --workspace "$$root/$$1" > "$$out/status_$$1.txt" 2> "$$root/$$1.err"
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
workspace healthy 0.0.0 0.0.0
select_tool healthy
(cd "$$root/healthy" && find . | LC_ALL=C sort) > "$$root/healthy.before"
status healthy 0
(cd "$$root/healthy" && find . | LC_ALL=C sort) > "$$root/healthy.after"
cmp "$$root/healthy.before" "$$root/healthy.after"
workspace unselected 0.0.0 0.0.0
status unselected 0
workspace bare 0.0.0 ""
status bare 1
workspace skew 0.0.0 9.9.9
status skew 1
workspace missing_tool 0.0.0 0.0.0
select_tool missing_tool
rm "$$root/missing_tool/bazel-out/tool"
status missing_tool 1
workspace corrupt_tool 0.0.0 0.0.0
select_tool corrupt_tool
rm "$$root/corrupt_tool/.dx/environments/$$env/artifacts/tool"
echo 'not the pinned tool' > "$$root/corrupt_tool/.dx/environments/$$env/artifacts/tool"
status corrupt_tool 1
workspace pin 9.9.9 0.0.0
status pin 1
""",
    tools = ["@rules_dx//:dx"],
    visibility = ["//visibility:public"],
)
'''

def _consumer_dx_repo_impl(ctx):
    """Writes the consumer repository that depends on the public dx label."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    return ctx.repo_metadata(reproducible = True)

consumer_dx_repo = repository_rule(
    implementation = _consumer_dx_repo_impl,
)

_SETS_BUILD_BAZEL = '''genrule(
    name = "consumer_sets_probe",
    outs = [
        "bad_kind.txt",
        "bump_pinned.txt",
        "bump_unsupported.txt",
        "license_json.txt",
        "nolock_license.txt",
        "security_json.txt",
        "security_npm_json.txt",
        "security_ruby_json.txt",
        "update_pinned.txt",
        "sets_ok/dx.toml",
        "sets_ok/MODULE.bazel",
        "sets_ok/.dx/version",
        "sets_ok/apps/frontend/pyproject.toml",
        "sets_ok/apps/frontend/uv.lock",
        "sets_ok/services/worker/pyproject.toml",
        "sets_ok/services/worker/uv.lock",
        "sets_ok/apps/web/package.json",
        "sets_ok/apps/web/pnpm-lock.yaml",
        "sets_ok/apps/rb/Gemfile",
        "sets_ok/apps/rb/Gemfile.lock",
        "sets_ok/tools/ps/PSGallery.requirements.psd1",
        "sets_ok/tools/ps/PSGallery.lock.json",
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
[[dependency_set]]
name = "web"
ecosystem = "npm"
manifests = ["apps/web/package.json"]
locks = ["apps/web/pnpm-lock.yaml"]
scopes = ["apps/web"]
[[dependency_set]]
name = "gems"
ecosystem = "ruby"
manifests = ["apps/rb/Gemfile"]
locks = ["apps/rb/Gemfile.lock"]
scopes = ["apps/rb"]
[[dependency_set]]
name = "modules"
ecosystem = "powershell"
manifests = ["tools/ps/PSGallery.requirements.psd1"]
locks = ["tools/ps/PSGallery.lock.json"]
scopes = ["tools/ps"]
EOF
echo 'source "https://rubygems.org"' > "$(location sets_ok/apps/rb/Gemfile)"
cat > "$(location sets_ok/apps/rb/Gemfile.lock)" <<'EOF'
GEM
  remote: https://rubygems.org/
  specs:

PLATFORMS
  ruby
EOF
echo '@{}' > "$(location sets_ok/tools/ps/PSGallery.requirements.psd1)"
echo '{"modules": {}}' > "$(location sets_ok/tools/ps/PSGallery.lock.json)"
cp "$(location sets_ok/apps/rb/Gemfile.lock)" /tmp/consumer_sets_gemfile_lock.orig
echo '{"name": "web", "version": "1.0.0", "private": true}' > "$(location sets_ok/apps/web/package.json)"
cat > "$(location sets_ok/apps/web/pnpm-lock.yaml)" <<'EOF'
lockfileVersion: '9.0'

importers:

  .: {}
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
gitleaks="$$(type -P true)"
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" > "$(location security_json.txt)" 2>/dev/null; then
  echo "security passed without a PyPI advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess frontend" "$(location security_json.txt)"
grep -q ".dx/advisory/pypi.json" "$(location security_json.txt)"
if grep -Eq 'no advisory coverage for [^"]*(frontend|gems|web|worker)' "$(location security_json.txt)"; then
  echo "security reported configured uv, npm, or ruby sets as uncovered" >&2
  exit 1
fi
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" web > "$(location security_npm_json.txt)" 2>/dev/null; then
  echo "security passed without an npm advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess web" "$(location security_npm_json.txt)"
grep -q ".dx/advisory/npm.json" "$(location security_npm_json.txt)"
if grep -q "no advisory coverage" "$(location security_npm_json.txt)"; then
  echo "security reported a configured npm set as uncovered" >&2
  exit 1
fi
"$$public" update --check --workspace "$$ok" gems modules > "$(location update_pinned.txt)" 2>&1
grep -q "gems pins are manual" "$(location update_pinned.txt)"
grep -q "modules pins are manual" "$(location update_pinned.txt)"
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" gems > "$(location security_ruby_json.txt)" 2>/dev/null; then
  echo "security passed without a RubyGems advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess gems" "$(location security_ruby_json.txt)"
grep -q ".dx/advisory/rubygems.json" "$(location security_ruby_json.txt)"
if grep -q "no advisory coverage" "$(location security_ruby_json.txt)"; then
  echo "security reported a configured ruby set as uncovered" >&2
  exit 1
fi
if "$$public" bump gems:rack 1.0 --workspace "$$ok" > "$(location bump_pinned.txt)" 2>&1; then
  echo "bump unexpectedly widened a configured ruby set" >&2
  exit 1
fi
grep -q "bump is not supported for ecosystem ruby set gems (supported operations: audit; check and update report pinned)" "$(location bump_pinned.txt)"
cmp /tmp/consumer_sets_gemfile_lock.orig "$(location sets_ok/apps/rb/Gemfile.lock)"
"$$public" license --output=json --workspace "$$ok" > "$(location license_json.txt)"
grep -q audit_license_clean "$(location license_json.txt)"
if "$$public" bump frontend 4.1.0 --workspace "$$ok" > "$(location bump_unsupported.txt)" 2>&1; then
  echo "bump unexpectedly widened a configured uv set" >&2
  exit 1
fi
grep -q "bump is not supported for ecosystem uv set frontend" "$(location bump_unsupported.txt)"
if "$$public" update --workspace "$$bad" > "$(location bad_kind.txt)" 2>&1; then
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
    return ctx.repo_metadata(reproducible = True)

consumer_sets_repo = repository_rule(
    implementation = _consumer_sets_repo_impl,
)

_SCAFFOLD_BUILD_BAZEL = '''genrule(
    name = "consumer_scaffold_probe",
    outs = [
        "new_apply.txt",
        "new_check.txt",
        "new_reapply.txt",
        "new_invalid.txt",
        "new_removed.txt",
        "new_c_apply.txt",
        "init_apply.txt",
        "init_check.txt",
        "init_clean.txt",
        "init_bad.txt",
        "foreign/myproj/hello.cc",
        "foreign/myproj/hello.h",
        "foreign/myproj/MODULE.bazel.snippet",
        "foreign/myproj/.dx/version",
        "foreign/myc/hello.c",
        "foreign/myc/hello.h",
        "fortree/.dx/version",
        "fortree/MODULE.bazel.snippet",
    ],
    cmd = """
set -eu
public="$(location @rules_dx//:dx)"
root="$$(dirname "$(location new_apply.txt)")"
foreign="$$root/foreign"
fortree="$$root/fortree"
checktree="$$root/checktree"
rm -rf "$$foreign" "$$fortree" "$$checktree"
mkdir -p "$$foreign" "$$fortree" "$$checktree"
"$$public" new --apply cpp myproj --workspace "$$foreign" > "$(location new_apply.txt)" 2>&1
echo "exit=0" >> "$(location new_apply.txt)"
grep -q "wrote myproj/hello.cc" "$(location new_apply.txt)"
grep -q "wrote myproj/hello.h" "$(location new_apply.txt)"
grep -q "wrote myproj/MODULE.bazel.snippet" "$(location new_apply.txt)"
test -f "$$foreign/myproj/hello.cc"
test -f "$$foreign/myproj/hello.h"
test -f "$$foreign/myproj/MODULE.bazel.snippet"
test -f "$$foreign/myproj/.dx/version"
if "$$public" new cpp checkonly --workspace "$$checktree" > "$(location new_check.txt)" 2>&1; then
  echo "check-mode applied a scaffold" >&2
  exit 1
else
  echo "exit=$$?" >> "$(location new_check.txt)"
fi
grep -q "new drift: missing" "$(location new_check.txt)"
test ! -e "$$checktree/checkonly"
cp "$$foreign/myproj/hello.cc" "$$root/hello.cc.orig"
"$$public" new --apply cpp myproj --workspace "$$foreign" > "$(location new_reapply.txt)" 2>&1
echo "exit=0" >> "$(location new_reapply.txt)"
grep -q "absent-only, left untouched" "$(location new_reapply.txt)"
cmp "$$root/hello.cc.orig" "$$foreign/myproj/hello.cc"
if "$$public" new --apply brainfuck bad --workspace "$$foreign" > "$(location new_invalid.txt)" 2>&1; then
  echo "new accepted an unknown language" >&2
  exit 1
else
  echo "exit=$$?" >> "$(location new_invalid.txt)"
fi
grep -q "unknown language" "$(location new_invalid.txt)"
test ! -e "$$foreign/bad"
for args in "--apply cc bad" "c-sharp bad" "--apply f# bad"; do
  if "$$public" new $$args --workspace "$$foreign" >> "$(location new_removed.txt)" 2>&1; then
    echo "new accepted a removed spelling: $$args" >&2
    exit 1
  else
    echo "exit=$$?" >> "$(location new_removed.txt)"
  fi
done
grep -q "use cpp" "$(location new_removed.txt)"
grep -q "use csharp" "$(location new_removed.txt)"
grep -q "use fsharp" "$(location new_removed.txt)"
test ! -e "$$foreign/bad"
"$$public" new --apply c myc --workspace "$$foreign" > "$(location new_c_apply.txt)" 2>&1
echo "exit=0" >> "$(location new_c_apply.txt)"
test -f "$$foreign/myc/hello.c"
test -f "$$foreign/myc/hello.h"
test ! -e "$$foreign/myc/hello.cc"
"$$public" init --apply demo --workspace "$$fortree" > "$(location init_apply.txt)" 2>&1
echo "exit=0" >> "$(location init_apply.txt)"
grep -q "wrote MODULE.bazel.snippet" "$(location init_apply.txt)"
grep -q "wrote .dx/version" "$(location init_apply.txt)"
test -f "$$fortree/MODULE.bazel.snippet"
test -f "$$fortree/.dx/version"
if "$$public" init demo --workspace "$$checktree" > "$(location init_check.txt)" 2>&1; then
  echo "check-mode applied a scaffold" >&2
  exit 1
else
  echo "exit=$$?" >> "$(location init_check.txt)"
fi
grep -q "init drift: missing" "$(location init_check.txt)"
test ! -e "$$checktree/.dx"
"$$public" init demo --workspace "$$fortree" > "$(location init_clean.txt)" 2>&1
echo "exit=0" >> "$(location init_clean.txt)"
grep -q "init current: demo" "$(location init_clean.txt)"
if "$$public" init "Bad Name" --workspace "$$fortree" > "$(location init_bad.txt)" 2>&1; then
  echo "init accepted an invalid module" >&2
  exit 1
else
  echo "exit=$$?" >> "$(location init_bad.txt)"
fi
grep -q "invalid module name" "$(location init_bad.txt)"
""",
    tools = [
        "@rules_dx//:dx",
    ],
    visibility = ["//visibility:public"],
)
'''

def _consumer_scaffold_repo_impl(ctx):
    """Writes the consumer repository that scaffolds foreign trees with dx."""
    ctx.file("BUILD.bazel", _SCAFFOLD_BUILD_BAZEL)
    return ctx.repo_metadata(reproducible = True)

consumer_scaffold_repo = repository_rule(
    implementation = _consumer_scaffold_repo_impl,
)

_COMPLETION_BUILD_BAZEL = '''genrule(
    name = "consumer_completion_probe",
    testonly = True,
    outs = [
        "completion_argv.txt",
        "completion_broken.txt",
        "completion_config.txt",
        "completion_env.txt",
        "completion_failed_query.txt",
        "completion_flag.txt",
    ],
    cmd = """
set -eu
unset CI
export MSYS=winsymlinks:nativestrict MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'
public="$$PWD/$(location @rules_dx//:dx)"
probe="$$PWD/$(location @rules_dx//tools/testing/process_probe)"
out="$$PWD/$$(dirname "$(location completion_flag.txt)")"
root="$$(cd "$$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$$root"' EXIT
bin="$$root/bin"
ext=
native() { printf '%s' "$$1"; }
case "$$probe" in
  *.exe)
    ext=.exe
    root="$$(cygpath -m -l "$$root")"
    native() { printf '%s' "$$(cygpath -m -l "$$1")"; }
    ;;
esac
ws="$$root/consumer ws ü"
other="$$root/other"
start="$$root/start"
mkdir -p "$$ws/pkg" "$$other/lib" "$$start" "$$root/bin"
for dir in "$$ws" "$$other"; do
  echo 'module(name = "consumer_completion", version = "0.0.0")' > "$$dir/MODULE.bazel"
  touch "$$dir/BUILD.bazel"
done
touch "$$ws/pkg/BUILD.bazel" "$$other/lib/BUILD.bazel"
ln -s "$$public" "$$root/bin/dx$$ext"
cp "$$probe" "$$root/bin/bazel$$ext"
"$$public" completion bash > "$$root/dx.bash"
. "$$root/dx.bash"
export PROCESS_PROBE_SHIM_LOG="$$root/argv.txt" PROCESS_PROBE_SHIM_EXIT=0
export PROCESS_PROBE_SHIM_STDOUT='//pkg:alpha
//pkg:beta
WARNING: not a label
//pkg:another
'
export PATH="$$bin:$$PATH"
complete_words() {
  dest="$$1"
  shift
  COMP_WORDS=("$$@")
  COMP_CWORD=$$(($$# - 1))
  COMP_TYPE=9
  COMPREPLY=()
  set +eu
  _clap_complete_dx dx "$${COMP_WORDS[$$COMP_CWORD]}" "$${COMP_WORDS[$$COMP_CWORD - 1]}" 2> "$$root/stderr.txt"
  for reply in "$${COMPREPLY[@]}"; do
    echo "candidate=$$reply"
  done > "$$dest"
  set -eu
  echo "stderr_bytes=$$(wc -c < "$$root/stderr.txt" | tr -d ' ')" >> "$$dest"
}
# Hermetic rc policy (CI set) pins the --nohome_rc --nosystem_rc startup tokens on any host.
(cd "$$start" && export CI=true && complete_words "$$out/completion_flag.txt" dx build --workspace "$$ws" "--bazel-startup-option=--output_base=$$root/base ü" "--bazel-startup-option=--output_user_root=$$root/user root" "//pkg:a")
{ native "$$(head -n 1 "$$root/argv.txt")"; echo; tail -n +2 "$$root/argv.txt"; } | sed "s|$$root|ROOT|g" > "$$out/completion_argv.txt"
(cd "$$start" && export DX_WORKSPACE="$$other" && complete_words "$$out/completion_env.txt" dx build "")
echo '[dx]' > "$$start/dx.toml"
echo 'workspace = "'"$$ws"'"' >> "$$start/dx.toml"
(cd "$$start" && complete_words "$$out/completion_config.txt" dx build "//")
echo 'not toml = [' > "$$start/dx.toml"
(cd "$$start" && complete_words "$$out/completion_broken.txt" dx build "")
rm "$$start/dx.toml"
(cd "$$start" && export PROCESS_PROBE_SHIM_EXIT=1 && complete_words "$$out/completion_failed_query.txt" dx build --workspace "$$ws" "//pkg:")
""",
    tools = [
        "@rules_dx//:dx",
        "@rules_dx//tools/testing/process_probe",
    ],
    visibility = ["//visibility:public"],
)
'''

def _consumer_completion_repo_impl(ctx):
    """Writes the consumer repository that drives the public dx shell callback."""
    ctx.file("BUILD.bazel", _COMPLETION_BUILD_BAZEL)
    return ctx.repo_metadata(reproducible = True)

consumer_completion_repo = repository_rule(
    implementation = _consumer_completion_repo_impl,
)
