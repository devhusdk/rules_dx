"""Analysis subject reporting the selected C++ toolchain."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load(":bindings.bzl", "cc_toolchain_identity", "cc_toolchain_info", "cc_toolchain_toolchains")

_EXTRA_TOOLCHAINS = "//command_line_option:extra_toolchains"

def _extra_toolchains_impl(settings, attr):
    return {
        _EXTRA_TOOLCHAINS: list(attr.extra_toolchains) + settings[_EXTRA_TOOLCHAINS],
        "//cc/toolchains:compiler": attr.compiler or settings["//cc/toolchains:compiler"],
    }

_transition = transition(
    implementation = _extra_toolchains_impl,
    inputs = [_EXTRA_TOOLCHAINS, "//cc/toolchains:compiler"],
    outputs = [_EXTRA_TOOLCHAINS, "//cc/toolchains:compiler"],
)

def _cc_toolchain_identity_subject_impl(ctx):
    cc_info = cc_toolchain_info(ctx)
    identity = cc_toolchain_identity(ctx)
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "cc.builtin_includes": " | ".join(sorted([str(p) for p in cc_info.built_in_include_directories])),
            "cc.abi": identity.abi,
            "cc.compiler": cc_info.compiler,
            "cc.coverage": identity.coverage,
            "cc.driver": identity.driver,
            "cc.family": identity.family,
            "cc.libc": cc_info.libc,
            "cc.sysroot": str(cc_info.sysroot),
            "cc.syntax_only": identity.syntax_only,
            "cc.target": cc_info.target_gnu_system_name,
        }),
    ]

cc_toolchain_identity_subject = rule(
    implementation = _cc_toolchain_identity_subject_impl,
    doc = "Analysis subject reporting the selected C/C++ toolchain.",
    attrs = {
        "compiler": attr.string(doc = "Compiler flag value to build with."),
        "extra_toolchains": attr.string_list(doc = "Toolchain labels to register ahead of the defaults."),
    },
    cfg = _transition,
    toolchains = cc_toolchain_toolchains(),
)
