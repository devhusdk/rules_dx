"""Tests that the standard attributes of a *_deploy macro land on every generated target."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":config.bzl", "deploy_attrs")

PROBE_TAG = "dx_attrs_probe"
PROBE_VISIBILITY = ["//visibility:private"]
PROBE_COMPATIBLE = ["@platforms//os:linux"]

def _missing(prefix):
    missing = []
    found = 0
    for rule_name, rule in native.existing_rules().items():
        if rule_name != prefix and not rule_name.startswith(prefix + "_"):
            continue
        found += 1
        if PROBE_TAG not in rule.get("tags", []):
            missing.append(rule_name + " tags")
        if not rule.get("testonly", False):
            missing.append(rule_name + " testonly")
        if "private" not in str(rule.get("visibility", "")):
            missing.append(rule_name + " visibility")
        if len(rule.get("target_compatible_with", [])) != 1:
            missing.append(rule_name + " target_compatible_with")
    if found < 2:
        missing.append(prefix + " generated only " + str(found) + " targets")
    return missing

def standard_attrs_unit_tests(name, prefixes):
    """Checks every target generated under each prefix carries the probe attributes."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "deploy_attrs keeps all four attributes",
                deploy_attrs(["a"], ["//visibility:private"], True, ["b"]),
                {
                    "tags": ["a"],
                    "target_compatible_with": ["b"],
                    "testonly": True,
                    "visibility": ["//visibility:private"],
                },
            ),
        ] + [
            expect_equal(
                prefix + " forwards the standard attributes to every generated target",
                _missing(prefix),
                [],
            )
            for prefix in prefixes
        ],
    )
