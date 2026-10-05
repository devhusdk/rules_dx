"""Shared wrapper conformance checks for language rules."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:wrapper.bzl", "dx_forwarded_test_kwargs", "dx_merged_environment")
load("//quality:sources.bzl", "KNOWN_SEMANTIC_FILE_CLASSES")

def dx_file_class_checks(classes):
    """Builds known-class checks for each file class."""
    checks = []
    for file_class in classes:
        checks.append(expect_equal(file_class + " class is known", file_class in KNOWN_SEMANTIC_FILE_CLASSES, True))
    return checks

def dx_forwarder_checks():
    """Builds shared test-attribute and environment-merge checks."""
    return [
        expect_equal("test kwargs keep the user manual tag", dx_forwarded_test_kwargs({"tags": ["manual", "cpu:4"]}), {"tags": ["manual", "cpu:4"]}),
        expect_equal("test kwargs forward timeout, shard_count and size", dx_forwarded_test_kwargs({"timeout": "short", "shard_count": 2, "size": "small"}), {"timeout": "short", "shard_count": 2, "size": "small"}),
        expect_equal("test kwargs forward the declared env", dx_forwarded_test_kwargs({"env": {"A": "b"}}), {"env": {"A": "b"}}),
        expect_equal("test kwargs forward allowed inherited variables", dx_forwarded_test_kwargs({"env_inherit": ["PATH"]}), {"env_inherit": ["PATH"]}),
        expect_equal("test kwargs forward flaky", dx_forwarded_test_kwargs({"flaky": True}), {"flaky": True}),
        expect_equal("test kwargs empty stays empty", dx_forwarded_test_kwargs({}), {}),
        expect_equal("test kwargs drop language attributes", dx_forwarded_test_kwargs({"copts": ["-Werror"]}), {}),
        expect_equal("the upstream env value wins over the declared fallback", dx_merged_environment({"A": "upstream", "B": "upstream"}, [], {"A": "declared"}, []), ({"A": "upstream", "B": "upstream"}, [])),
        expect_equal("a declared env key the upstream omits still applies", dx_merged_environment({"A": "upstream"}, [], {"A": "declared", "C": "declared"}, []), ({"A": "upstream", "C": "declared"}, [])),
        expect_equal("upstream inherited variables survive", dx_merged_environment({}, ["PATH"], {}, ["HOME"]), ({}, ["PATH", "HOME"])),
        expect_equal("a repeated inherited variable stays once", dx_merged_environment({}, ["PATH"], {}, ["PATH"]), ({}, ["PATH"])),
    ]

def dx_wrapper_contract_tests(name, file_classes = None, extra_checks = None, include_forwarder_checks = True):
    """Instantiates one wrapper contract test from shared checks."""
    classes = file_classes if file_classes != None else []
    extras = extra_checks if extra_checks != None else []
    checks = []
    checks.extend(dx_file_class_checks(classes))
    if include_forwarder_checks:
        checks.extend(dx_forwarder_checks())
    checks.extend(extras)
    starlark_test(
        name = name,
        mode = "unit",
        checks = checks,
    )
