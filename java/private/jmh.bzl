"""Private JMH helpers for java_bench, kotlin_bench and scala_bench."""

load("@rules_java//java:defs.bzl", "JavaInfo", "java_import", _java_library = "java_library", _java_plugin = "java_plugin")
load("//core:fuzz_bench.bzl", "DEFAULT_SIZE", "merge_tags", "split_test_kwargs")
load("//core:wrapper.bzl", "dx_upstream_tags")

_JMH = [Label("@dx_jmh_core//jar"), Label("@dx_jmh_jopt_simple//jar"), Label("@dx_jmh_commons_math3//jar")]
_GENERATOR = Label("//java/private/jmh:jmh_generate")
_PROCESSOR = [Label("@dx_jmh_generator_annprocess//jar")] + _JMH
_TEST_KWARGS = ("size", "timeout", "args", "env", "flaky", "jvm_flags", "data", "visibility")

def _split(tags, kwargs, library_kwargs):
    """Returns (library, run, inner): the kwargs of <name>_bin, of the test, and of every private helper target."""
    library, run = split_test_kwargs(kwargs, _TEST_KWARGS)
    inner = {"tags": dx_upstream_tags(tags), "testonly": True, "visibility": ["//visibility:private"]}
    compatible = library.pop("target_compatible_with", None)
    if compatible != None:
        inner["target_compatible_with"] = compatible
        run["target_compatible_with"] = compatible
    library = dict(library_kwargs) | library | dict(inner, tags = tags)
    library.setdefault("warnings_as_errors", False)
    return library, run, inner

def jmh_bench(name, library, test, srcs, deps, tags, kwargs, library_kwargs = {}):
    """Declares the JMH library, the runner and the dx-bench test. library is the java_library or kotlin_library macro, test is java_jmh_test."""
    library_kwargs, run, inner = _split(tags, kwargs, library_kwargs)
    processor = name + "_jmh_processor"
    _java_plugin(
        name = processor,
        processor_class = "org.openjdk.jmh.generators.BenchmarkProcessor",
        deps = _PROCESSOR,
        **inner
    )
    library(
        name = name + "_bin",
        srcs = srcs,
        deps = deps + _JMH,
        plugins = [processor] + library_kwargs.pop("plugins", []),
        **library_kwargs
    )
    _runner_and_test(name, test, [name + "_bin"], tags, run, inner)

def _runner_and_test(name, test, runtime_deps, tags, run, inner):
    _java_library(
        name = name + "_jmh_runner",
        srcs = [Label("//java/private/jmh:JmhRunner.java")],
        deps = _JMH,
        **inner
    )
    test(
        name,
        runtime_deps = runtime_deps + [name + "_jmh_runner"],
        tags = merge_tags(tags, "dx-bench"),
        testonly = True,
        **dict(run, size = run.get("size", DEFAULT_SIZE))
    )

def _jmh_generated_impl(ctx):
    java = ctx.attr.lib[JavaInfo]
    jars = depset(java.runtime_output_jars, transitive = [java.transitive_runtime_jars])
    ctx.actions.run(
        executable = ctx.executable._generator,
        arguments = [
            java.runtime_output_jars[0].path,
            ctx.outputs.srcjar.path,
            ctx.outputs.resources.path,
            ctx.actions.args().add_all(jars),
        ],
        inputs = jars,
        outputs = [ctx.outputs.srcjar, ctx.outputs.resources],
        mnemonic = "JmhGenerate",
        progress_message = "Generating JMH benchmarks for %{label}",
    )

_jmh_generated = rule(
    implementation = _jmh_generated_impl,
    doc = "Runs the JMH bytecode generator over a library jar into a source jar and resources.",
    attrs = {
        "lib": attr.label(providers = [JavaInfo], mandatory = True, doc = "The compiled benchmark library."),
        "srcjar": attr.output(mandatory = True, doc = "The generated Java sources."),
        "resources": attr.output(mandatory = True, doc = "The generated JMH resources."),
        "_generator": attr.label(default = _GENERATOR, executable = True, cfg = "exec"),
    },
)

def jmh_bench_generated(name, library, test, srcs, deps, tags, kwargs, library_kwargs = {}):
    """Like jmh_bench for languages whose compiler runs no Java annotation processors. The library compiles without the processor, JmhGenerate runs the JMH bytecode generator over its jar, and javac compiles the generated Java."""
    library_kwargs, run, inner = _split(tags, kwargs, library_kwargs)
    library(
        name = name + "_bin",
        srcs = srcs,
        deps = deps + _JMH[:1],
        **library_kwargs
    )
    _jmh_generated(
        name = name + "_jmh_gen",
        lib = name + "_bin",
        srcjar = name + "_jmh.srcjar",
        resources = name + "_jmh_resources.jar",
        **inner
    )
    _java_library(
        name = name + "_jmh_lib",
        srcs = [name + "_jmh.srcjar"],
        deps = [name + "_bin"] + _JMH,
        **inner
    )
    java_import(
        name = name + "_jmh_resources",
        jars = [name + "_jmh_resources.jar"],
        **inner
    )
    _runner_and_test(name, test, [name + "_bin", name + "_jmh_lib", name + "_jmh_resources"], tags, run, inner)
