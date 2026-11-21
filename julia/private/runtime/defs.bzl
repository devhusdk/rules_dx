"""Rules over the resolved Julia toolchain for the source-built runtime."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//core/coreutils:defs.bzl", "COREUTILS_TOOLCHAIN_TYPE", "coreutils_copy")
load("//core/modules:versions.bzl", "JULIA_RUNTIME_VERSION")

_JULIA_TOOLCHAIN = "@rules_julia//julia:toolchain_type"
_VERSION_SETTING = "@rules_julia//julia/settings:version"
_STOCK_SETTING = str(Label("//julia/private/runtime:stock"))

_LIBRARY = "libjulia-internal.so." + JULIA_RUNTIME_VERSION

def source_exec_compatible_with(downloaded, source_constraints):
    """Returns the exec constraints of the source toolchain: none when @dx_prebuilt downloads julia_internal for linux_x86_64, else source_constraints."""
    if "linux_x86_64" in downloaded.get("julia_internal", []):
        return []
    return source_constraints

def _runtime_version_impl(_settings, _attr):
    return {_STOCK_SETTING: True, _VERSION_SETTING: JULIA_RUNTIME_VERSION}

# The source-built runtime and the toolchain it links against must be the same Julia release.
# The stock flag keeps the source toolchain out of resolution here, so these rules see the
# downloaded release instead of themselves.
_runtime_version = transition(
    implementation = _runtime_version_impl,
    inputs = [],
    outputs = [_STOCK_SETTING, _VERSION_SETTING],
)

def _julia_runtime_files_impl(ctx):
    toolchain = ctx.toolchains[_JULIA_TOOLCHAIN]
    return [DefaultInfo(
        files = depset([toolchain.julia]),
        runfiles = ctx.runfiles(files = [toolchain.julia], transitive_files = toolchain.all_files),
    )]

julia_runtime_files = rule(
    implementation = _julia_runtime_files_impl,
    doc = "The julia executable of the runtime's Julia toolchain, with every file of its tree in runfiles.",
    cfg = _runtime_version,
    toolchains = [_JULIA_TOOLCHAIN],
)

def _julia_shipped_libs_impl(ctx):
    wanted = {name: None for name in ctx.attr.libraries}
    found = {}
    for f in ctx.toolchains[_JULIA_TOOLCHAIN].all_files.to_list():
        if f.basename in wanted and f.basename not in found:
            found[f.basename] = f
    missing = [name for name in ctx.attr.libraries if name not in found]
    if missing:
        fail("the Julia toolchain ships no {}".format(", ".join(missing)))
    cc_toolchain = find_cc_toolchain(ctx)
    feature_configuration = cc_common.configure_features(
        ctx = ctx,
        cc_toolchain = cc_toolchain,
        requested_features = ctx.features,
        unsupported_features = ctx.disabled_features,
    )
    libraries = [
        cc_common.create_library_to_link(
            actions = ctx.actions,
            cc_toolchain = cc_toolchain,
            dynamic_library = found[name],
            feature_configuration = feature_configuration,
        )
        for name in ctx.attr.libraries
    ]
    linker_input = cc_common.create_linker_input(
        owner = ctx.label,
        libraries = depset(libraries),
    )
    return [CcInfo(linking_context = cc_common.create_linking_context(
        linker_inputs = depset([linker_input]),
    ))]

julia_shipped_libs = rule(
    implementation = _julia_shipped_libs_impl,
    doc = "Links against shared libraries of the runtime's Julia toolchain, found by file name.",
    cfg = _runtime_version,
    attrs = {
        "libraries": attr.string_list(doc = "File names of the shared libraries.", mandatory = True),
    },
    fragments = ["cpp"],
    toolchains = use_cc_toolchain() + [_JULIA_TOOLCHAIN],
)

def _rlocationpath(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[len("../"):]
    return ctx.workspace_name + "/" + file.short_path

def _julia_source_toolchain_impl(ctx):
    stock = ctx.toolchains[_JULIA_TOOLCHAIN]
    # The source build carries the versioned file among others; the hub file and the
    # guard stub are the one file of their target.
    built = [f for f in ctx.files.library if f.basename == _LIBRARY] or ctx.files.library
    if len(built) != 1:
        fail("{} holds no {} and is not one file".format(ctx.attr.library.label, _LIBRARY))
    root = stock.julia.dirname.rpartition("/")[0] + "/"
    alias_dir = "lib/julia/"
    internal = alias_dir + _LIBRARY
    aliases = [alias_dir + "libjulia-internal.so", internal.rpartition(".")[0]]
    tree = {}
    links = []
    for f in stock.all_files.to_list():
        if not f.path.startswith(root):
            fail("{} lies outside the Julia tree {}".format(f.path, root))
        rel = f.path[len(root):]
        if rel in tree:
            continue
        out = ctx.actions.declare_file(ctx.label.name + "/" + rel)
        tree[rel] = out

        # The dynamic loader resolves $ORIGIN and Julia finds libjulia-internal next to
        # the real path of julia and libjulia, so those must be files in this tree.
        if rel == internal:
            coreutils_copy(ctx, built[0], out, "JuliaRuntimeCopy")
        elif rel.startswith("bin/") or rel.startswith("lib/libjulia.so"):
            coreutils_copy(ctx, f, out, "JuliaRuntimeCopy")
        elif rel in aliases:
            links.append(rel)
        else:
            ctx.actions.symlink(output = out, target_file = f)
    if internal not in tree:
        fail("the Julia toolchain ships no " + internal)
    for rel in links:
        ctx.actions.symlink(output = tree[rel], target_file = tree[internal])

    julia = tree[stock.julia.path[len(root):]]
    make_variable_info = platform_common.TemplateVariableInfo({
        "JULIA": julia.path,
        "JULIA_RLOCATIONPATH": _rlocationpath(ctx, julia),
    })
    fields = {
        name: getattr(stock, name)
        for name in dir(stock)
        if type(getattr(stock, name)) != "builtin_function_or_method"
    }
    fields.update(
        all_files = depset(tree.values()),
        julia = julia,
        link_files = depset([tree[f.path[len(root):]] for f in stock.link_files.to_list()]),
        make_variable_info = make_variable_info,
        sysimage = tree[stock.sysimage.path[len(root):]] if stock.sysimage else None,
    )
    return [
        platform_common.ToolchainInfo(**fields),
        make_variable_info,
    ]

julia_source_toolchain = rule(
    implementation = _julia_source_toolchain_impl,
    doc = "The runtime's Julia toolchain with the source-built libjulia-internal in its tree.",
    cfg = _runtime_version,
    attrs = {
        "library": attr.label(doc = "The libjulia-internal: the source build, its release prebuilt or the guard stub.", mandatory = True, allow_files = True),
    },
    toolchains = [_JULIA_TOOLCHAIN, COREUTILS_TOOLCHAIN_TYPE],
)
