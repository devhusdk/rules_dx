"""Unit tests proving optional-provider forwarding warns instead of silently skipping."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:resources.bzl", "dx_bundler_resources", "dx_quality_resource_set", "dx_test_resource_tags")
load("//libs/starlark:wrapper.bzl", "dx_append_flags", "dx_binary_forward_kwargs", "dx_binary_upstream_kwargs", "dx_missing_optional_names", "dx_optional_forward_warning", "dx_setdefaults", "dx_src_path", "dx_symlink_executable_name", "dx_test_forward_kwargs", "dx_test_upstream_kwargs", "dx_upstream_tags", "dx_wrap_forward_kwargs", "dx_wrap_upstream_kwargs")
load("//tools/testing:nested_bazel.bzl", "NESTED_BAZEL_TAGS")

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
                    ["DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo"],
                    ["DotnetAssemblyCompileInfo"],
                ),
                ["DotnetAssemblyRuntimeInfo"],
            ),
            expect_equal(
                "partial warning names the missing provider and the forward count",
                dx_optional_forward_warning(
                    "csharp_*",
                    "//cs:bin_upstream",
                    ["DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo"],
                    ["DotnetAssemblyRuntimeInfo"],
                ),
                "csharp_*: upstream //cs:bin_upstream omits optional provider(s) DotnetAssemblyRuntimeInfo (forwarded 1 of 2)",
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

def _forwarded_exec_groups_input():
    """Returns the exec group kwargs after one forward pass reads them."""
    kwargs = {"exec_group_compatible_with": {"test": ["@platforms//os:linux"], "cpp_link": ["@platforms//cpu:x86_64"]}}
    dx_test_forward_kwargs(kwargs)
    return kwargs

def wrapper_kwarg_hooks_tests(name):
    """Checks the append-flags and setdefaults kwargs hooks."""
    append_input = {"copts": ["-O2"], "deps": [":a"]}
    append_output = dx_append_flags(append_input, "copts", ["-Werror", "-O2", "-Wall"])
    select_output = dx_append_flags({"copts": select({"//conditions:default": ["-O2"]})}, "copts", ["-Werror"])
    defaults_input = {"keep": "user", "off": False}
    defaults_output = dx_setdefaults(defaults_input, {"keep": "default", "new": ["net10.0"], "off": True})
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "append flags sets a missing key",
                dx_append_flags({}, "javacopts", ["-Werror", "-Xlint:all"]),
                {"javacopts": ["-Werror", "-Xlint:all"]},
            ),
            expect_equal(
                "append flags replaces none",
                dx_append_flags({"javacopts": None}, "javacopts", ["-Werror"]),
                {"javacopts": ["-Werror"]},
            ),
            expect_equal(
                "append flags skips present flags and keeps order",
                append_output["copts"],
                ["-O2", "-Werror", "-Wall"],
            ),
            expect_equal(
                "append flags keeps other keys",
                append_output["deps"],
                [":a"],
            ),
            expect_equal(
                "append flags leaves its input unchanged",
                append_input,
                {"copts": ["-O2"], "deps": [":a"]},
            ),
            expect_equal(
                "append flags keeps a select",
                type(select_output["copts"]),
                "select",
            ),
            expect_equal(
                "setdefaults sets an absent key",
                defaults_output["new"],
                ["net10.0"],
            ),
            expect_equal(
                "setdefaults keeps a present key",
                defaults_output["keep"],
                "user",
            ),
            expect_equal(
                "setdefaults keeps false",
                defaults_output["off"],
                False,
            ),
            expect_equal(
                "setdefaults keeps an explicit none",
                dx_setdefaults({"opts": None}, {"opts": "default"}),
                {"opts": None},
            ),
            expect_equal(
                "setdefaults leaves its input unchanged",
                defaults_input,
                {"keep": "user", "off": False},
            ),
        ],
    )

def wrapper_shape_kwargs_tests(name):
    """Checks upstream and forwarder kwargs for library, test and binary shapes."""
    wrap_input = {"copts": ["-Werror"], "tags": ["manual", "no-lint"], "visibility": ["//visibility:public"]}
    wrap_output = dx_wrap_upstream_kwargs(wrap_input, ["lib.cc"])
    upstream_tags_input = ["no-lint"]
    upstream_tags_output = dx_upstream_tags(upstream_tags_input)
    test_input = {"srcs": ["a_test.go"], "tags": ["no-lint"]}
    test_output = dx_test_upstream_kwargs(test_input, ["b_test.go"])
    binary_input = {"copts": ["-Werror"], "srcs": ["a.go"], "tags": ["no-lint"], "visibility": ["//visibility:public"]}
    binary_output = dx_binary_upstream_kwargs(binary_input, [])
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "src path joins package",
                dx_src_path("pkg", "a/B.java"),
                "pkg/a/B.java",
            ),
            expect_equal(
                "src path strips colon",
                dx_src_path("pkg", ":a/B.java"),
                "pkg/a/B.java",
            ),
            expect_equal(
                "src path resolves absolute label",
                dx_src_path("pkg", "//p:a/B.java"),
                "p/a/B.java",
            ),
            expect_equal(
                "src path resolves shorthand label",
                dx_src_path("pkg", "//p/q"),
                "p/q/q",
            ),
            expect_equal(
                "src path resolves root label",
                dx_src_path("pkg", "//:B.java"),
                "B.java",
            ),
            expect_equal(
                "src path at root has no leading slash",
                dx_src_path("", "a/B.java"),
                "a/B.java",
            ),
            expect_equal(
                "src path skips external labels",
                dx_src_path("pkg", "@x//p:B.java"),
                None,
            ),
            expect_equal(
                "src path skips non-strings",
                dx_src_path("pkg", None),
                None,
            ),
            expect_equal(
                "library upstream without tags stays private",
                dx_wrap_upstream_kwargs({}, ["lib.cc"]),
                {"srcs": ["lib.cc"], "tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "library upstream none tags become manual",
                dx_wrap_upstream_kwargs({"tags": None}, []),
                {"srcs": [], "tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "library upstream leaves srcs out when srcs is none",
                "srcs" in dx_wrap_upstream_kwargs({}, None),
                False,
            ),
            expect_equal(
                "library upstream drops a stray srcs kwarg when srcs is none",
                "srcs" in dx_wrap_upstream_kwargs({"srcs": ["stray.cc"]}, None),
                False,
            ),
            expect_equal(
                "library upstream passes empty srcs through",
                dx_wrap_upstream_kwargs({}, [])["srcs"],
                [],
            ),
            expect_equal(
                "library upstream keeps manual",
                dx_wrap_upstream_kwargs({"tags": ["manual"]}, ["lib.cc"])["tags"],
                ["manual"],
            ),
            expect_equal(
                "library upstream appends manual after user tags",
                dx_wrap_upstream_kwargs({"tags": ["no-remote-exec", "no-lint"]}, [])["tags"],
                ["no-remote-exec", "dx:no-lint", "manual"],
            ),
            expect_equal(
                "upstream tags for none are manual",
                dx_upstream_tags(None),
                ["manual"],
            ),
            expect_equal(
                "upstream tags keep one manual and rewrite skip tags",
                dx_upstream_tags(["manual", "no-format"]),
                ["manual", "dx:no-format"],
            ),
            expect_equal(
                "upstream tags append manual to a new list",
                upstream_tags_output,
                ["dx:no-lint", "manual"],
            ),
            expect_equal(
                "upstream tags leave their input unchanged",
                upstream_tags_input,
                ["no-lint"],
            ),
            expect_equal(
                "library upstream keeps execution tags in order and rewrites skip tags",
                dx_wrap_upstream_kwargs({"tags": ["manual", "no-remote-exec", "requires-network", "no-lint"]}, ["lib.cc"])["tags"],
                ["manual", "no-remote-exec", "requires-network", "dx:no-lint"],
            ),
            expect_equal(
                "library upstream forces private visibility and passes srcs and copts",
                wrap_output,
                {"copts": ["-Werror"], "srcs": ["lib.cc"], "tags": ["manual", "dx:no-lint"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "library upstream leaves its input unchanged",
                wrap_input,
                {"copts": ["-Werror"], "tags": ["manual", "no-lint"], "visibility": ["//visibility:public"]},
            ),
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
                "binary testonly rides forwarder",
                dx_binary_forward_kwargs({"testonly": True}),
                {"testonly": True},
            ),
            expect_equal(
                "binary args ride forwarder",
                dx_binary_forward_kwargs({"args": ["--x"]}),
                {"args": ["--x"]},
            ),
            expect_equal(
                "binary args carry data",
                dx_binary_forward_kwargs({"args": ["--x"], "data": [":d"]}),
                {"args": ["--x"], "data": [":d"]},
            ),
            expect_equal(
                "binary data alone stays upstream",
                dx_binary_forward_kwargs({"data": [":d"]}),
                {},
            ),
            expect_equal(
                "binary none args stay empty",
                dx_binary_forward_kwargs({"args": None, "data": [":d"]}),
                {},
            ),
            expect_equal(
                "binary target_compatible_with rides forwarder",
                dx_binary_forward_kwargs({"target_compatible_with": ["@platforms//os:linux"]}),
                {"target_compatible_with": ["@platforms//os:linux"]},
            ),
            expect_equal(
                "binary none tags stay empty",
                dx_binary_forward_kwargs({"tags": None}),
                {},
            ),
            expect_equal(
                "test upstream empty stays private",
                dx_test_upstream_kwargs({}),
                {"tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream keeps manual-only tags",
                dx_test_upstream_kwargs({"tags": ["manual"]}),
                {"tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream keeps non-manual tags",
                dx_test_upstream_kwargs({"tags": ["manual", "cpu:4"]}),
                {"tags": ["manual", "cpu:4"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream adds manual",
                dx_test_upstream_kwargs({"tags": ["cpu:4"]}),
                {"tags": ["cpu:4", "manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream forces private visibility",
                dx_test_upstream_kwargs({"visibility": ["//visibility:public"]}),
                {"tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream sets srcs when given",
                dx_test_upstream_kwargs({}, ["hello_test.go"]),
                {"tags": ["manual"], "visibility": ["//visibility:private"], "srcs": ["hello_test.go"]},
            ),
            expect_equal(
                "test upstream keeps compiler flags",
                dx_test_upstream_kwargs({"copts": ["-Werror"]}),
                {"copts": ["-Werror"], "tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream rewrites skip tags and adds manual",
                dx_test_upstream_kwargs({"tags": ["no-lint", "cpu:4"]})["tags"],
                ["dx:no-lint", "cpu:4", "manual"],
            ),
            expect_equal(
                "test upstream rewrites skip tags after manual",
                dx_test_upstream_kwargs({"tags": ["manual", "no-format", "no-remote-exec"]})["tags"],
                ["manual", "dx:no-format", "no-remote-exec"],
            ),
            expect_equal(
                "test upstream sets srcs and rewrites skip tags",
                test_output,
                {"srcs": ["b_test.go"], "tags": ["dx:no-lint", "manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream leaves its input unchanged",
                test_input,
                {"srcs": ["a_test.go"], "tags": ["no-lint"]},
            ),
            expect_equal(
                "binary upstream empty stays private",
                dx_binary_upstream_kwargs({}, []),
                {"tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "binary upstream sets srcs when given",
                dx_binary_upstream_kwargs({"srcs": ["old.go"]}, ["main.go"]),
                {"srcs": ["main.go"], "tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "binary upstream drops srcs when empty",
                dx_binary_upstream_kwargs({"srcs": ["old.go"]}, []),
                {"tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "binary upstream drops srcs when none",
                dx_binary_upstream_kwargs({"srcs": ["old.go"]}, None),
                {"tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "library forward empty stays empty",
                dx_wrap_forward_kwargs({}),
                {},
            ),
            expect_equal(
                "library forward keeps hints, tags, testonly and compatibility",
                dx_wrap_forward_kwargs({
                    "aspect_hints": ["//hints:a"],
                    "tags": ["no-lint"],
                    "target_compatible_with": ["@platforms//os:linux"],
                    "testonly": True,
                }),
                {
                    "aspect_hints": ["//hints:a"],
                    "tags": ["no-lint"],
                    "target_compatible_with": ["@platforms//os:linux"],
                    "testonly": True,
                },
            ),
            expect_equal(
                "library forward keeps select compatibility",
                dx_wrap_forward_kwargs({"target_compatible_with": select({"//conditions:default": ["@platforms//:incompatible"]})}).keys(),
                ["target_compatible_with"],
            ),
            expect_equal(
                "library forward drops hdrs and copts",
                dx_wrap_forward_kwargs({"copts": ["-Werror"], "hdrs": ["a.h"], "testonly": False}),
                {"testonly": False},
            ),
            expect_equal(
                "library forward drops none tags",
                dx_wrap_forward_kwargs({"tags": None}),
                {},
            ),
            expect_equal(
                "binary upstream rewrites skip tags and keeps manual",
                dx_binary_upstream_kwargs({"tags": ["manual", "no-lint", "requires-network"]}, [])["tags"],
                ["manual", "dx:no-lint", "requires-network"],
            ),
            expect_equal(
                "binary upstream appends manual after user tags",
                dx_binary_upstream_kwargs({"tags": ["no-format", "cpu:4"]}, [])["tags"],
                ["dx:no-format", "cpu:4", "manual"],
            ),
            expect_equal(
                "binary upstream forces private visibility and keeps copts",
                binary_output,
                {"copts": ["-Werror"], "tags": ["dx:no-lint", "manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "binary upstream leaves its input unchanged",
                binary_input,
                {"copts": ["-Werror"], "srcs": ["a.go"], "tags": ["no-lint"], "visibility": ["//visibility:public"]},
            ),
            expect_equal(
                "test forward empty stays empty",
                dx_test_forward_kwargs({}),
                {},
            ),
            expect_equal(
                "test forward keeps manual",
                dx_test_forward_kwargs({"tags": ["manual", "cpu:4"]}),
                {"tags": ["manual", "cpu:4"]},
            ),
            expect_equal(
                "test forward keeps manual-only",
                dx_test_forward_kwargs({"tags": ["manual"]}),
                {"tags": ["manual"]},
            ),
            expect_equal(
                "test forward keeps timeout and flaky",
                dx_test_forward_kwargs({"timeout": "short", "flaky": True}),
                {"timeout": "short", "flaky": True},
            ),
            expect_equal(
                "test forward keeps flaky",
                dx_test_forward_kwargs({"flaky": True}),
                {"flaky": True},
            ),
            expect_equal(
                "test forward keeps args",
                dx_test_forward_kwargs({"args": ["--gtest_filter=Foo.*"]}),
                {"args": ["--gtest_filter=Foo.*"]},
            ),
            expect_equal(
                "test forward keeps data with args",
                dx_test_forward_kwargs({"args": ["$(rootpath :d)"], "data": [":d"]}),
                {"args": ["$(rootpath :d)"], "data": [":d"]},
            ),
            expect_equal(
                "test forward drops data without args",
                dx_test_forward_kwargs({"data": [":d"]}),
                {},
            ),
            expect_equal(
                "test forward keeps env upstream",
                dx_test_forward_kwargs({"env": {"K": "1"}, "env_inherit": ["HOME"]}),
                {},
            ),
            expect_equal(
                "test forward rides hints",
                dx_test_forward_kwargs({"aspect_hints": ["//quality:hint"]}),
                {"aspect_hints": ["//quality:hint"]},
            ),
            expect_equal(
                "test forward rides exec_properties",
                dx_test_forward_kwargs({"exec_properties": {"Pool": "gpu"}}),
                {"exec_properties": {"Pool": "gpu"}},
            ),
            expect_equal(
                "test forward drops other exec group properties",
                dx_test_forward_kwargs({"exec_properties": {"cpp_link.Pool": "c"}}),
                {},
            ),
            expect_equal(
                "test forward keeps test and unprefixed properties",
                dx_test_forward_kwargs({"exec_properties": {"Pool": "a", "test.Pool": "b", "cpp_link.Pool": "c"}}),
                {"exec_properties": {"Pool": "a", "test.Pool": "b"}},
            ),
            expect_equal(
                "test forward rides exec_compatible_with",
                dx_test_forward_kwargs({"exec_compatible_with": ["@platforms//os:linux"]}),
                {"exec_compatible_with": ["@platforms//os:linux"]},
            ),
            expect_equal(
                "test forward rides exec_group_compatible_with",
                dx_test_forward_kwargs({"exec_group_compatible_with": {"test": ["@platforms//os:linux"], "cpp_link": ["@platforms//cpu:x86_64"]}}),
                {"exec_group_compatible_with": {"test": ["@platforms//os:linux"]}},
            ),
            expect_equal(
                "test forward keeps upstream-only exec groups upstream",
                dx_test_forward_kwargs({"exec_group_compatible_with": {"cpp_link": ["@platforms//cpu:x86_64"]}}),
                {},
            ),
            expect_equal(
                "test upstream keeps exec groups",
                dx_test_upstream_kwargs({"exec_group_compatible_with": {"test": ["@platforms//os:linux"], "cpp_link": ["@platforms//cpu:x86_64"]}}),
                {"exec_group_compatible_with": {"test": ["@platforms//os:linux"], "cpp_link": ["@platforms//cpu:x86_64"]}, "tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test forward leaves exec groups input unchanged",
                _forwarded_exec_groups_input(),
                {"exec_group_compatible_with": {"test": ["@platforms//os:linux"], "cpp_link": ["@platforms//cpu:x86_64"]}},
            ),
            expect_equal(
                "test forward rides local",
                dx_test_forward_kwargs({"local": True}),
                {"local": True},
            ),
            expect_equal(
                "test forward keeps exec placement, drops copts",
                dx_test_forward_kwargs({"local": True, "exec_properties": {"Pool": "gpu"}, "copts": ["-Werror"]}),
                {"local": True, "exec_properties": {"Pool": "gpu"}},
            ),
            expect_equal(
                "test upstream keeps exec placement",
                dx_test_upstream_kwargs({"exec_properties": {"Pool": "gpu"}}),
                {"exec_properties": {"Pool": "gpu"}, "tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test upstream keeps env",
                dx_test_upstream_kwargs({"env": {"K": "1"}, "env_inherit": ["HOME"]}),
                {"env": {"K": "1"}, "env_inherit": ["HOME"], "tags": ["manual"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "test forward drops non-test attrs",
                dx_test_forward_kwargs({"copts": ["-Werror"]}),
                {},
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
            expect_equal(
                "upstream basenames map onto the forwarder name",
                [
                    dx_symlink_executable_name("hello", struct(basename = "hello_upstream")),
                    dx_symlink_executable_name("hello", struct(basename = "hello_upstream.exe")),
                ],
                ["hello", "hello.exe"],
            ),
            expect_equal(
                "dotted target names keep the whole name",
                [
                    dx_symlink_executable_name("tool.v2", struct(basename = "tool.v2_upstream")),
                    dx_symlink_executable_name("tool.v2", struct(basename = "tool.v2_upstream.exe")),
                    dx_symlink_executable_name("tool.v2", struct(basename = "tool.v2_upstream.bat")),
                ],
                ["tool.v2", "tool.v2.exe", "tool.v2.bat"],
            ),
            expect_equal(
                "other executables keep their own suffix",
                dx_symlink_executable_name("deploy", struct(basename = "push.sh")),
                "deploy.sh",
            ),
            expect_equal(
                "a longer upstream name is not a match",
                dx_symlink_executable_name("hello", struct(basename = "hello_upstream2")),
                "hello",
            ),
        ],
    )

def wrapper_resource_tags_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "forward kwargs add resource tags to empty tags",
                dx_test_forward_kwargs({"size": "large", "tags": []})["tags"],
                ["resources:cpu:2", "resources:memory:2048"],
            ),
            expect_equal(
                "forward kwargs add resource tags after user tags",
                dx_test_forward_kwargs({"size": "large", "tags": ["x"]})["tags"],
                ["x", "resources:cpu:2", "resources:memory:2048"],
            ),
            expect_equal(
                "forward kwargs keep small empty tags empty",
                dx_test_forward_kwargs({"size": "small", "tags": []})["tags"],
                [],
            ),
            expect_equal(
                "upstream kwargs keep size and hints and mark manual",
                dx_test_upstream_kwargs({"tags": ["x"], "size": "large", "aspect_hints": ["//h"]}, None),
                {"tags": ["x", "manual"], "size": "large", "aspect_hints": ["//h"], "visibility": ["//visibility:private"]},
            ),
            expect_equal(
                "small tests declare nothing",
                dx_test_resource_tags("small", ["a"]),
                ["a"],
            ),
            expect_equal(
                "unsized tests declare nothing",
                dx_test_resource_tags(None, None),
                [],
            ),
            expect_equal(
                "large tests declare two cpus and 2 GB",
                dx_test_resource_tags("large", ["a"]),
                ["a", "resources:cpu:2", "resources:memory:2048"],
            ),
            expect_equal(
                "enormous tests declare four cpus and 4 GB",
                dx_test_resource_tags("enormous", None),
                ["resources:cpu:4", "resources:memory:4096"],
            ),
            expect_equal(
                "explicit resource tags win",
                dx_test_resource_tags("large", ["resources:memory:512", "resources:cpu:1"]),
                ["resources:memory:512", "resources:cpu:1"],
            ),
            expect_equal(
                "a lone nested_bazel tag keeps the default cpu and memory",
                dx_test_resource_tags("large", ["resources:nested_bazel:1"]),
                ["resources:nested_bazel:1", "resources:cpu:2", "resources:memory:2048"],
            ),
            expect_equal(
                "a lone cpu tag keeps the default memory",
                dx_test_resource_tags("large", ["resources:cpu:8"]),
                ["resources:cpu:8", "resources:memory:2048"],
            ),
            expect_equal(
                "forwarded large tests carry the resource tags",
                dx_test_forward_kwargs({"size": "large", "tags": ["a"]}),
                {"size": "large", "tags": ["a", "resources:cpu:2", "resources:memory:2048"]},
            ),
            expect_equal(
                "nested Bazel tests keep 2 cpus and 2560 MB through the test wrapper",
                [t for t in dx_test_forward_kwargs({"size": "large", "tags": NESTED_BAZEL_TAGS})["tags"] if t.startswith("resources:")],
                ["resources:cpu:2", "resources:memory:2560", "resources:nested_bazel:1"],
            ),
            expect_equal(
                "bundler actions grow on macOS",
                [dx_bundler_resources("linux", 0), dx_bundler_resources("osx", 0)],
                [{"cpu": 1, "memory": 1536}, {"cpu": 1, "memory": 2048}],
            ),
            expect_equal(
                "light quality actions grow on macOS",
                [dx_quality_resource_set(["light"])("linux", 0), dx_quality_resource_set(["light"])("osx", 0)],
                [{"cpu": 1, "memory": 512}, {"cpu": 1, "memory": 1024}],
            ),
            expect_equal(
                "quality actions take the costliest tier",
                [
                    dx_quality_resource_set(["light", "heavy"])("linux", 0),
                    dx_quality_resource_set(["medium", "light"])("linux", 0),
                ],
                [{"cpu": 2, "memory": 2048}, {"cpu": 1, "memory": 1024}],
            ),
            expect_equal(
                "quality actions with no launched tool are light",
                dx_quality_resource_set([])("linux", 0),
                {"cpu": 1, "memory": 512},
            ),
        ],
    )
