"""Firefox acquisition for the documentation browser harness, pinned in tools/docs/site/browser/pins.json."""

_EXECUTABLE = "firefox/firefox"

_REPOSITORY_PREFIX = "dx_firefox_"

_CELL_KEYS = ["binary", "sha256", "type", "urls"]

def repository_name(platform):
    """Returns the repository name one platform's Firefox is acquired into."""
    return _REPOSITORY_PREFIX + platform

def constraints(platform):
    """Returns the os and cpu constraint values of one pinned cell."""
    os, cpu = platform.split("_", 1)
    return ["@platforms//os:" + os, "@platforms//cpu:" + cpu]

def no_match_error(pins):
    """Returns the diagnostic for an execution platform upstream ships no Firefox for."""
    return ("rules_dx: no pinned Firefox " + pins["version"] + " artifact for this execution " +
            "platform; want one of " + ", ".join(sorted(pins["cells"].keys())))

def artifact_errors(pins):
    """Returns one error string per cell that is not ready to download."""
    errors = []
    for platform in sorted(pins["cells"].keys()):
        cell = pins["cells"][platform]
        missing = [key for key in _CELL_KEYS if not cell.get(key)]
        if len(missing) > 0:
            errors.append("cell '" + platform + "' is missing " + ", ".join(missing))
            continue
        if cell["binary"] != _EXECUTABLE:
            errors.append("cell '" + platform + "' binary is '" + cell["binary"] + "', want '" + _EXECUTABLE + "'")
        expected = "/firefox-" + pins["version"] + ".tar.xz"
        if not cell["urls"][0].endswith(expected):
            errors.append("cell '" + platform + "' url is '" + cell["urls"][0] + "', want one ending in '" + expected + "'")
    return errors

def hub_build(pins):
    """Returns the hub BUILD file selecting one pinned artifact per platform."""
    platforms = sorted(pins["cells"].keys())
    lines = []
    for platform in platforms:
        lines.append("config_setting(")
        lines.append('    name = "%s",' % platform)
        lines.append("    constraint_values = [")
        for constraint in constraints(platform):
            lines.append('        "%s",' % constraint)
        lines.append("    ],")
        lines.append(")")
    for target, source in (("firefox", "binary"), ("tree", "tool")):
        lines.append("")
        lines.append("alias(")
        lines.append('    name = "%s",' % target)
        lines.append("    actual = select({")
        for platform in platforms:
            lines.append('        ":%s": "@%s//:%s",' % (platform, repository_name(platform), source))
        lines.append("    }, no_match_error = \"%s\")," % no_match_error(pins))
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")
    return "\n".join(lines) + "\n"

def _firefox_repo_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.urls,
        output = "extracted",
        sha256 = ctx.attr.sha256,
        type = ctx.attr.archive_format,
        canonical_id = "dx-firefox-archive:" + ctx.attr.urls[0],
    )
    if not ctx.path("extracted/" + _EXECUTABLE).exists:
        fail("firefox: '" + ctx.attr.urls[0] + "' holds no '" + _EXECUTABLE + "'")
    ctx.file("BUILD.bazel", _tool_build())
    return ctx.repo_metadata(reproducible = True)

def _tool_build():
    """Returns the BUILD file exposing one acquired Firefox tree."""
    return "\n".join([
        "filegroup(",
        '    name = "binary",',
        '    srcs = ["extracted/%s"],' % _EXECUTABLE,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "tool",',
        "    srcs = glob([",
        '        "extracted/**",',
        "    ]),",
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

firefox_repo = repository_rule(
    implementation = _firefox_repo_impl,
    attrs = {
        "archive_format": attr.string(
            doc = "Archive type used to extract the downloaded tool.",
            mandatory = True,
        ),
        "sha256": attr.string(
            doc = "Expected SHA-256 of the downloaded archive.",
            mandatory = True,
        ),
        "urls": attr.string_list(doc = "Download URLs of the tool archive.", mandatory = True),
    },
)
