"""Shared helpers for the web component languages."""

load("@bazel_skylib//rules:build_test.bzl", "build_test")
load("@bazel_skylib//rules:copy_file.bzl", "copy_file")
load("//cli/web_server:server.bzl", "javascript_web_server")
load("//javascript:defs.bzl", "javascript_test")

def dx_web_app_logo(name = "logo"):
    """Places the shared web app logo at src/assets/logo.png in the calling package."""
    copy_file(
        name = name,
        src = Label("//tools/testing:web_app_logo.png"),
        out = "src/assets/logo.png",
        testonly = False,
    )

def dx_web_app(build, library, library_srcs, srcs, tests, size, library_name = "components", client_test = False, **kwargs):
    """Declares the web app fixture and its tests."""
    dx_web_app_logo()
    if client_test:
        copy_file(
            name = "site_client_src",
            src = Label("//tools/testing:web_app_site_client.test.js"),
            out = "site_client.test.js",
        )
        tests = dict(tests)
        tests["site_client.test.js"] = [
            ":site",
            "//:node_modules/jest-environment-jsdom",
        ]
    library(
        name = library_name,
        srcs = library_srcs,
    )
    build(
        name = "site",
        srcs = srcs + [":logo"],
        node_modules = "//:node_modules",
        **kwargs
    )
    javascript_web_server(
        name = "serve",
        app = ":site",
    )
    for src, data in tests.items():
        if not src.endswith(".test.js"):
            fail("dx_web_app tests keys must end in .test.js, got %r" % src)
        javascript_test(
            name = src[:-len(".test.js")] + "_test",
            size = size,
            srcs = [src],
            data = data,
            node_modules = "//:node_modules",
            node_options = ["--experimental-vm-modules"],
            package_json = "//:package_json",
        )
    build_test(
        name = "app_build_test",
        size = size,
        targets = [
            ":" + library_name,
            ":serve",
            ":site",
        ],
    )
