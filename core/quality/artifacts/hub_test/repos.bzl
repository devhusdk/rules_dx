"""Fixture tool repositories for the hub selection tests."""

def _hub_fixture_repo_impl(ctx):
    ctx.file("tool.txt", "fixture " + ctx.attr.tool + " repository for " + ctx.attr.platform + "\n")
    ctx.file("BUILD.bazel", "\n".join([
        "filegroup(",
        '    name = "tool",',
        '    srcs = ["tool.txt"],',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ]))
    return ctx.repo_metadata(reproducible = True)

hub_fixture_repo = repository_rule(
    implementation = _hub_fixture_repo_impl,
    attrs = {
        "platform": attr.string(
            doc = "Platform name written into the fixture tool file.",
            mandatory = True,
        ),
        "tool": attr.string(
            doc = "Tool name written into the fixture tool file.",
            mandatory = True,
        ),
    },
)
