# Java

This directory helps you build, run and test Java code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//java:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Java page](../docs/languages/java.md) shows
setup, every rule and the tools dx runs.

Run the Java tests from a rules_dx checkout:

```sh
tools/dx test //java/...
```

## Dependencies

List the artifacts in a rules_jvm_external `maven_install.json` and run
`dx generate --apply`. It writes the targets and their `deps`. Each artifact
is the label `@maven//:<group>_<artifact>`, with dots and dashes turned into
underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

### Writing targets

Warnings fail the build unless a target turns that off.

`java_binary` and `java_test` work out `main_class` and `test_class` from the
sources and the package path. Set them to override. Without `srcs`, or with
`srcs` in a `select()`, the class comes from the package path. When the
package path has no Java root, the build fails and asks you to set
`main_class` or `test_class`.

## Quality

| Check | Default tools |
| --- | --- |
| Format | `google_java_format` |
| Lint | `pmd`, `spotbugs`, `typos` |
| Typecheck | `javac` |

`pmd` reads a ruleset bound with `pmd_config`. Without one, it reports unused
private methods, fields, locals, parameters and assignments as info. See the
[quality config table](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/quality.md).

## Bench and fuzz

`java_bench` runs JMH benchmarks for `dx bench`. rules_dx brings JMH, so you
add no Maven entries.

```python
load("@rules_dx//java:defs.bzl", "java_bench")

java_bench(
    name = "sum_bench",
    srcs = ["Sum.java"],
)
```

The test is tagged `dx-bench` and runs with your other tests. It writes
`dx-bench.json` with one row per `@Benchmark` method. Each benchmark runs one
warmup and three measured iterations of 200 ms in the test JVM, without
forking.

`size`, `timeout`, `args`, `env`, `flaky`, `jvm_flags`, `data` and
`visibility` go to the test. The other attributes go to the library that
compiles `srcs`. `tags` and `target_compatible_with` go to both.

`kotlin_bench` from `@rules_dx//kotlin:defs.bzl` and `scala_bench` from
`@rules_dx//scala:defs.bzl` take Kotlin and Scala sources the same way.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
