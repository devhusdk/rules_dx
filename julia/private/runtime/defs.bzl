"""Rules over the resolved Julia toolchain for the source-built runtime."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

_JULIA_TOOLCHAIN = "@rules_julia//julia:toolchain_type"

def _julia_runtime_files_impl(ctx):
    toolchain = ctx.toolchains[_JULIA_TOOLCHAIN]
    return [DefaultInfo(
        files = depset([toolchain.julia]),
        runfiles = ctx.runfiles(files = [toolchain.julia], transitive_files = toolchain.all_files),
    )]

julia_runtime_files = rule(
    implementation = _julia_runtime_files_impl,
    doc = "The resolved Julia toolchain's julia executable, with every file of its tree in runfiles.",
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
    doc = "Links against shared libraries of the resolved Julia toolchain, found by file name.",
    attrs = {
        "libraries": attr.string_list(doc = "File names of the shared libraries.", mandatory = True),
    },
    fragments = ["cpp"],
    toolchains = use_cc_toolchain() + [_JULIA_TOOLCHAIN],
)
