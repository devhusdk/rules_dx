"""Pinned Chromium headless shell for typescript_playwright_test."""

visibility(["//..."])

CHROMIUM_REVISION = "1254"

ARTIFACTS = {
    "linux_arm64": struct(
        constraints = ["@platforms//os:linux", "@platforms//cpu:arm64"],
        path = "linux-arm64/chrome-headless-shell-linux-arm64.zip",
        sha256 = "b136462898353d94c7fab50f9cdaa6d8f0d4c190153916936aa0a03353a26bf2",
    ),
    "linux_x86_64": struct(
        constraints = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        path = "linux64/chrome-headless-shell-linux64.zip",
        sha256 = "18352d22cb0cdf520cbff2ad8c41391f0e02a72886cb53b0fde4bdfc4bd075d6",
    ),
    "macos_arm64": struct(
        constraints = ["@platforms//os:macos", "@platforms//cpu:arm64"],
        path = "mac-arm64/chrome-headless-shell-mac-arm64.zip",
        sha256 = "f34261d2aad94fb71b336597acabf20166ee5a2f14926c731f33d04c5da6cd3f",
    ),
}

_NO_MATCH = ("typescript_playwright_test has no pinned Chromium for this platform; " +
             "supported: " + ", ".join(sorted(ARTIFACTS.keys())) +
             ". Windows is not supported.")

def repository_name(platform):
    """Returns the repository name one platform's Chromium is fetched into."""
    return "dx_playwright_chromium_" + platform

def hub_build():
    """Returns the hub BUILD file selecting one Chromium per platform."""
    lines = []
    for platform in sorted(ARTIFACTS.keys()):
        lines += [
            "config_setting(",
            '    name = "%s",' % platform,
            "    constraint_values = %s," % repr(ARTIFACTS[platform].constraints),
            ")",
            "",
        ]
    for target, source in (("browsers", "browsers"), ("marker", "marker")):
        lines += ["alias(", '    name = "%s",' % target, "    actual = select({"]
        for platform in sorted(ARTIFACTS.keys()):
            lines.append('        ":%s": "@%s//:%s",' % (platform, repository_name(platform), source))
        lines += ["    }, no_match_error = %s)," % repr(_NO_MATCH), '    visibility = ["//visibility:public"],', ")", ""]
    return "\n".join(lines)

def _chromium_impl(rctx):
    rctx.download_and_extract(
        url = rctx.attr.url,
        output = "browsers/chromium_headless_shell-" + CHROMIUM_REVISION,
        sha256 = rctx.attr.sha256,
        type = "zip",
    )
    rctx.file("browsers/chromium_headless_shell-" + CHROMIUM_REVISION + "/INSTALLATION_COMPLETE", "")
    rctx.file("BUILD.bazel", "\n".join([
        "filegroup(",
        '    name = "browsers",',
        '    srcs = glob(["browsers/**"]),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "marker",',
        '    srcs = ["browsers/chromium_headless_shell-%s/INSTALLATION_COMPLETE"],' % CHROMIUM_REVISION,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ]))
    return rctx.repo_metadata(reproducible = True)

chromium_repo = repository_rule(
    implementation = _chromium_impl,
    attrs = {
        "sha256": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
    },
)
