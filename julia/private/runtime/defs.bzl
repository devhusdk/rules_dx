"""Rules over the resolved Julia toolchain for the source-built runtime."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//core/modules:versions.bzl", "JULIA_RUNTIME_VERSION")

_JULIA_TOOLCHAIN = "@rules_julia//julia:toolchain_type"
_VERSION_SETTING = "@rules_julia//julia/settings:version"

def _runtime_version_impl(_settings, _attr):
    return {_VERSION_SETTING: JULIA_RUNTIME_VERSION}

# The source-built runtime and the toolchain it links against must be the same Julia release.
_runtime_version = transition(
    implementation = _runtime_version_impl,
    inputs = [],
    outputs = [_VERSION_SETTING],
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
