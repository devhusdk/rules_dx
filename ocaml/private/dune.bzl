"""Runs dune inside Bazel actions with the pinned native OCaml toolchain.

`dx_dune_bootstrap` builds dune from its own sources once; `dx_dune_build`
builds one executable of a dune workspace and `dx_dune_library` builds one
library package as OCaml providers that `ocaml_module` and `ocaml_test` take in
`deps`. The actions use only the pinned conda OCaml tree, the resolved C
toolchain, and /usr/bin and /bin as the host baseline; dune's cache and the
network are off.
"""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_ocaml//build:providers.bzl", "OCamlArchiveProvider", "OCamlCcInfo", "OCamlDepsProvider", "OCamlImportProvider")
load("//core:wrapper.bzl", "dx_language_kwargs")
load("//ocaml/private/toolchain:ocaml.bzl", "native_cc_env")
load(":defs.bzl", "OCAML_COMPATIBLE_WITH")

DEPSET_ORDER = "postorder"

_PRELUDE = """set -euo pipefail
ROOT="$PWD"
SCRATCH="$(mktemp -d)"
trap 'chmod -R u+w "$SCRATCH" 2>/dev/null || true; rm -rf "$SCRATCH"' EXIT
{env}
if [ "$(uname -s)" = Darwin ]; then export SDKROOT="$(/usr/bin/xcrun --show-sdk-path)"; fi
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
# The conda zstd dylib is loaded through an rpath the executables lack, so link libzstd.a from the zstd-static pin instead.
# ld prefers a dylib found in any -L directory, so search the directories in order and put ours first.
if [ "$(uname -s)" = Darwin ] && [ -f "${{PATH%%:*}}/../lib/libzstd.a" ]; then
  mkdir -p "$SCRATCH/static"
  ln -s "$(cd "${{PATH%%:*}}/../lib" && pwd)/libzstd.a" "$SCRATCH/static/libzstd.a"
  export CONDA_OCAML_MKEXE="$CONDA_OCAML_MKEXE -Wl,-search_paths_first -L$SCRATCH/static"
fi
# Fallback for dylib links: executables built and run inside one action find libzstd through the tree's lib directory.
if [ "$(uname -s)" = Darwin ]; then
  export CONDA_OCAML_MKEXE="$CONDA_OCAML_MKEXE -Wl,-rpath,$(cd "${{PATH%%:*}}/../lib" && pwd)"
fi
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

_LIBRARY = """mkdir "$SCRATCH/ws"
cp -rL "$ROOT/{root}/." "$SCRATCH/ws"
chmod -R u+w "$SCRATCH/ws"
cd "$SCRATCH/ws"
{ocamlpath}
if [ -f dune ] && grep -q '^(vendored_dirs' dune; then
  sed -i '/^(vendored_dirs/d' dune
  "$ROOT/{dune}" build --display=quiet -j {jobs} @{package}/install
else
  "$ROOT/{dune}" build --display=quiet -j {jobs} -p {package} @install
fi
LIB="_build/install/default/lib/{package}"
STUBS="$LIB/lib{library}_stubs.a"
if [ -f "$STUBS" ] && [ -z "{stubs}" ]; then
  echo "dx_dune_library: $STUBS holds C stubs; set stubs = True to link them" >&2
  exit 1
fi
if [ -n "{stubs}" ]; then
  if [ ! -f "$STUBS" ]; then
    echo "dx_dune_library: stubs = True but $STUBS was not built" >&2
    exit 1
  fi
  cp "$STUBS" "$ROOT/{stubs}"
fi
for archive in {archives}; do
  if [ ! -f "$LIB/$archive" ]; then
    echo "dx_dune_library: $LIB/$archive was not built; set library = to the dune (name) of the library" >&2
    exit 1
  fi
done
cp -RL "$LIB/." "$ROOT/{tree}"
for archive in {archives}; do
  cp "$LIB/$archive" "$ROOT/{archive_dir}/$archive"
done
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
    link_env = {
        key: _abs(value) if "/" in value and not value.startswith("-") else value
        for key, value in cc_common.get_environment_variables(
            feature_configuration = features,
            action_name = ACTION_NAMES.cpp_link_executable,
            variables = cc_common.empty_variables(),
        ).items()
    }
    env = native_cc_env(
        cc = tool(ACTION_NAMES.c_compile),
        assembler = tool(ACTION_NAMES.assemble),
        linker = tool(ACTION_NAMES.cpp_link_executable),
        archiver = tool(ACTION_NAMES.cpp_link_static_library),
        dist_bin = _abs(compiler.dirname),
        macos = ctx.target_platform_has_constraint(ctx.attr._macos[platform_common.ConstraintValueInfo]),
        ar_wrapper = _abs(ctx.file._libtool_ar.path),
        link_env = link_env,
    )
    return env, cc.all_files

def _shell_env(env):
    return "\n".join(["export {}=\"{}\"".format(key, env[key]) for key in sorted(env)])

def _context(ctx):
    tree = ctx.attr._tree[DefaultInfo].files.to_list()
    env, cc_files = _env(ctx, tree)
    return _PRELUDE.format(env = _shell_env(env)), depset(tree + [ctx.file._libtool_ar], transitive = [cc_files])

def _dune_bootstrap_impl(ctx):
    prelude, inputs = _context(ctx)
    scripts = [f for f in ctx.files.srcs if f.path.endswith("/boot/bootstrap.ml")]
    if len(scripts) != 1:
        fail("srcs must hold exactly one boot/bootstrap.ml, found {}".format(len(scripts)))
    out = ctx.actions.declare_file(ctx.label.name + ".exe")
    # run_shell: The script exports the toolchain environment and runs several commands in one action.
    ctx.actions.run_shell(
        outputs = [out],
        inputs = depset(ctx.files.srcs, transitive = [inputs]),
        command = prelude + _BOOTSTRAP.format(
            dune_root = scripts[0].dirname.removesuffix("/boot"),
            jobs = ctx.attr.jobs,
            output = out.path,
        ),
        mnemonic = "DxDuneBootstrap",
        progress_message = "Bootstrapping dune %{label}",
        use_default_shell_env = False,
    )
    return [DefaultInfo(files = depset([out]), executable = out)]

dx_dune_bootstrap = rule(
    implementation = _dune_bootstrap_impl,
    doc = "Builds the dune executable from dune's own sources with its bootstrap script.",
    attrs = {
        "jobs": attr.int(default = 4, doc = "Bootstrap parallelism."),
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Dune's source tree, including boot/bootstrap.ml."),
        "_libtool_ar": attr.label(default = "//ocaml/private/toolchain:libtool_ar.sh", allow_single_file = True),
        "_macos": attr.label(default = "@platforms//os:macos"),
        "_tree": attr.label(default = "@dx_ocaml_toolchains//:tree", cfg = "exec", allow_files = True),
    },
    executable = True,
    toolchains = use_cc_toolchain(),
    fragments = ["cpp"],
)

def _dune_build_impl(ctx):
    prelude, inputs = _context(ctx)
    dune = ctx.executable.dune
    out = ctx.actions.declare_file(ctx.label.name)
    # run_shell: The script exports the toolchain environment and runs several commands in one action.
    ctx.actions.run_shell(
        outputs = [out],
        inputs = depset(ctx.files.srcs + [dune], transitive = [inputs]),
        command = prelude + _BUILD.format(
            root = ctx.file.workspace.dirname,
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
        "dune": attr.label(mandatory = True, executable = True, cfg = "exec", doc = "The dune executable, for example a dx_dune_bootstrap target."),
        "jobs": attr.int(default = 4, doc = "Dune parallelism."),
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Every file of the workspace."),
        "target": attr.string(mandatory = True, doc = "The executable, relative to the workspace root, for example zanuda/src/main.exe."),
        "workspace": attr.label(allow_single_file = True, mandatory = True, doc = "The workspace's root dune-project file."),
        "_stdlib": attr.label(default = "@dx_ocaml_toolchains//:stdlib", cfg = "exec", allow_files = True, doc = "The OCaml standard library the built tool reads .cmi files from at run time; zanuda finds it through OCAMLLIB."),
        "_libtool_ar": attr.label(default = "//ocaml/private/toolchain:libtool_ar.sh", allow_single_file = True),
        "_macos": attr.label(default = "@platforms//os:macos"),
        "_tree": attr.label(default = "@dx_ocaml_toolchains//:tree", cfg = "exec", allow_files = True),
    },
    executable = True,
    toolchains = use_cc_toolchain(),
    fragments = ["cpp"],
)

def _dune_library_impl(ctx):
    prelude, inputs = _context(ctx)
    dune = ctx.executable.dune
    package = ctx.attr.package
    library = ctx.attr.library or package
    native = ctx.attr._emitter[BuildSettingInfo].value == "sys"
    names = [library + ".cmxa", library + ".a"] if native else [library + ".cma"]
    tree = ctx.actions.declare_directory(ctx.label.name + "_ocamlpath/" + package)
    dep_info = [dep[OCamlDepsProvider] for dep in ctx.attr.deps]
    dep_trees = depset(transitive = [d.sigs for d in dep_info] + [d.archives for d in dep_info] + [d.afiles for d in dep_info])
    dep_dirs = {}
    for path in depset(transitive = [d.paths for d in dep_info]).to_list():
        dep_dirs[_abs(path.rpartition("/")[0])] = True
    ocamlpath = 'export OCAMLPATH="{}"'.format(":".join(sorted(dep_dirs))) if dep_dirs else ""
    stubs = ctx.actions.declare_file("{}_archive/lib{}_stubs.a".format(ctx.label.name, library)) if ctx.attr.stubs else None
    archives = [ctx.actions.declare_file("{}_archive/{}".format(ctx.label.name, n)) for n in names]
    # run_shell: The script exports the toolchain environment and runs several commands in one action.
    ctx.actions.run_shell(
        outputs = [tree] + archives + ([stubs] if stubs else []),
        inputs = depset(ctx.files.srcs + [dune], transitive = [inputs, dep_trees]),
        command = prelude + _LIBRARY.format(
            library = library,
            ocamlpath = ocamlpath,
            stubs = stubs.path if stubs else "",
            root = ctx.file.workspace.dirname,
            dune = dune.path,
            jobs = ctx.attr.jobs,
            package = package,
            archives = " ".join(names),
            tree = tree.path,
            archive_dir = archives[0].dirname,
        ),
        mnemonic = "DxDuneLibrary",
        progress_message = "Building %{label} with dune",
        use_default_shell_env = False,
    )
    archive = archives[0]
    afiles = archives[1:]
    deps = [dep[OCamlDepsProvider] for dep in ctx.attr.deps if OCamlDepsProvider in dep]
    merge = lambda field, direct: depset(direct, transitive = [getattr(d, field) for d in deps], order = DEPSET_ORDER)
    empty = lambda field: merge(field, [])
    cc_infos = [dep[CcInfo] for dep in ctx.attr.deps if CcInfo in dep]
    if stubs:
        cc_infos.append(CcInfo(linking_context = cc_common.create_linking_context(linker_inputs = depset([
            cc_common.create_linker_input(
                owner = ctx.label,
                libraries = depset([cc_common.create_library_to_link(
                    actions = ctx.actions,
                    static_library = stubs,
                    alwayslink = True,
                )]),
            ),
        ]))))
    cc_info = cc_common.merge_cc_infos(cc_infos = cc_infos)
    return [
        DefaultInfo(files = depset([tree] + archives + ([stubs] if stubs else []))),
        OCamlDepsProvider(
            sigs = merge("sigs", [tree]),
            cli_link_deps = merge("cli_link_deps", [archive]),
            link_archives_deps = merge("link_archives_deps", [archive]),
            archives = merge("archives", [archive]),
            structs = empty("structs"),
            astructs = empty("astructs"),
            afiles = merge("afiles", afiles),
            ofiles = empty("ofiles"),
            cmxs = empty("cmxs"),
            cmts = empty("cmts"),
            cmtis = empty("cmtis"),
            paths = merge("paths", [tree.path]),
        ),
        OCamlArchiveProvider(archive = archive),
        OCamlImportProvider(name = ctx.label.name),
        cc_info,
        OCamlCcInfo(direct = cc_info, archived = cc_info),
        OutputGroupInfo(archive = depset([archive])),
    ]

_dx_dune_library = rule(
    implementation = _dune_library_impl,
    doc = "Builds one library package of a dune workspace and exposes it to rules_ocaml deps.",
    attrs = {
        "deps": attr.label_list(providers = [OCamlDepsProvider], doc = "Other OCaml libraries the package needs."),
        "dune": attr.label(mandatory = True, executable = True, cfg = "exec", doc = "The dune executable, for example a dx_dune_bootstrap target."),
        "jobs": attr.int(default = 4, doc = "Dune parallelism."),
        "library": attr.string(doc = "The dune (name) of the library whose archive is linked; defaults to the package."),
        "package": attr.string(mandatory = True, doc = "The opam package, built with dune build -p."),
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Every file of the workspace."),
        "stubs": attr.bool(doc = "Link lib<library>_stubs.a, the package's C stubs, into native links."),
        "workspace": attr.label(allow_single_file = True, mandatory = True, doc = "The workspace's root dune-project file."),
        "_emitter": attr.label(default = "//ocaml/private/toolchain:emitter"),
        "_libtool_ar": attr.label(default = "//ocaml/private/toolchain:libtool_ar.sh", allow_single_file = True),
        "_macos": attr.label(default = "@platforms//os:macos"),
        "_tree": attr.label(default = "@dx_ocaml_toolchains//:tree", cfg = "exec", allow_files = True),
    },
    toolchains = use_cc_toolchain(),
    fragments = ["cpp"],
)

def dx_dune_library(name, **kwargs):
    """Builds a dune library package; the OCaml cells are always appended to target_compatible_with."""
    _dx_dune_library(name = name, **dx_language_kwargs(kwargs, [], OCAML_COMPATIBLE_WITH))
