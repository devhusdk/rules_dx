"""Gleam compiled to JavaScript with the pinned compiler and run on the managed Node.js runtime."""

load("@bazel_skylib//lib:paths.bzl", "paths")

GleamJsInfo = provider(
    doc = "One Gleam package compiled to JavaScript.",
    fields = {
        "package": "Gleam package name of this target.",
        "tree": "Directory with this package's compiled modules.",
        "trees": "Depset of this package's and its dependencies' directories.",
    },
)

GLEAM_TOOLCHAIN = "@rules_gleam//gleam_tools:toolchain_type"

NODE_TOOLCHAIN = "@rules_nodejs//nodejs:runtime_toolchain_type"

_COMPILE = """set -eu
compiler="$PWD/$1"
out="$PWD/$2"
toml="$3"
strict="$4"
label="$5"
shift 5
pkg="$(mktemp -d)"
lib="$(mktemp -d)"
log="$(mktemp)"
trap 'rm -rf "$pkg" "$lib" "$log"' EXIT
mkdir -p "$pkg/src" "$pkg/test"
printf '%s' "$toml" > "$pkg/gleam.toml"
while [ "$#" -gt 0 ]; do
    if [ "$1" = --lib ]; then
        ln -s "$PWD/$2" "$lib/${2##*/}"
    else
        mkdir -p "$pkg/$(dirname "$1")"
        cp "$2" "$pkg/$1"
        TZ=UTC touch -t 200001010000 "$pkg/$1"
    fi
    shift 2
done
cd "$pkg"
if ! "$compiler" compile-package --target javascript --no-beam --package . --out "$out" --lib "$lib" --javascript-prelude ../prelude.mjs >"$log" 2>&1; then
    cat "$log" >&2
    exit 1
fi
cat "$log" >&2
if [ "$strict" = 1 ] && grep -q '^warning' "$log"; then
    echo "error: Gleam warnings are errors for $label" >&2
    exit 1
fi
"""

_BUNDLE = """set -eu
compiler="$1"
out="$2"
shift 2
mkdir -p "$out"
"$compiler" export javascript-prelude > "$out/prelude.mjs"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --tree)
            cp -RL "$2" "$out/${2##*/}"
            rm -rf "$out/${2##*/}/_gleam_artefacts"
            ;;
        --text)
            printf '%s' "$3" > "$out/$2"
            shift
            ;;
        *)
            mkdir -p "$out/$(dirname "$1")"
            cp "$2" "$out/$1"
            ;;
    esac
    shift 2
done
"""

_LAUNCHER = """#!/usr/bin/env bash
set -euo pipefail
if [[ -d "$0.runfiles" ]]; then
    runfiles="$0.runfiles"
elif [[ -n "${{RUNFILES_DIR:-}}" ]]; then
    runfiles="$RUNFILES_DIR"
elif [[ "$0" == *.runfiles/* ]]; then
    runfiles="${{0%%.runfiles/*}}.runfiles"
else
    echo "$0: no runfiles tree" >&2
    exit 2
fi
node="$runfiles/{node}"
bundle="$runfiles/{bundle}"
{run}
"""

_TEST_RUN = """bundle="$(cd "$bundle" && pwd)"
node="$(cd "$(dirname "$node")" && pwd)/${node##*/}"
work="$(mktemp -d "${TEST_TMPDIR:-${TMPDIR:-/tmp}}/gleam_js.XXXXXX")"
trap 'rm -rf "$work"' EXIT
for entry in "$PWD"/* "$PWD"/.[!.]*; do
    if [[ -e "$entry" ]]; then
        ln -s "$entry" "$work/${entry##*/}"
    fi
done
rm -f "$work/gleam.toml" "$work/test"
ln -s "$bundle/gleam.toml" "$work/gleam.toml"
ln -s "$bundle/test" "$work/test"
cd "$work"
status=0
"$node" "$bundle/dx_main.mjs" "$@" || status=$?
exit "$status"
"""

def package_name(repo, package, name):
    """Returns the default Gleam package name for one target."""
    raw = "_".join([part for part in [repo, package, name] if part]).lower()
    name = "".join([c if c.isalnum() or c == "_" else "_" for c in raw.elems()])
    return name if name[0].isalpha() else "p_" + name

def gleam_toml(package, deps):
    """Returns the gleam.toml naming one package and its direct dependencies."""
    lines = ['name = "{}"'.format(package), 'version = "0.0.0"', "", "[dependencies]"]
    lines.extend(['{} = ">= 0.0.0"'.format(dep) for dep in sorted(deps)])
    return "\n".join(lines) + "\n"

def source_path(file, prefix, directory):
    """Returns where one source sits in the staged Gleam package."""
    path = file.short_path
    if path.startswith("../"):
        path = path.split("/", 2)[2]
    prefix = prefix.removesuffix("/")
    if prefix:
        if not path.startswith(prefix + "/"):
            fail("Gleam source {} is not below {}".format(path, prefix))
        path = path.removeprefix(prefix + "/")
    return paths.join(directory, path)

def module_path(file, prefix):
    """Returns the Gleam module path one source compiles to."""
    return paths.replace_extension(source_path(file, prefix, ""), "")

def unique_trees(trees):
    """Returns the package directories, failing when two packages share a name."""
    seen = {}
    for tree in trees:
        if tree.basename in seen and seen[tree.basename] != tree:
            fail("Gleam JavaScript packages {} and {} share the package name {}; set package on one of them.".format(seen[tree.basename].path, tree.path, tree.basename))
        seen[tree.basename] = tree
    return [seen[name] for name in sorted(seen.keys())]

def runfiles_path(ctx, file):
    """Returns one file's path below the runfiles root."""
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return paths.join(ctx.workspace_name, file.short_path)

def _source_root(ctx, directory):
    if ctx.attr.strip_src_prefix:
        return ctx.attr.strip_src_prefix
    return paths.join(ctx.label.package, directory)

def _compile(ctx, directory):
    package = ctx.attr.package or package_name(ctx.label.repo_name, ctx.label.package, ctx.label.name)
    deps = [dep[GleamJsInfo] for dep in ctx.attr.deps]
    compiler = ctx.toolchains[GLEAM_TOOLCHAIN].gleamtools.compiler
    tree = ctx.actions.declare_directory(paths.join(ctx.label.name + ".gleam_js", package))
    args = ctx.actions.args()
    args.add_all([compiler, tree.path, gleam_toml(package, [dep.package for dep in deps]), "1" if ctx.attr.warnings_as_errors else "0", str(ctx.label)])
    prefix = _source_root(ctx, directory)
    for src in ctx.files.srcs:
        args.add_all([source_path(src, prefix, directory), src])
    for dep in deps:
        args.add_all(["--lib", dep.tree.path])
    ctx.actions.run_shell(
        command = _COMPILE,
        arguments = [args],
        inputs = depset(ctx.files.srcs + [dep.tree for dep in deps]),
        tools = [compiler],
        outputs = [tree],
        mnemonic = "GleamJavaScriptCompile",
        progress_message = "Compiling Gleam %{label} to JavaScript",
    )
    return GleamJsInfo(
        package = package,
        tree = tree,
        trees = depset([tree], transitive = [dep.trees for dep in deps]),
    )

def _gleam_js_library_impl(ctx):
    info = _compile(ctx, "src")
    runfiles = ctx.runfiles(ctx.files.data).merge_all([dep[DefaultInfo].default_runfiles for dep in ctx.attr.deps + ctx.attr.data])
    return [DefaultInfo(files = depset([info.tree]), runfiles = runfiles), info]

def _executable(ctx, info, entry, extra, run):
    compiler = ctx.toolchains[GLEAM_TOOLCHAIN].gleamtools.compiler
    node = ctx.toolchains[NODE_TOOLCHAIN].nodeinfo.node
    bundle = ctx.actions.declare_directory(ctx.label.name + ".gleam_js_bundle")
    args = ctx.actions.args()
    args.add_all([compiler, bundle.path])
    trees = unique_trees(info.trees.to_list())
    for tree in trees:
        args.add_all(["--tree", tree.path])
    args.add_all(["--text", "dx_main.mjs", entry])
    inputs = list(trees)
    for path, file in extra:
        args.add_all([path, file])
        inputs.append(file)
    ctx.actions.run_shell(
        command = _BUNDLE,
        arguments = [args],
        inputs = inputs,
        tools = [compiler],
        outputs = [bundle],
        mnemonic = "GleamJavaScriptBundle",
        progress_message = "Bundling Gleam JavaScript for %{label}",
    )
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(
        launcher,
        _LAUNCHER.format(node = runfiles_path(ctx, node), bundle = runfiles_path(ctx, bundle), run = run),
        is_executable = True,
    )
    runfiles = ctx.runfiles([launcher, bundle, node] + ctx.files.data)
    runfiles = runfiles.merge_all([dep[DefaultInfo].default_runfiles for dep in ctx.attr.deps + ctx.attr.data])
    return [DefaultInfo(executable = launcher, files = depset([launcher, bundle]), runfiles = runfiles), info]

def _gleam_js_binary_impl(ctx):
    info = _compile(ctx, "src")
    prefix = _source_root(ctx, "src")
    main_module = ctx.attr.main_module
    if not main_module:
        if len(ctx.files.srcs) != 1:
            fail("gleam_js_binary needs main_module when it has more than one source", "main_module")
        main_module = module_path(ctx.files.srcs[0], prefix)
    entry = 'import {{ main }} from "./{}/{}.mjs";\nmain();\n'.format(info.package, main_module)
    return _executable(ctx, info, entry, [], 'exec "$node" "$bundle/dx_main.mjs" "$@"')

def _gleam_js_test_impl(ctx):
    info = _compile(ctx, "test")
    if "gleeunit" not in [tree.basename for tree in info.trees.to_list()]:
        fail("gleam_js_test needs gleeunit compiled to JavaScript in deps", "deps")
    prefix = _source_root(ctx, "test")
    extra = [(source_path(src, prefix, "test"), src) for src in ctx.files.srcs if src.extension == "gleam"]
    entry = 'import { main } from "./gleeunit/gleeunit.mjs";\nmain();\n'
    toml = ctx.actions.declare_file(ctx.label.name + ".gleam_js_test/gleam.toml")
    ctx.actions.write(toml, gleam_toml(info.package, []))
    return _executable(ctx, info, entry, extra + [("gleam.toml", toml)], _TEST_RUN)

_COMMON_ATTRS = {
    "data": attr.label_list(allow_files = True, doc = "Files available at runtime."),
    "deps": attr.label_list(providers = [GleamJsInfo], doc = "Gleam packages compiled to JavaScript."),
    "package": attr.string(doc = "Gleam package name; derived from the label when empty."),
    "strip_src_prefix": attr.string(doc = "Repository directory that module paths are relative to; the package's src/ or test/ when empty."),
    "warnings_as_errors": attr.bool(doc = "Fail the compile when Gleam prints a warning."),
}

_SRCS = attr.label_list(allow_files = [".gleam", ".mjs"], doc = "Gleam modules and JavaScript FFI files.")

gleam_js_library = rule(
    implementation = _gleam_js_library_impl,
    doc = "Compiles one Gleam package to JavaScript.",
    attrs = dict(_COMMON_ATTRS, srcs = _SRCS),
    provides = [GleamJsInfo],
    toolchains = [GLEAM_TOOLCHAIN],
)

gleam_js_binary = rule(
    implementation = _gleam_js_binary_impl,
    doc = "Runs one Gleam module's main function on Node.js.",
    attrs = dict(_COMMON_ATTRS, srcs = _SRCS, main_module = attr.string(doc = "Module whose main runs; the only source when empty.")),
    executable = True,
    toolchains = [GLEAM_TOOLCHAIN, NODE_TOOLCHAIN],
)

gleam_js_test = rule(
    implementation = _gleam_js_test_impl,
    doc = "Runs gleeunit tests on Node.js.",
    attrs = dict(_COMMON_ATTRS, srcs = _SRCS),
    test = True,
    toolchains = [GLEAM_TOOLCHAIN, NODE_TOOLCHAIN],
)
