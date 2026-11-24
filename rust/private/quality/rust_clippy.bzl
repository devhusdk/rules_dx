"""Clippy for dx lint that reports unused crate dependencies."""

load("@bazel_skylib//lib:structs.bzl", "structs")
load("@rules_cc//cc:find_cc_toolchain.bzl", "use_cc_toolchain")
load("@rules_rust//rust:defs.bzl", "rust_clippy_action", "rust_common")
load("@rules_rust//rust/private:clippy.bzl", "ClippyFlagsInfo")
load("@rules_rust//rust/private:rustc.bzl", "get_error_format")
load("@rules_rust//rust/settings:incompatible.bzl", "IncompatibleFlagInfo")
load("//core/quality:sources.bzl", "forwarded_upstream")
load("//rust/private/toolchain:bindings.bzl", "RUST_TOOLCHAIN_TYPE")

DxClippyInfo = provider(
    doc = "The JSON clippy diagnostics dx lint reads for a crate.",
    fields = {"diagnostics": "list of File"},
)

UNUSED_CRATE_DEPENDENCIES = "--force-warn=unused_crate_dependencies"

TEST_EXEMPT_LINTS = [
    "clippy::exit",
    "clippy::expect_used",
    "clippy::get_unwrap",
    "clippy::indexing_slicing",
    "clippy::panic",
    "clippy::panic_in_result_fn",
    "clippy::string_slice",
    "clippy::unreachable",
    "clippy::unwrap_in_result",
    "clippy::unwrap_used",
]

def _flag_lint(flag):
    for prefix in ["--deny=", "--warn=", "--forbid=", "--force-warn=", "-D", "-W", "-F"]:
        if flag.startswith(prefix):
            return flag[len(prefix):].replace("-", "_")
    return None

def test_crate_flags(flags):
    """Returns flags without those naming a lint in TEST_EXEMPT_LINTS."""
    return [flag for flag in flags if _flag_lint(flag) not in TEST_EXEMPT_LINTS]

def _package_clippy_config(crate_info, label, default):
    """Returns the clippy.toml a non-test crate lists in compile_data from its own package, else default."""
    if crate_info.is_test:
        return default
    for f in crate_info.compile_data.to_list():
        if f.basename == "clippy.toml" and f.is_source and f.owner.workspace_name == label.workspace_name and f.owner.package == label.package:
            return f
    return default

def _dx_clippy_aspect_impl(target, ctx):
    # A forwarder's crate lives in the upstream's configuration, and rustc writes beside it.
    upstream = forwarded_upstream(ctx.rule.attr, DxClippyInfo)
    if upstream != None:
        return [upstream[DxClippyInfo]]
    if OutputGroupInfo in target and hasattr(target[OutputGroupInfo], "clippy_checks"):
        return [DxClippyInfo(diagnostics = [])]
    crate_info = rust_clippy_action.get_clippy_ready_crate_info(target, ctx)
    if not crate_info:
        return [DxClippyInfo(diagnostics = [])]

    diagnostics = ctx.actions.declare_file(ctx.label.name + ".dx_clippy.diagnostics", sibling = crate_info.output)

    fields = structs.to_dict(crate_info)
    fields["output"] = diagnostics
    crate_info = rust_common.create_crate_info(**fields)
    use_clippy_error_format = ctx.attr._incompatible_change_clippy_error_format[IncompatibleFlagInfo].enabled
    error_format = get_error_format(ctx.attr, "_clippy_error_format" if use_clippy_error_format else "_error_format")
    flags = ctx.attr._clippy_flags[ClippyFlagsInfo].clippy_flags + ctx.attr._clippy_flag[ClippyFlagsInfo].clippy_flags
    if crate_info.is_test:
        flags = test_crate_flags(flags)
    rust_clippy_action.action(
        ctx = ctx,
        clippy_executable = ctx.toolchains[RUST_TOOLCHAIN_TYPE].clippy_driver,
        process_wrapper = ctx.executable._process_wrapper,
        crate_info = crate_info,
        config = _package_clippy_config(crate_info, ctx.label, ctx.file._config),
        cap_at_warnings = True,
        extra_clippy_flags = flags + [UNUSED_CRATE_DEPENDENCIES],
        error_format = error_format,
        clippy_diagnostics_file = diagnostics,
    )
    return [DxClippyInfo(diagnostics = [diagnostics])]

dx_clippy_aspect = aspect(
    implementation = _dx_clippy_aspect_impl,
    attr_aspects = ["upstream"],
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
