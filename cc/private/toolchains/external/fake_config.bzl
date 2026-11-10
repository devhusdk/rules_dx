"""Configuration of a fake rules_cc toolchain that never compiles."""

load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "tool_path")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

def _impl(ctx):
    return cc_common.create_cc_toolchain_config_info(
        ctx = ctx,
        toolchain_identifier = "fake-" + ctx.attr.compiler,
        host_system_name = "local",
        target_system_name = ctx.attr.target,
        target_cpu = "fake",
        target_libc = "fake",
        compiler = ctx.attr.compiler,
        abi_version = "fake",
        abi_libc_version = "fake",
        tool_paths = [tool_path(name = name, path = ctx.attr.tool) for name in ["gcc", "ld", "ar", "cpp", "gcov", "nm", "objdump", "strip"]],
    )

fake_cc_toolchain_config = rule(
    implementation = _impl,
    doc = "Toolchain configuration reporting the given compiler and target.",
    attrs = {
        "compiler": attr.string(doc = "Compiler string to report.", mandatory = True),
        "target": attr.string(doc = "Target system name to report.", mandatory = True),
        "tool": attr.string(doc = "Path reported for every tool.", default = "/bin/false"),
    },
)
