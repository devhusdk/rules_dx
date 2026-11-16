"""Chromium headless shell for typescript_playwright_test, pinned in typescript/private/playwright/pins.json."""

load("//core:deb_closure.bzl", "deb_closure")
load("//core:jammy_runtime.bzl", "JAMMY_RUNTIME_MIRROR")
load(":closure.bzl", "CLOSURE_DIR", "chromium_closure")

visibility(["//..."])

def constraints(platform):
    """Returns the os and cpu constraint values of one pinned cell."""
    os, cpu = platform.split("_", 1)
    return ["@platforms//os:" + os, "@platforms//cpu:" + cpu]

def repository_name(platform):
    """Returns the repository name one platform's Chromium is fetched into."""
    return "dx_playwright_chromium_" + platform

def hub_build(platforms):
    """Returns the hub BUILD file selecting one Chromium per platform."""
    platforms = sorted(platforms)
    no_match = ("typescript_playwright_test has no pinned Chromium for this platform; " +
                "supported: " + ", ".join(platforms) +
                ". Windows is not supported.")
    lines = []
    for platform in platforms:
        lines += [
            "config_setting(",
            '    name = "%s",' % platform,
            "    constraint_values = %s," % repr(constraints(platform)),
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    for target, source in (("browsers", "browsers"), ("closure", "closure"), ("marker", "marker")):
        lines += ["alias(", '    name = "%s",' % target, "    actual = select({"]
        for platform in platforms:
            lines.append('        ":%s": "@%s//:%s",' % (platform, repository_name(platform), source))
        lines += ["    }, no_match_error = %s)," % repr(no_match), '    visibility = ["//visibility:public"],', ")", ""]
    return "\n".join(lines)

def _chromium_impl(rctx):
    browser = "browsers/chromium_headless_shell-" + rctx.attr.revision
    rctx.download_and_extract(
        url = rctx.attr.urls,
        output = browser,
        sha256 = rctx.attr.sha256,
        type = "zip",
    )
    rctx.file(browser + "/INSTALLATION_COMPLETE", "")
    pins = json.decode(rctx.read(Label(":pins.json")))
    deb_closure(rctx, JAMMY_RUNTIME_MIRROR, chromium_closure(pins, rctx.attr.platform), CLOSURE_DIR)
    rctx.file("BUILD.bazel", "\n".join([
        "filegroup(",
        '    name = "browsers",',
        '    srcs = glob(["browsers/**"]),',
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "closure",',
        '    srcs = glob(["%s/**"], allow_empty = True),' % CLOSURE_DIR,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
        "filegroup(",
        '    name = "marker",',
        '    srcs = ["%s/INSTALLATION_COMPLETE"],' % browser,
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ]))
    return rctx.repo_metadata(reproducible = True)

chromium_repo = repository_rule(
    implementation = _chromium_impl,
    attrs = {
        "platform": attr.string(mandatory = True),
        "revision": attr.string(mandatory = True),
        "sha256": attr.string(mandatory = True),
        "urls": attr.string_list(mandatory = True),
    },
)
