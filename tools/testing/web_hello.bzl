"""Shared hello fixture for the web component languages."""

load("@bazel_skylib//rules:build_test.bzl", "build_test")
load("//javascript:defs.bzl", "javascript_library")
load("//rust:defs.bzl", "rust_test")

def dx_web_hello_fixture(component, library_rule):
    """Declares the hello component library, helper, coverage test and aggregate build test around the :hello_test the package declares.

    Args:
        component: Component source file name.
        library_rule: Language library rule.
    """
    library_rule(
        name = "hello_lib",
        srcs = [component],
    )

    javascript_library(
        name = "helper_lib",
        srcs = ["helper.js"],
    )

    rust_test(
        name = "coverage_test",
        srcs = [Label("//tools/testing:web_hello_coverage_test.rs")],
        data = [":hello_test"],
        env = {
            "HELLO_SOURCES": "hello/%s hello/helper.js" % component,
            "HELLO_TEST": "$(rootpath :hello_test)",
        },
        tags = ["no-remote-exec"],
        deps = [Label("//tools/testing:dx_testing")],
    )

    build_test(
        name = "hello_aggregate_test",
        targets = [
            ":hello_lib",
            ":hello_test",
            ":coverage_test",
        ],
    )
