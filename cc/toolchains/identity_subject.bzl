"""Analysis subject reporting the selected C++ toolchain."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load(":bindings.bzl", "cc_toolchain_identity", "cc_toolchain_info", "cc_toolchain_toolchains")

def _cc_toolchain_identity_subject_impl(ctx):
    info = cc_toolchain_info(ctx)
    cc_info = info.cc if hasattr(info, "cc") else info
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
            "cc.sysroot": str(cc_info.sysroot),
            "cc.syntax_only": identity.syntax_only,
            "cc.target": cc_info.target_gnu_system_name,
        }),
    ]

cc_toolchain_identity_subject = rule(
    implementation = _cc_toolchain_identity_subject_impl,
    doc = "Analysis subject reporting the selected C/C++ toolchain.",
    toolchains = cc_toolchain_toolchains(),
)
