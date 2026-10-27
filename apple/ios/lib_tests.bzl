"""Analysis checks for Rust iOS library cells over stub archives."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load(":defs.bzl", "AppleIosLibInfo")

_XCODES = str(Label("//apple/provisioning:managed_xcodes"))

_MACOS = str(Label("@apple_support//platforms:macos_arm64"))

def _linking(ctx, archive):
    return CcInfo(linking_context = cc_common.create_linking_context(
        linker_inputs = depset([cc_common.create_linker_input(
            owner = ctx.label,
            libraries = depset([cc_common.create_library_to_link(
                actions = ctx.actions,
                static_library = archive,
            )]),
        )]),
    ))

def _archive(ctx):
    archive = ctx.actions.declare_file("lib" + ctx.label.name + ".a")
    ctx.actions.write(archive, "")
    return archive

def _stub_archive_impl(ctx):
    archive = _archive(ctx)
    return [_linking(ctx, archive), DefaultInfo(files = depset([archive]))]

stub_archive = rule(
    implementation = _stub_archive_impl,
    doc = "Provides a CcInfo that links one empty static archive.",
)

def _stub_ios_lib_impl(ctx):
    archive = _archive(ctx)
    return [
        _linking(ctx, archive),
        AppleIosLibInfo(
            archive = archive,
            cell = ctx.attr.cell,
            deployment = "",
            report = archive,
            triple = "",
        ),
    ]

stub_ios_lib = rule(
    implementation = _stub_ios_lib_impl,
    doc = "Provides a CcInfo and an AppleIosLibInfo for one declared cell.",
    attrs = {"cell": attr.string(default = "device/arm64")},
)

def _lib_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    info = target[AppleIosLibInfo]
    asserts.equals(env, "simulator/arm64", info.cell)
    asserts.equals(env, "aarch64-apple-ios-sim", info.triple)
    asserts.equals(env, ctx.attr.report, info.report.basename)
    flags = []
    for linker_input in target[CcInfo].linking_context.linker_inputs.to_list():
        flags.extend(linker_input.user_link_flags)
    asserts.true(env, "-Wl,-framework,Foundation" in flags, "want the Foundation link flag, got {}".format(flags))
    files = [file.basename for file in target[DefaultInfo].files.to_list()]
    asserts.true(env, ctx.attr.report in files, "want the report in the outputs, got {}".format(files))
    return analysistest.end(env)

_lib_test = analysistest.make(
    _lib_impl,
    attrs = {"report": attr.string(mandatory = True)},
    config_settings = {
        "//command_line_option:macos_minimum_os": "14.0",
        "//command_line_option:platforms": [_MACOS],
        "//command_line_option:xcode_version_config": _XCODES,
    },
)

def ios_lib_test(name, target_under_test, report, **kwargs):
    """Asserts one ios_lib moves a macOS build to its simulator cell and checks it."""
    kwargs.setdefault("size", "small")
    _lib_test(name = name, target_under_test = target_under_test, report = report, **kwargs)
