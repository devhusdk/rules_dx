"""Shared hello fixture for the web component languages."""

load("@bazel_skylib//rules:build_test.bzl", "build_test")
load("@bazel_skylib//rules:copy_file.bzl", "copy_file")
load("//javascript:defs.bzl", "javascript_library")
load("//tools/testing/conformance:defs.bzl", "dx_coverage_test")

def dx_web_hello_fixture(component, library_rule):
    """Declares the hello component library, helper, coverage test and aggregate build test around the :hello_test the package declares."""
    library_rule(
        name = "hello_lib",
        srcs = [component],
    )

    javascript_library(
        name = "helper_lib",
        srcs = ["helper.js"],
    )

    dx_coverage_test(
        name = "coverage_test",
        subject = ":hello_test",
        size = "small",
        covered_source = "hello/helper.js",
    )

    dx_coverage_test(
        name = "component_coverage_test",
        subject = ":hello_test",
        size = "small",
        covered_source = "hello/%s" % component,
    )

    build_test(
        name = "hello_aggregate_test",
        size = "small",
        targets = [
            ":hello_lib",
            ":hello_test",
            ":coverage_test",
            ":component_coverage_test",
        ],
    )

def dx_web_app_logo(name = "logo"):
    """Places the shared web app logo at src/assets/logo.png in the calling package."""
    copy_file(
        name = name,
        src = Label("//tools/testing:web_app_logo.png"),
        out = "src/assets/logo.png",
        testonly = False,
    )
