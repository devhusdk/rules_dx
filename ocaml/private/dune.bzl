"""Runs dune inside Bazel actions with the pinned native OCaml toolchain.

`dx_dune_bootstrap` builds dune from its own sources once; `dx_dune_build`
builds one executable of a dune workspace and `dx_dune_library` builds one
library package as OCaml providers that `ocaml_module` and `ocaml_test` take in
`deps`. The actions use only the pinned conda OCaml tree, the resolved C
toolchain and the pinned coreutils; dune's cache and the
network are off.
"""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_ocaml//build:providers.bzl", "OCamlArchiveProvider", "OCamlCcInfo", "OCamlDepsProvider", "OCamlImportProvider")
load("//core:wrapper.bzl", "dx_language_kwargs")
load("//core/coreutils:defs.bzl", "COREUTILS_TOOLCHAIN_TYPE", "coreutils")
load("//ocaml/private/toolchain:ocaml.bzl", "cc_drivers", "native_cc_env")
load(":defs.bzl", "OCAML_COMPATIBLE_WITH")

DEPSET_ORDER = "postorder"

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
    drivers = cc_drivers(ctx)
    macos = ctx.target_platform_has_constraint(ctx.attr._macos[platform_common.ConstraintValueInfo])
    applets = coreutils(ctx)
    env = native_cc_env(
        cc = tool(ACTION_NAMES.c_compile),
        assembler = tool(ACTION_NAMES.assemble),
        linker = tool(ACTION_NAMES.cpp_link_executable),
        archiver = tool(ACTION_NAMES.cpp_link_static_library),
        path = ":".join([_abs(drivers[0].dirname), _abs(compiler.dirname), _abs(applets.path)]),
        macos = macos,
        link_env = link_env,
    )
    env["DX_OCAML_AS_DROP_DEBUG_PREFIX_MAP"] = "1"
    if not macos:
        env["CONDA_OCAML_MKEXE"] += " -fuse-ld=lld"
        env["CONDA_OCAML_MKDLL"] += " -fuse-ld=lld"
    return struct(
        env = env,
        inputs = depset(tree + drivers + [ctx.executable._cc_driver], transitive = [cc.all_files, applets.files]),
        ocaml_bin = compiler.dirname,
    )

def _context(ctx):
    tree = ctx.attr._tree[DefaultInfo].files.to_list()
    return _env(ctx, tree)

def _args(ctx, context, mode):
    args = ctx.actions.args()
    args.add_all(["dune", mode, "--jobs", str(ctx.attr.jobs), "--ocaml-bin", context.ocaml_bin])
    return args

def _dune_bootstrap_impl(ctx):
    context = _context(ctx)
    scripts = [f for f in ctx.files.srcs if f.path.endswith("/boot/bootstrap.ml")]
    if len(scripts) != 1:
        fail("srcs must hold exactly one boot/bootstrap.ml, found {}".format(len(scripts)))
    out = ctx.actions.declare_file(ctx.label.name + ".exe")
    args = _args(ctx, context, "bootstrap")
    args.add_all(["--source", scripts[0].dirname.removesuffix("/boot"), "--output", out])
    ctx.actions.run(
        executable = ctx.executable._cc_driver,
        arguments = [args],
        env = context.env,
        outputs = [out],
        inputs = depset(ctx.files.srcs, transitive = [context.inputs]),
        mnemonic = "DxDuneBootstrap",
        progress_message = "Bootstrapping dune %{label}",
    )
    return [DefaultInfo(files = depset([out]), executable = out)]

dx_dune_bootstrap = rule(
    implementation = _dune_bootstrap_impl,
    doc = "Builds the dune executable from dune's own sources with its bootstrap script.",
    attrs = {
        "jobs": attr.int(default = 4, doc = "Bootstrap parallelism."),
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Dune's source tree, including boot/bootstrap.ml."),
        "_cc_driver": attr.label(default = Label("//cli/ocaml_cc_driver"), executable = True, cfg = "exec"),
        "_macos": attr.label(default = "@platforms//os:macos"),
        "_tree": attr.label(default = "@dx_ocaml_toolchains//:tree", cfg = "exec", allow_files = True),
    },
    executable = True,
    toolchains = use_cc_toolchain() + [COREUTILS_TOOLCHAIN_TYPE],
    fragments = ["cpp"],
)

def _dune_build_impl(ctx):
    context = _context(ctx)
    dune = ctx.executable.dune
    out = ctx.actions.declare_file(ctx.label.name)
    args = _args(ctx, context, "build")
    args.add_all(["--workspace", ctx.file.workspace.dirname, "--dune", dune, "--target", ctx.attr.target, "--output", out])
    ctx.actions.run(
        executable = ctx.executable._cc_driver,
        arguments = [args],
        env = context.env,
        outputs = [out],
        inputs = depset(ctx.files.srcs + [dune], transitive = [context.inputs]),
        mnemonic = "DxDuneBuild",
        progress_message = "Building %{label} with dune",
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
        "_cc_driver": attr.label(default = Label("//cli/ocaml_cc_driver"), executable = True, cfg = "exec"),
        "_macos": attr.label(default = "@platforms//os:macos"),
        "_tree": attr.label(default = "@dx_ocaml_toolchains//:tree", cfg = "exec", allow_files = True),
    },
    executable = True,
    toolchains = use_cc_toolchain() + [COREUTILS_TOOLCHAIN_TYPE],
    fragments = ["cpp"],
)

def _dune_library_impl(ctx):
    context = _context(ctx)
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
    env = dict(context.env)
    if dep_dirs:
        env["OCAMLPATH"] = ":".join(sorted(dep_dirs))
    stubs = ctx.actions.declare_file("{}_archive/lib{}_stubs.a".format(ctx.label.name, library)) if ctx.attr.stubs else None
    archives = [ctx.actions.declare_file("{}_archive/{}".format(ctx.label.name, n)) for n in names]
    args = _args(ctx, context, "library")
    args.add_all(["--workspace", ctx.file.workspace.dirname, "--dune", dune, "--package", package, "--stubs-name", "lib{}_stubs.a".format(library), "--tree", tree.path])
    if stubs:
        args.add_all(["--stubs", stubs])
    for name, archive in zip(names, archives):
        args.add("--archive")
        args.add(name + "=" + archive.path)
    ctx.actions.run(
        executable = ctx.executable._cc_driver,
        arguments = [args],
        env = env,
        outputs = [tree] + archives + ([stubs] if stubs else []),
        inputs = depset(ctx.files.srcs + [dune], transitive = [context.inputs, dep_trees]),
        mnemonic = "DxDuneLibrary",
        progress_message = "Building %{label} with dune",
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
        "_cc_driver": attr.label(default = Label("//cli/ocaml_cc_driver"), executable = True, cfg = "exec"),
        "_macos": attr.label(default = "@platforms//os:macos"),
        "_tree": attr.label(default = "@dx_ocaml_toolchains//:tree", cfg = "exec", allow_files = True),
    },
    toolchains = use_cc_toolchain() + [COREUTILS_TOOLCHAIN_TYPE],
    fragments = ["cpp"],
)

def dx_dune_library(name, **kwargs):
    """Builds a dune library package; the OCaml cells are always appended to target_compatible_with."""
    _dx_dune_library(name = name, **dx_language_kwargs(kwargs, [], OCAML_COMPATIBLE_WITH))
