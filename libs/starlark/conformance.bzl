"""Shared wrapper conformance checks for language rules."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:wrapper.bzl", "dx_binary_forward_kwargs", "dx_effective_visibility", "dx_forwarded_test_kwargs")
load("//quality:sources.bzl", "KNOWN_SEMANTIC_FILE_CLASSES")

def dx_file_class_checks(classes):
    """Builds known-class checks for each file class."""
    checks = []
    for file_class in classes:
        checks.append(expect_equal(file_class + " class is known", file_class in KNOWN_SEMANTIC_FILE_CLASSES, True))
    return checks

def dx_forwarder_checks():
    """Builds shared visibility and test kwarg checks."""
    return [
        expect_equal("forwarder defaults to private", dx_effective_visibility(None), ["//visibility:private"]),
        expect_equal("explicit visibility wins", dx_effective_visibility(["//visibility:public"]), ["//visibility:public"]),
        expect_equal("test kwargs keep manual", dx_forwarded_test_kwargs({"tags": ["manual", "cpu:4"]}), {"tags": ["manual", "cpu:4"]}),
        expect_equal("test kwargs forward timeout and flaky", dx_forwarded_test_kwargs({"timeout": "short", "flaky": True}), {"timeout": "short", "flaky": True}),
        expect_equal("test kwargs forward flaky", dx_forwarded_test_kwargs({"flaky": True}), {"flaky": True}),
        expect_equal("test kwargs forward args", dx_forwarded_test_kwargs({"args": ["--x"]}), {"args": ["--x"]}),
        expect_equal("test kwargs forward data with args", dx_forwarded_test_kwargs({"args": ["--x"], "data": [":d"]}), {"args": ["--x"], "data": [":d"]}),
        expect_equal("test kwargs keep data upstream without args", dx_forwarded_test_kwargs({"data": [":d"]}), {}),
        expect_equal("test kwargs keep env upstream", dx_forwarded_test_kwargs({"env": {"K": "1"}}), {}),
        expect_equal("test kwargs keep env_inherit upstream", dx_forwarded_test_kwargs({"env_inherit": ["HOME"]}), {}),
        expect_equal("test kwargs forward exec_properties", dx_forwarded_test_kwargs({"exec_properties": {"Pool": "gpu"}}), {"exec_properties": {"Pool": "gpu"}}),
        expect_equal("test kwargs drop other exec group properties", dx_forwarded_test_kwargs({"exec_properties": {"cpp_link.Pool": "c"}}), {}),
        expect_equal("test kwargs keep test and unprefixed properties", dx_forwarded_test_kwargs({"exec_properties": {"Pool": "a", "test.Pool": "b", "cpp_link.Pool": "c"}}), {"exec_properties": {"Pool": "a", "test.Pool": "b"}}),
        expect_equal("test kwargs forward exec_compatible_with", dx_forwarded_test_kwargs({"exec_compatible_with": ["@platforms//os:linux"]}), {"exec_compatible_with": ["@platforms//os:linux"]}),
        expect_equal("test kwargs forward test exec group", dx_forwarded_test_kwargs({"exec_group_compatible_with": {"test": ["@platforms//os:linux"]}}), {"exec_group_compatible_with": {"test": ["@platforms//os:linux"]}}),
        expect_equal("test kwargs keep other exec groups upstream", dx_forwarded_test_kwargs({"exec_group_compatible_with": {"cpp_link": ["@platforms//os:linux"]}}), {}),
        expect_equal("test kwargs forward only test exec group", dx_forwarded_test_kwargs({"exec_group_compatible_with": {"test": ["@platforms//os:linux"], "cpp_link": ["@platforms//cpu:x86_64"]}}), {"exec_group_compatible_with": {"test": ["@platforms//os:linux"]}}),
        expect_equal("test kwargs forward local", dx_forwarded_test_kwargs({"local": True}), {"local": True}),
        expect_equal("test kwargs keep local False", dx_forwarded_test_kwargs({"local": False}), {"local": False}),
        expect_equal("binary kwargs forward testonly", dx_binary_forward_kwargs({"testonly": True}), {"testonly": True}),
        expect_equal("binary kwargs forward args with data", dx_binary_forward_kwargs({"args": ["--x"], "data": [":d"]}), {"args": ["--x"], "data": [":d"]}),
        expect_equal("binary kwargs keep data upstream without args", dx_binary_forward_kwargs({"data": [":d"]}), {}),
        expect_equal("binary kwargs drop none args", dx_binary_forward_kwargs({"args": None}), {}),
        expect_equal("test kwargs empty stays empty", dx_forwarded_test_kwargs({}), {}),
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
