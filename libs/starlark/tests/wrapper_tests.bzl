"""Unit tests proving optional-provider forwarding warns instead of silently skipping."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:wrapper.bzl", "DX_BINARY_FORWARD_ATTRS", "DX_LIBRARY_FORWARD_ATTRS", "DX_TEST_FORWARD_ATTRS", "dx_binary_forward_kwargs", "dx_forwarded_contract_kwargs", "dx_missing_optional_names", "dx_optional_forward_warning", "dx_symlink_executable_name", "dx_test_forward_kwargs", "dx_test_upstream_kwargs")

def wrapper_optional_forward_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "empty request forwards nothing without warning",
                dx_missing_optional_names([], ["CcInfo"]),
                [],
            ),
            expect_equal(
                "empty request warns nothing",
                dx_optional_forward_warning("go_*", "//go:bin_upstream", [], []),
                None,
            ),
            expect_equal(
                "all present misses nothing",
                dx_missing_optional_names(["GoArchive"], ["GoArchive"]),
                [],
            ),
            expect_equal(
                "all present warns nothing",
                dx_optional_forward_warning("go_*", "//go:bin_upstream", ["GoArchive"], []),
                None,
            ),
            expect_equal(
                "partial skip reports only the missing provider",
                dx_missing_optional_names(
                    ["InstrumentedFilesInfo", "OutputGroupInfo"],
                    ["OutputGroupInfo"],
                ),
                ["InstrumentedFilesInfo"],
            ),
            expect_equal(
                "partial warning names the missing provider and the forward count",
                dx_optional_forward_warning(
                    "javascript_*",
                    "//js:bin_upstream",
                    ["InstrumentedFilesInfo", "OutputGroupInfo"],
                    ["InstrumentedFilesInfo"],
                ),
                "javascript_*: upstream //js:bin_upstream omits optional provider(s) InstrumentedFilesInfo (forwarded 1 of 2)",
            ),
            expect_equal(
                "total skip reports every requested provider",
                dx_missing_optional_names(["CcInfo"], []),
                ["CcInfo"],
            ),
            expect_equal(
                "empty forward warns and marks the empty case expected",
                dx_optional_forward_warning("go_*", "//go:bin_upstream", ["GoArchive"], ["GoArchive"]),
                "go_*: upstream //go:bin_upstream omits optional provider(s) GoArchive (forwarded 0 of 1); empty forward is expected when upstream omits the surface",
            ),
        ],
    )

def wrapper_shape_kwargs_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "binary empty stays empty",
                dx_binary_forward_kwargs({}),
                {},
            ),
            expect_equal(
                "binary tags ride verbatim including manual",
                dx_binary_forward_kwargs({"tags": ["manual", "cpu:4"]}),
                {"tags": ["manual", "cpu:4"]},
            ),
            expect_equal(
                "binary hints ride forwarder",
                dx_binary_forward_kwargs({"aspect_hints": ["//quality:hint"]}),
                {"aspect_hints": ["//quality:hint"]},
            ),
            expect_equal(
                "binary drops non-forwarded attrs",
                dx_binary_forward_kwargs({"timeout": "short", "copts": ["-Werror"]}),
                {},
            ),
            expect_equal(
                "binary target_compatible_with rides forwarder",
                dx_binary_forward_kwargs({"target_compatible_with": ["@platforms//os:linux"]}),
                {"target_compatible_with": ["@platforms//os:linux"]},
            ),
            expect_equal(
                "binary execution constraints ride forwarder",
                dx_binary_forward_kwargs({
                    "compatible_with": ["@platforms//os:linux"],
                    "exec_compatible_with": ["@platforms//os:linux"],
                }),
                {
                    "compatible_with": ["@platforms//os:linux"],
                    "exec_compatible_with": ["@platforms//os:linux"],
                },
            ),
            expect_equal(
                "binary none tags stay empty",
                dx_binary_forward_kwargs({"tags": None}),
                {},
            ),
            expect_equal(
                "library shape forwards compatibility and execution constraints",
                dx_forwarded_contract_kwargs({
                    "compatible_with": ["@platforms//os:linux"],
                    "exec_compatible_with": ["@platforms//os:linux"],
                    "hdrs": ["greet.h"],
                    "target_compatible_with": ["@platforms//os:linux"],
                }, DX_LIBRARY_FORWARD_ATTRS),
                {
                    "compatible_with": ["@platforms//os:linux"],
                    "exec_compatible_with": ["@platforms//os:linux"],
                    "hdrs": ["greet.h"],
                    "target_compatible_with": ["@platforms//os:linux"],
                },
            ),
            expect_equal(
                "library shape drops language kwargs",
                dx_forwarded_contract_kwargs({"deps": [":greet_lib"], "copts": ["-Wall"]}, DX_LIBRARY_FORWARD_ATTRS),
                {},
            ),
            expect_equal(
                "binary shape does not forward library headers",
                dx_forwarded_contract_kwargs({"hdrs": ["greet.h"]}, DX_BINARY_FORWARD_ATTRS),
                {},
            ),
            expect_equal(
                "test shape does not forward library headers",
                dx_forwarded_contract_kwargs({"hdrs": ["greet.h"]}, DX_TEST_FORWARD_ATTRS),
                {},
            ),
            expect_equal(
                "test shape drops language kwargs",
                dx_forwarded_contract_kwargs({"crate": ":greet", "deps": [":greet_lib"]}, DX_TEST_FORWARD_ATTRS),
                {},
            ),
            expect_equal(
                "an undeclared attribute is never forwarded",
                dx_forwarded_contract_kwargs({"data": ["greet.txt"]}, DX_TEST_FORWARD_ATTRS),
                {},
            ),
            expect_equal(
                "test upstream empty stays private",
                dx_test_upstream_kwargs({}),
                {"visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream strips manual-only tags",
                dx_test_upstream_kwargs({"tags": ["manual"]}),
                {"visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream keeps non-manual tags",
                dx_test_upstream_kwargs({"tags": ["manual", "cpu:4"]}),
                {"tags": ["cpu:4"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream forces private visibility",
                dx_test_upstream_kwargs({"visibility": ["//visibility:public"]}),
                {"visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream sets srcs when given",
                dx_test_upstream_kwargs({}, ["hello_test.go"]),
                {"visibility": ["//visibility:private"], "srcs": ["hello_test.go"]},
            ),
            expect_equal(
                "test upstream keeps compiler flags",
                dx_test_upstream_kwargs({"copts": ["-Werror"]}),
                {"copts": ["-Werror"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test forward empty stays empty",
                dx_test_forward_kwargs({}),
                {},
            ),
            expect_equal(
                "test forward keeps the user manual tag",
                dx_test_forward_kwargs({"tags": ["manual", "cpu:4"]}),
                {"tags": ["manual", "cpu:4"]},
            ),
            expect_equal(
                "test forward keeps timeout and flaky",
                dx_test_forward_kwargs({"timeout": "short", "flaky": True}),
                {"flaky": True, "timeout": "short"},
            ),
            expect_equal(
                "test forward declares the test environment",
                dx_test_forward_kwargs({"env": {"A": "b"}, "env_inherit": ["PATH"]}),
                {"env": {"A": "b"}, "env_inherit": ["PATH"]},
            ),
            expect_equal(
                "test forward rides hints",
                dx_test_forward_kwargs({"aspect_hints": ["//quality:hint"]}),
                {"aspect_hints": ["//quality:hint"]},
            ),
            expect_equal(
                "test forward drops non-test attrs",
                dx_test_forward_kwargs({"copts": ["-Werror"]}),
                {},
            ),
            expect_equal(
                "test forward rides the execution constraints",
                dx_test_forward_kwargs({
                    "compatible_with": ["@platforms//os:linux"],
                    "exec_compatible_with": ["@platforms//os:linux"],
                    "target_compatible_with": ["@platforms//os:linux"],
                }),
                {
                    "compatible_with": ["@platforms//os:linux"],
                    "exec_compatible_with": ["@platforms//os:linux"],
                    "target_compatible_with": ["@platforms//os:linux"],
                },
            ),
        ],
    )

def wrapper_symlink_naming_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "extensionless upstream keeps the forwarder name",
                dx_symlink_executable_name("hello", struct(basename = "hello")),
                "hello",
            ),
            expect_equal(
                "windows executables stay .exe",
                dx_symlink_executable_name("hello", struct(basename = "hello.exe")),
                "hello.exe",
            ),
            expect_equal(
                "windows batch launchers stay .bat",
                dx_symlink_executable_name("hello", struct(basename = "hello.bat")),
                "hello.bat",
            ),
            expect_equal(
                "powershell and py launchers keep their own suffix",
                [
                    dx_symlink_executable_name("greet", struct(basename = "greet.ps1")),
                    dx_symlink_executable_name("greet", struct(basename = "greet.py")),
                ],
                ["greet.ps1", "greet.py"],
            ),
        ],
    )
