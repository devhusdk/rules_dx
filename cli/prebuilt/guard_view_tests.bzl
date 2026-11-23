"""Analysis tests for the guard_view flag in the exec configuration."""

load("@bazel_skylib//lib:paths.bzl", "paths")
load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

visibility("private")

_ExecToolsInfo = provider(
    doc = "The executables of exec-configured tools.",
    fields = {"executables": "list of File"},
)

def _exec_tools_impl(ctx):
    return [_ExecToolsInfo(executables = [tool[DefaultInfo].files_to_run.executable for tool in ctx.attr.tools])]

_exec_tools = rule(
    implementation = _exec_tools_impl,
    attrs = {"tools": attr.label_list(cfg = "exec")},
)

def _guard_view_impl(ctx):
    env = analysistest.begin(ctx)
    executables = analysistest.target_under_test(env)[_ExecToolsInfo].executables
    asserts.equals(env, ["guard_stub"] * len(executables), [paths.basename(f.path).removesuffix(".exe") for f in executables])
    return analysistest.end(env)

_guard_view_test = analysistest.make(
    _guard_view_impl,
    config_settings = {str(Label("//cli/prebuilt:guard_view")): True},
)

def guard_view_tests(name, size = "small"):
    """Declares the test that exec-configured hub tools resolve to guard_stub under guard_view."""
    _exec_tools(
        name = name + "_tools",
        tools = [
            "@dx_prebuilt//:quality_runner",
            "@dx_prebuilt//:deadnix",
            "@dx_prebuilt//:javac_check",
            "@dx_gazelle//:gazelle",
        ],
        tags = ["manual"],
        testonly = True,
    )
    _guard_view_test(name = name, size = size, target_under_test = ":" + name + "_tools")
