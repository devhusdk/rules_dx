"""Builds one executable of a vendored OCaml source tree with dune, inside Bazel actions.

The rule is generic: it takes a source tree that holds one dune workspace (a
dune-project, a dune file with vendored_dirs, and one directory per package)
and a dune target. Dune itself is built from the `dune_src` directory of that
tree by its own bootstrap script. Both actions use only the pinned conda OCaml
tree, the resolved C toolchain, and /usr/bin and /bin as the host baseline;
dune's cache and the network are off.
"""

load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

DUNE_DIRECTORY = "dune_src"

_PRELUDE = """set -euo pipefail
ROOT="$PWD"
SCRATCH="$(mktemp -d)"
trap 'chmod -R u+w "$SCRATCH" 2>/dev/null || true; rm -rf "$SCRATCH"' EXIT
{env}
# ocamlopt hands the assembler GNU's --debug-prefix-map; clang spells it differently, so drop it.
mkdir -p "$SCRATCH/bin"
export REAL_AS="$CONDA_OCAML_AS"
cat > "$SCRATCH/bin/as" <<'ASM'
#!/bin/bash
args=()
skip=0
for arg in "$@"; do
  if [ "$skip" = 1 ]; then skip=0; continue; fi
  if [ "$arg" = --debug-prefix-map ]; then skip=1; continue; fi
  args+=("$arg")
done
exec $REAL_AS "${{args[@]}}"
ASM
chmod +x "$SCRATCH/bin/as"
export CONDA_OCAML_AS="$SCRATCH/bin/as"
export DUNE_CACHE=disabled
export HOME="$SCRATCH/home" XDG_CACHE_HOME="$SCRATCH/home/.cache" XDG_CONFIG_HOME="$SCRATCH/home/.config" XDG_DATA_HOME="$SCRATCH/home/.local"
export TMPDIR="$SCRATCH/tmp"
mkdir -p "$HOME" "$TMPDIR"
"""

_BOOTSTRAP = """cp -rL "$ROOT/{dune_root}/." "$SCRATCH/dune"
chmod -R u+w "$SCRATCH/dune"
cd "$SCRATCH/dune"
ocaml boot/bootstrap.ml -j {jobs}
cp _boot/dune.exe "$ROOT/{output}"
"""

_BUILD = """mkdir "$SCRATCH/ws"
cp -rL "$ROOT/{root}/." "$SCRATCH/ws"
chmod -R u+w "$SCRATCH/ws"
cd "$SCRATCH/ws"
"$ROOT/{dune}" build --release --display=quiet -j {jobs} ./{target}
cp "_build/default/{target}" "$ROOT/{output}"
"""

def _abs(path):
    return path if path.startswith("/") else "$ROOT/" + path

def _find(files, suffix):
    for f in files:
        if f.path.endswith(suffix):
            return f
    fail("the OCaml install tree has no " + suffix)

def _env(ctx, tree):
    cc = find_cc_toolchain(ctx)
    features = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    tool = lambda action: _abs(cc_common.get_tool_for_action(feature_configuration = features, action_name = action))
    compiler = _find(tree, "/bin/ocamlopt.opt")
    return {
        "CONDA_OCAML_AR": tool(ACTION_NAMES.cpp_link_static_library),
        "CONDA_OCAML_AS": tool(ACTION_NAMES.assemble) + " -c",
        "CONDA_OCAML_CC": tool(ACTION_NAMES.c_compile),
        "CONDA_OCAML_MKDLL": tool(ACTION_NAMES.cpp_link_executable) + " -shared",
        "CONDA_OCAML_MKEXE": tool(ACTION_NAMES.cpp_link_executable),
        "PATH": _abs(compiler.dirname) + ":/usr/bin:/bin",
    }, cc.all_files

def _shell_env(env):
    return "\n".join(["export {}=\"{}\"".format(key, env[key]) for key in sorted(env)])

def _dune_build_impl(ctx):
    tree = ctx.attr._tree[DefaultInfo].files.to_list()
    env, cc_files = _env(ctx, tree)
    prelude = _PRELUDE.format(env = _shell_env(env))
    srcs = ctx.files.srcs
    marker = ctx.file.workspace
    root = marker.dirname
    dune_files = [f for f in srcs if f.path.startswith(root + "/" + DUNE_DIRECTORY + "/")]
    dune = ctx.actions.declare_file(ctx.label.name + "_dune.exe")
    inputs_common = depset(tree, transitive = [cc_files])
    ctx.actions.run_shell(
        outputs = [dune],
        inputs = depset(dune_files, transitive = [inputs_common]),
        command = prelude + _BOOTSTRAP.format(
            dune_root = root + "/" + DUNE_DIRECTORY,
            jobs = ctx.attr.jobs,
            output = dune.path,
        ),
        mnemonic = "DxDuneBootstrap",
        progress_message = "Bootstrapping dune for %{label}",
        use_default_shell_env = False,
    )
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.run_shell(
        outputs = [out],
        inputs = depset(srcs + [dune], transitive = [inputs_common]),
        command = prelude + _BUILD.format(
            root = root,
            dune = dune.path,
            jobs = ctx.attr.jobs,
            target = ctx.attr.target,
            output = out.path,
        ),
        mnemonic = "DxDuneBuild",
        progress_message = "Building %{label} with dune",
        use_default_shell_env = False,
    )
    return [DefaultInfo(
        files = depset([out]),
        executable = out,
        runfiles = ctx.runfiles(transitive_files = depset(ctx.files._stdlib)),
    )]

dx_dune_build = rule(
    implementation = _dune_build_impl,
    doc = "Builds one executable target of a vendored dune workspace with the pinned native OCaml toolchain.",
    attrs = {
        "jobs": attr.int(default = 4, doc = "Dune and bootstrap parallelism."),
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Every file of the workspace, including the dune_src directory that holds dune's own sources."),
        "target": attr.string(mandatory = True, doc = "The executable, relative to the workspace root, for example zanuda/src/main.exe."),
        "workspace": attr.label(allow_single_file = True, mandatory = True, doc = "The workspace's root dune-project file."),
        "_stdlib": attr.label(default = "@dx_ocaml_toolchains//:stdlib", cfg = "exec", allow_files = True, doc = "The OCaml standard library the built tool reads .cmi files from at run time; zanuda finds it through OCAMLLIB."),
        "_tree": attr.label(default = "@dx_ocaml_toolchains//:tree", cfg = "exec", allow_files = True),
    },
    executable = True,
    toolchains = use_cc_toolchain(),
    fragments = ["cpp"],
)
