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

def dx_web_app(build, library, library_srcs, srcs, tests, size, library_name = "components", **kwargs):
    """Declares the web app fixture: logo, component library, site build, server, jest tests and a build test."""
    dx_web_app_logo()
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
    for test, data in tests.items():
        javascript_test(
            name = test,
            size = size,
            srcs = [test[:-len("_test")] + ".test.js"],
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
