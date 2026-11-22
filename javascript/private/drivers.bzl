"""Bench and fuzz driver forwarders that reach the private driver scripts."""

load("@aspect_rules_js//js:providers.bzl", "JsInfo")
load("//core:wrapper.bzl", "dx_private_dep_rule", "dx_upstream_tags")

visibility(["//javascript/...", "//typescript/..."])

_bench_driver = dx_private_dep_rule(
    Label("//javascript/private:bench_driver"),
    [DefaultInfo, JsInfo],
    "Provides the script that runs a tinybench Bench and writes dx-bench.json.",
)

_fuzz_driver = dx_private_dep_rule(
    Label("//javascript/private:fuzz_driver"),
    [DefaultInfo, JsInfo],
    "Provides the script that runs a Jazzer.js fuzz target.",
)

def javascript_driver(name, kind, tags = None):
    """Declares the bench or fuzz driver for one target and returns its label."""
    rule = {"bench": _bench_driver, "fuzz": _fuzz_driver}[kind]
    driver = name + "_dx_" + kind + "_driver"
    rule(
        name = driver,
        tags = dx_upstream_tags(tags),
        visibility = ["//visibility:private"],
    )
    return native.package_relative_label(":" + driver)
