"""Private Jazzer helper for java_fuzz_test, kotlin_fuzz_test and scala_fuzz_test."""

load("@rules_java//java:defs.bzl", _java_binary = "java_binary")
load("//libs/starlark:fuzz.bzl", "dx_fuzz_runner")
load("//libs/starlark:fuzz_bench.bzl", "merge_tags", "split_runner_kwargs")

def jazzer_fuzz(name, library, srcs, deps, target_class, visibility, tags, kwargs, library_kwargs = {}):
    """Declares the fuzz library, the Jazzer binary and the `<name>_run` runner. library is the language's library macro."""
    build, run = split_runner_kwargs(kwargs)
    library(
        name = name + "_bin",
        srcs = srcs,
        deps = deps,
        testonly = True,
        tags = merge_tags(tags, "manual"),
        visibility = ["//visibility:private"],
        **dict(build, **library_kwargs)
    )
    _java_binary(
        name = name,
        main_class = "com.code_intelligence.jazzer.Jazzer",
        runtime_deps = [":" + name + "_bin", Label("@dx_jazzer//jar"), Label("@dx_jazzer_api//jar")],
        testonly = True,
        tags = merge_tags(tags, "dx-fuzz", "manual"),
        visibility = visibility,
        **run
    )
    dx_fuzz_runner(name, ":" + name, visibility = visibility, engine = "jazzer", target = target_class)
