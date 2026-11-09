"""Pinned Chromium headless shell for typescript_playwright_test."""

CHROMIUM_REVISION = "1248"

ARTIFACTS = {
    "linux_arm64": struct(
        constraints = ["@platforms//os:linux", "@platforms//cpu:arm64"],
        path = "linux-arm64/chrome-headless-shell-linux-arm64.zip",
        sha256 = "81ab9d4d3c301dc5fdfb6c541b9e6632b16fc819a6a854460f40a76fe2b2c476",
    ),
    "linux_x86_64": struct(
        constraints = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        path = "linux64/chrome-headless-shell-linux64.zip",
        sha256 = "c3e50596aef52c58f8fda3c6b3411c33d34c88b177b3ed1c2243359fd3b8ba20",
    ),
    "macos_arm64": struct(
        constraints = ["@platforms//os:macos", "@platforms//cpu:arm64"],
        path = "mac-arm64/chrome-headless-shell-mac-arm64.zip",
        sha256 = "94e4337b70cbc5638f0935fbc46b9f242bc7416f657e3fbe924baf348404b53d",
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
