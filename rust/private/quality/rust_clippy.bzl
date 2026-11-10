"""Clippy for dx lint: reports unused crate dependencies without changing the build configuration.

The attrs follow rust_clippy_aspect in rules_rust 0.75.0 (rust/private/clippy.bzl); re-check them on every rules_rust upgrade.
"""

load("@rules_cc//cc:find_cc_toolchain.bzl", "use_cc_toolchain")
load("@rules_rust//rust:defs.bzl", "rust_clippy_action", "rust_common")
load("@rules_rust//rust/private:clippy.bzl", "ClippyFlagsInfo")
load("@rules_rust//rust/private:rustc.bzl", "get_error_format")
load("@rules_rust//rust/settings:incompatible.bzl", "IncompatibleFlagInfo")
load("//rust/private/toolchains:bindings.bzl", "RUST_TOOLCHAIN_TYPE")

DxClippyInfo = provider(
    doc = "The JSON clippy diagnostics dx lint reads for a crate.",
    fields = {"diagnostics": "list of File"},
)

UNUSED_CRATE_DEPENDENCIES = "--force-warn=unused_crate_dependencies"

def _dx_clippy_aspect_impl(target, ctx):
    if OutputGroupInfo in target and hasattr(target[OutputGroupInfo], "clippy_checks"):
        return [DxClippyInfo(diagnostics = [])]
    crate_info = rust_clippy_action.get_clippy_ready_crate_info(target, ctx)
    if not crate_info:
        return [DxClippyInfo(diagnostics = [])]

    diagnostics = ctx.actions.declare_file(ctx.label.name + ".dx_clippy.diagnostics", sibling = crate_info.output)
    use_clippy_error_format = ctx.attr._incompatible_change_clippy_error_format[IncompatibleFlagInfo].enabled
    error_format = get_error_format(ctx.attr, "_clippy_error_format" if use_clippy_error_format else "_error_format")
    flags = ctx.attr._clippy_flags[ClippyFlagsInfo].clippy_flags + ctx.attr._clippy_flag[ClippyFlagsInfo].clippy_flags
    rust_clippy_action.action(
        ctx = ctx,
        clippy_executable = ctx.toolchains[RUST_TOOLCHAIN_TYPE].clippy_driver,
        process_wrapper = ctx.executable._process_wrapper,
        crate_info = crate_info,
        config = ctx.file._config,
        cap_at_warnings = True,
        extra_clippy_flags = flags + [UNUSED_CRATE_DEPENDENCIES],
        error_format = error_format,
        clippy_diagnostics_file = diagnostics,
    )
    return [DxClippyInfo(diagnostics = [diagnostics])]

dx_clippy_aspect = aspect(
    implementation = _dx_clippy_aspect_impl,
    doc = "Runs clippy with unused_crate_dependencies forced to a warning and keeps its JSON diagnostics.",
    fragments = ["cpp"],
    attrs = {
        "_clippy_error_format": attr.label(default = "@rules_rust//rust/settings:clippy_error_format"),
        "_clippy_flag": attr.label(default = "@rules_rust//rust/settings:clippy_flag"),
        "_clippy_flags": attr.label(default = "@rules_rust//rust/settings:clippy_flags"),
        "_config": attr.label(allow_single_file = True, default = "@rules_rust//rust/settings:clippy.toml"),
        "_error_format": attr.label(default = "@rules_rust//rust/settings:error_format"),
        "_extra_rustc_flag": attr.label(default = "@rules_rust//rust/settings:extra_rustc_flag"),
        "_incompatible_change_clippy_error_format": attr.label(default = "@rules_rust//rust/settings:incompatible_change_clippy_error_format"),
        "_per_crate_rustc_flag": attr.label(default = "@rules_rust//rust/settings:per_crate_rustc_flag"),
        "_process_wrapper": attr.label(default = "@rules_rust//util/process_wrapper", executable = True, cfg = "exec"),
    },
    provides = [DxClippyInfo],
    required_providers = [
        [rust_common.crate_info],
        [rust_common.test_crate_info],
    ],
    toolchains = [RUST_TOOLCHAIN_TYPE] + use_cc_toolchain(mandatory = False),
)
