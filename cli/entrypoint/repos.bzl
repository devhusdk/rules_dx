"""A consumer repository outside this module that runs the public dx launcher."""

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
echo 'bazel_dep(name = "rules_dx", version = "0.0.0")' >> "$(location workspace/MODULE.bazel)"
echo '0.0.0' > "$(location workspace/.dx/pins)"
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
rm "$$root/corrupt_tool/.dx/local/environments/$$env/artifacts/tool"
echo 'not the pinned tool' > "$$root/corrupt_tool/.dx/local/environments/$$env/artifacts/tool"
status corrupt_tool 1
workspace pin 9.9.9 0.0.0
status pin 0
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
        "edit_cargo.txt",
        "edit_composer.txt",
        "edit_opam.txt",
        "edit_pinned.txt",
        "edit_unsupported.txt",
        "license_json.txt",
        "nolock_license.txt",
        "security_cargo_json.txt",
        "security_cpan_json.txt",
        "security_composer_json.txt",
        "security_luarocks_json.txt",
        "security_json.txt",
        "security_npm_json.txt",
        "security_ruby_json.txt",
        "security_swiftpm_json.txt",
        "update_pinned.txt",
        "sets_ok/MODULE.bazel",
        "sets_ok/.dx/pins",
        "sets_ok/apps/frontend/pyproject.toml",
        "sets_ok/apps/frontend/uv.lock",
        "sets_ok/services/worker/pyproject.toml",
        "sets_ok/services/worker/uv.lock",
        "sets_ok/apps/web/package.json",
        "sets_ok/apps/web/pnpm-lock.yaml",
        "sets_ok/apps/rb/Gemfile",
        "sets_ok/apps/rb/Gemfile.lock",
        "sets_ok/apps/rs/Cargo.toml",
        "sets_ok/apps/rs/Cargo.lock",
        "sets_ok/perl/cpanfile",
        "sets_ok/perl/cpanfile.snapshot",
        "sets_ok/perl/cpanfile.snapshot.lock.json",
        "sets_ok/apps/swift/Package.swift",
        "sets_ok/apps/swift/Package.resolved",
        "sets_ok/apps/php/composer.json",
        "sets_ok/apps/php/composer.lock",
        "sets_ok/apps/php/composer.digests.json",
        "sets_ok/apps/lua/luarocks.lock",
        "sets_ok/apps/lua/luarocks.digests.json",
        "sets_ok/apps/ml/ocaml_app.opam",
        "sets_ok/apps/ml/ocaml_app.opam.locked",
        "sets_bad/MODULE.bazel",
        "sets_bad/.dx/pins",
        "sets_nolock/MODULE.bazel",
        "sets_nolock/.dx/pins",
        "sets_nolock/apps/frontend/pyproject.toml",
    ],
    cmd = """
set -eu
public="$(location @rules_dx//:dx)"
ok="$$(dirname "$(location sets_ok/MODULE.bazel)")"
bad="$$(dirname "$(location sets_bad/MODULE.bazel)")"
nolock="$$(dirname "$(location sets_nolock/MODULE.bazel)")"
orig="$$(mktemp -d)"
trap 'rm -rf "$$orig"' EXIT
for ws in "$$ok" "$$bad" "$$nolock"; do
  echo 'module(name = "consumer_sets", version = "0.0.0")' > "$$ws/MODULE.bazel"
  mkdir -p "$$ws/.dx"
  echo '0.0.0' > "$$ws/.dx/pins"
done
cat >> "$(location sets_ok/MODULE.bazel)" <<'EOF'
uv = use_extension("@rules_py//uv:extensions.bzl", "uv")
uv.project(hub_name = "pypi", lock = "//apps/frontend:uv.lock", pyproject = "//apps/frontend:pyproject.toml")
uv.project(hub_name = "pypi", lock = "//services/worker:uv.lock", pyproject = "//services/worker:pyproject.toml")
npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(name = "web", pnpm_lock = "//apps/web:pnpm-lock.yaml")
ruby = use_extension("@rules_dx//ruby:extensions.bzl", "ruby")
ruby.bundle_fetch(name = "gems", gemfile = "//apps/rb:Gemfile", gemfile_lock = "//apps/rb:Gemfile.lock")
crate = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
crate.from_cargo(name = "crates", cargo_lockfile = "//apps/rs:Cargo.lock", manifests = ["//apps/rs:Cargo.toml"])
cpan = use_extension("@rules_perl//perl/cpan:extensions.bzl", "cpan")
cpan.install(name = "cpan_deps", lock = "//perl:cpanfile.snapshot.lock.json")
swift_deps = use_extension("@rules_swift_package_manager//:extensions.bzl", "swift_deps")
swift_deps.from_package(resolved = "//apps/swift:Package.resolved", swift = "//apps/swift:Package.swift")
composer = use_extension("@rules_dx//php:extensions.bzl", "composer")
composer.lock(name = "vendor", lock = "//apps/php:composer.lock", digests = "//apps/php:composer.digests.json")
luarocks = use_extension("@rules_dx//lua/luarocks:extensions.bzl", "luarocks")
luarocks.lock(name = "rocks", lock = "//apps/lua:luarocks.lock", digests = "//apps/lua:luarocks.digests.json")
ocaml = use_extension("@rules_dx//ocaml/toolchain:extensions.bzl", "ocaml")
ocaml.lock(name = "ocaml_app", lock = "//apps/ml:ocaml_app.opam.locked")
EOF
echo "requires 'Text::CSV', '== 2.06';" > "$(location sets_ok/perl/cpanfile)"
cat > "$(location sets_ok/perl/cpanfile.snapshot)" <<'EOF'
# carton snapshot format: version 1.0
DISTRIBUTIONS
  Text-CSV-2.06
    pathname: I/IS/ISHIGAKI/Text-CSV-2.06.tar.gz
EOF
echo '{"Text-CSV": {"strip_prefix": "Text-CSV-2.06"}}' > "$(location sets_ok/perl/cpanfile.snapshot.lock.json)"
echo 'import PackageDescription' > "$(location sets_ok/apps/swift/Package.swift)"
echo '{"pins": [], "version": 3}' > "$(location sets_ok/apps/swift/Package.resolved)"
echo '{"require": {"psr/log": "^1.1"}}' > "$(location sets_ok/apps/php/composer.json)"
echo '{"packages": [], "packages-dev": []}' > "$(location sets_ok/apps/php/composer.lock)"
echo '{"packages": {}}' > "$(location sets_ok/apps/php/composer.digests.json)"
printf 'return {\n   dependencies = {\n      inspect = "3.1.3-0",\n   },\n}\n' > "$(location sets_ok/apps/lua/luarocks.lock)"
echo '{"packages": {}}' > "$(location sets_ok/apps/lua/luarocks.digests.json)"
cp "$(location sets_ok/apps/php/composer.json)" "$$orig/consumer_sets_composer_json.orig"
printf 'opam-version: "2.0"\ndepends: [\n  "astring" {>= "0.8.5"}\n]\n' > "$(location sets_ok/apps/ml/ocaml_app.opam)"
printf 'opam-version: "2.0"\ndepends: [\n  "astring" {= "0.8.5"}\n]\n' > "$(location sets_ok/apps/ml/ocaml_app.opam.locked)"
cp "$(location sets_ok/apps/ml/ocaml_app.opam)" "$$orig/consumer_sets_opam.orig"
cat > "$(location sets_ok/apps/rs/Cargo.toml)" <<'EOF'
[package]
name = "app"
version = "0.1.0"
EOF
cat > "$(location sets_ok/apps/rs/Cargo.lock)" <<'EOF'
version = 4

[[package]]
name = "app"
version = "0.1.0"
EOF
cp "$(location sets_ok/apps/rs/Cargo.lock)" "$$orig/consumer_sets_cargo_lock.orig"
echo 'source "https://rubygems.org"' > "$(location sets_ok/apps/rb/Gemfile)"
cat > "$(location sets_ok/apps/rb/Gemfile.lock)" <<'EOF'
GEM
  remote: https://rubygems.org/
  specs:

PLATFORMS
  ruby
EOF
cp "$(location sets_ok/apps/rb/Gemfile.lock)" "$$orig/consumer_sets_gemfile_lock.orig"
echo '{"name": "web", "version": "1.0.0", "private": true}' > "$(location sets_ok/apps/web/package.json)"
cat > "$(location sets_ok/apps/web/pnpm-lock.yaml)" <<'EOF'
lockfileVersion: '9.0'

importers:

  .: {}
EOF
cat >> "$(location sets_bad/MODULE.bazel)" <<'EOF'
npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(**HUB)
EOF
cat >> "$(location sets_nolock/MODULE.bazel)" <<'EOF'
uv = use_extension("@rules_py//uv:extensions.bzl", "uv")
uv.project(hub_name = "pypi", lock = "//apps/frontend:uv.lock", pyproject = "//apps/frontend:pyproject.toml")
EOF
for project in "$(location sets_ok/apps/frontend/pyproject.toml)" "$(location sets_ok/services/worker/pyproject.toml)" "$(location sets_nolock/apps/frontend/pyproject.toml)"; do
  cat > "$$project" <<'EOF'
[project]
name = "project"
version = "0.1.0"
requires-python = ">=3.9"
dependencies = ["anyio>=4"]
EOF
done
for lock in "$(location sets_ok/apps/frontend/uv.lock)" "$(location sets_ok/services/worker/uv.lock)"; do
  cat > "$$lock" <<'EOF'
version = 1
requires-python = ">=3.9"
[[package]]
name = "anyio"
version = "4.0.0"
source = { registry = "https://pypi.org/simple" }
EOF
done
cp "$(location sets_ok/apps/frontend/uv.lock)" "$$orig/consumer_sets_frontend_uv_lock.orig"
cp "$(location sets_ok/services/worker/uv.lock)" "$$orig/consumer_sets_worker_uv_lock.orig"
gitleaks="$$(type -P true)"
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" pypi-frontend > "$(location security_json.txt)" 2>/dev/null; then
  echo "security passed without a PyPI advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess pypi-frontend" "$(location security_json.txt)"
grep -q 'no `advisory pypi` line in .dx/pins' "$(location security_json.txt)"
if grep -Eq 'no advisory coverage for [^"]*(pypi-frontend|gems|crates|cpan_deps|vendor|web|pypi-worker)' "$(location security_json.txt)"; then
  echo "security reported configured uv, npm, or ruby sets as uncovered" >&2
  exit 1
fi
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" web > "$(location security_npm_json.txt)" 2>/dev/null; then
  echo "security passed without an npm advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess web" "$(location security_npm_json.txt)"
grep -q 'no `advisory npm` line in .dx/pins' "$(location security_npm_json.txt)"
if grep -q "no advisory coverage" "$(location security_npm_json.txt)"; then
  echo "security reported a configured npm set as uncovered" >&2
  exit 1
fi
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" crates > "$(location security_cargo_json.txt)" 2>/dev/null; then
  echo "security passed without a cargo advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess crates" "$(location security_cargo_json.txt)"
grep -q 'no `advisory cargo` line in .dx/pins' "$(location security_cargo_json.txt)"
if grep -q "no advisory coverage" "$(location security_cargo_json.txt)"; then
  echo "security reported a configured cargo set as uncovered" >&2
  exit 1
fi
if "$$public" update crates:serde 2.0.0 --workspace "$$ok" > "$(location edit_cargo.txt)" 2>&1; then
  echo "update unexpectedly widened a configured cargo set" >&2
  exit 1
fi
grep -q 'update pending: crates:serde 2.0.0 is not declared in apps/rs/Cargo.toml' "$(location edit_cargo.txt)"
# The crate_universe hub is not in use_repo: dx refuses to run it.
if "$$public" update --workspace "$$ok" crates >> "$(location edit_cargo.txt)" 2>&1; then
  echo "update checked a cargo set without its crate_universe hub" >&2
  exit 1
fi
grep -q 'cannot resolve cargo hub "crates": MODULE.bazel must list "crates" in use_repo' "$(location edit_cargo.txt)"
cmp "$$orig/consumer_sets_cargo_lock.orig" "$(location sets_ok/apps/rs/Cargo.lock)"
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" vendor > "$(location security_composer_json.txt)" 2>/dev/null; then
  echo "security passed without a Packagist advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess vendor" "$(location security_composer_json.txt)"
grep -q 'no `advisory packagist` line in .dx/pins' "$(location security_composer_json.txt)"
if grep -q "no advisory coverage" "$(location security_composer_json.txt)"; then
  echo "security reported a configured composer set as uncovered" >&2
  exit 1
fi
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" rocks > "$(location security_luarocks_json.txt)" 2>/dev/null; then
  echo "security passed for a LuaRocks set without an uncovered entry" >&2
  exit 1
fi
grep -q "failed to assess rocks" "$(location security_luarocks_json.txt)"
grep -q "OSV defines no LuaRocks ecosystem" "$(location security_luarocks_json.txt)"
if "$$public" update vendor:acme/other 1.0.0 --apply --workspace "$$ok" > "$(location edit_composer.txt)" 2>&1; then
  echo "update declared a package in a configured composer set" >&2
  exit 1
fi
cmp "$$orig/consumer_sets_composer_json.orig" "$(location sets_ok/apps/php/composer.json)"
if "$$public" update ocaml_app:lwt 5.7.0 --apply --workspace "$$ok" > "$(location edit_opam.txt)" 2>&1; then
  echo "update declared a package in a configured opam set" >&2
  exit 1
fi
cmp "$$orig/consumer_sets_opam.orig" "$(location sets_ok/apps/ml/ocaml_app.opam)"
"$$public" update --workspace "$$ok" gems > "$(location update_pinned.txt)" 2>&1
grep -q "gems pins are manual" "$(location update_pinned.txt)"
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" gems > "$(location security_ruby_json.txt)" 2>/dev/null; then
  echo "security passed without a RubyGems advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess gems" "$(location security_ruby_json.txt)"
grep -q 'no `advisory rubygems` line in .dx/pins' "$(location security_ruby_json.txt)"
if grep -q "no advisory coverage" "$(location security_ruby_json.txt)"; then
  echo "security reported a configured ruby set as uncovered" >&2
  exit 1
fi
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" cpan_deps > "$(location security_cpan_json.txt)" 2>/dev/null; then
  echo "security passed without a cpan advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess cpan_deps" "$(location security_cpan_json.txt)"
grep -q 'no `advisory cpan` line in .dx/pins' "$(location security_cpan_json.txt)"
if grep -q "no advisory coverage" "$(location security_cpan_json.txt)"; then
  echo "security reported a configured cpan set as uncovered" >&2
  exit 1
fi
if DX_GITLEAKS_BIN="$$gitleaks" "$$public" security --output=json --workspace "$$ok" swiftpm > "$(location security_swiftpm_json.txt)" 2>/dev/null; then
  echo "security passed without a swifturl advisory snapshot" >&2
  exit 1
fi
grep -q "failed to assess swiftpm" "$(location security_swiftpm_json.txt)"
grep -q 'no `advisory swifturl` line in .dx/pins' "$(location security_swiftpm_json.txt)"
if grep -q "no advisory coverage" "$(location security_swiftpm_json.txt)"; then
  echo "security reported a configured swiftpm set as uncovered" >&2
  exit 1
fi
if "$$public" update gems:rack 1.0 --workspace "$$ok" > "$(location edit_pinned.txt)" 2>&1; then
  echo "update unexpectedly widened a configured ruby set" >&2
  exit 1
fi
grep -q "update pending: gems:rack 1.0 is not declared in apps/rb/Gemfile" "$(location edit_pinned.txt)"
cmp "$$orig/consumer_sets_gemfile_lock.orig" "$(location sets_ok/apps/rb/Gemfile.lock)"
"$$public" license --output=json --workspace "$$ok" > "$(location license_json.txt)"
grep -q audit_license_clean "$(location license_json.txt)"
if "$$public" update pypi-frontend 4.1.0 --workspace "$$ok" > "$(location edit_unsupported.txt)" 2>&1; then
  echo "update accepted a bare configured uv set" >&2
  exit 1
fi
grep -q "update needs one package, not a whole set" "$(location edit_unsupported.txt)"
if "$$public" update --workspace "$$bad" > "$(location bad_kind.txt)" 2>&1; then
  echo "update accepted a computed hub" >&2
  exit 1
fi
grep -q "needs literal arguments" "$(location bad_kind.txt)"
if "$$public" license --output=json --workspace "$$nolock" > "$(location nolock_license.txt)" 2>&1; then
  echo "license passed with a missing lockfile" >&2
  exit 1
fi
grep -q "failed to assess pypi-frontend" "$(location nolock_license.txt)"
grep -q "apps/frontend/uv.lock" "$(location nolock_license.txt)"
cmp "$$orig/consumer_sets_frontend_uv_lock.orig" "$(location sets_ok/apps/frontend/uv.lock)"
cmp "$$orig/consumer_sets_worker_uv_lock.orig" "$(location sets_ok/services/worker/uv.lock)"
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
        "new_removed.txt",
        "init_dir_apply.txt",
        "init_dir_reapply.txt",
        "init_apply.txt",
        "init_check.txt",
        "init_clean.txt",
        "init_bad.txt",
        "foreign/myproj/MODULE.bazel",
        "foreign/myproj/.bazelversion",
        "foreign/myproj/.dx/pins",
        "fortree/.dx/pins",
    ],
    cmd = """
set -eu
public="$(location @rules_dx//:dx)"
root="$$(dirname "$(location init_apply.txt)")"
foreign="$$root/foreign"
fortree="$$root/fortree"
checktree="$$root/checktree"
rm -rf "$$foreign" "$$fortree" "$$checktree"
mkdir -p "$$foreign" "$$fortree" "$$checktree"
echo "keep" > "$$fortree/README.md"
mkdir -p "$$fortree/src"
echo "keep" > "$$fortree/src/keep.txt"
echo 'module(name = "mine")' > "$$fortree/MODULE.bazel"
echo "keep" > "$$checktree/README.md"
if "$$public" new --apply cpp bad --workspace "$$foreign" > "$(location new_removed.txt)" 2>&1; then
  echo "dx new still runs" >&2
  exit 1
else
  echo "exit=$$?" >> "$(location new_removed.txt)"
fi
grep -q "unrecognized subcommand 'new'" "$(location new_removed.txt)"
test ! -e "$$foreign/bad"
"$$public" init --apply myproj --workspace "$$foreign" > "$(location init_dir_apply.txt)" 2>&1
echo "exit=0" >> "$(location init_dir_apply.txt)"
grep -q "wrote myproj/.dx/pins" "$(location init_dir_apply.txt)"
test -f "$$foreign/myproj/MODULE.bazel"
test -f "$$foreign/myproj/.bazelversion"
test -f "$$foreign/myproj/.dx/pins"
grep -q 'name = "myproj"' "$$foreign/myproj/MODULE.bazel"
cp "$$foreign/myproj/MODULE.bazel" "$$root/module.orig"
"$$public" init --apply myproj --workspace "$$foreign" > "$(location init_dir_reapply.txt)" 2>&1
echo "exit=0" >> "$(location init_dir_reapply.txt)"
cmp "$$root/module.orig" "$$foreign/myproj/MODULE.bazel"
"$$public" init --apply --workspace "$$fortree" > "$(location init_apply.txt)" 2>&1
echo "exit=0" >> "$(location init_apply.txt)"
grep -q "wrote .dx/pins" "$(location init_apply.txt)"
test -f "$$fortree/.dx/pins"
grep -q 'name = "mine"' "$$fortree/MODULE.bazel"
if "$$public" init --workspace "$$checktree" > "$(location init_check.txt)" 2>&1; then
  echo "check-mode applied a scaffold" >&2
  exit 1
else
  echo "exit=$$?" >> "$(location init_check.txt)"
fi
grep -q "would create" "$(location init_check.txt)"
test ! -e "$$checktree/.dx"
"$$public" init --workspace "$$fortree" > "$(location init_clean.txt)" 2>&1
echo "exit=0" >> "$(location init_clean.txt)"
grep -q "init current: my_project" "$(location init_clean.txt)"
if "$$public" init ../evil --workspace "$$fortree" > "$(location init_bad.txt)" 2>&1; then
  echo "init accepted a directory outside the workspace" >&2
  exit 1
else
  echo "exit=$$?" >> "$(location init_bad.txt)"
fi
grep -q "invalid directory for dx init" "$(location init_bad.txt)"
test ! -e "$$root/evil"
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
(cd "$$start" && complete_words "$$out/completion_env.txt" dx build --workspace "$$other" "")
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
