"""Stages conflicting node runtimes for the launcher ambiguity test."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _ambiguous_nodes_impl(ctx):
    """Writes two node runtimes into one runfiles closure."""
    runtimes = []
    for name in ["node_a/node.exe", "node_b/node.exe"]:
        runtime = ctx.actions.declare_file("ambiguous_node/" + name)
        ctx.actions.write(runtime, "runtime\n")
        runtimes.append(runtime)
    return [DefaultInfo(
        files = depset(runtimes),
        runfiles = ctx.runfiles(files = runtimes),
        executable = runtimes[0],
    )]

ambiguous_nodes = rule(
    doc = "Stages two node runtimes in one runfiles closure.",
    implementation = _ambiguous_nodes_impl,
)

def _windows_failure_impl(ctx):
    """Asserts the analysis failure names the expected text."""
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

windows_failure_test = analysistest.make(
    _windows_failure_impl,
    expect_failure = True,
    attrs = {
        "expected_failure_substring": attr.string(
            doc = "Text the analysis failure must contain.",
            mandatory = True,
        ),
    },
    config_settings = {"//command_line_option:platforms": str(Label("//msix/platforms:windows_x86_64"))},
)
