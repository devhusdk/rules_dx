# Java

Build, run and test Java. Warnings fail the build unless a target turns that off.

```python
load("@rules_dx//java/rules:defs.bzl", "java_binary", "java_library", "java_test")

java_library(
    name = "greet",
    srcs = ["Greet.java"],
)

java_binary(
    name = "hello",
    srcs = ["Hello.java"],
    deps = [":greet"],
)

java_test(
    name = "GreetTest",
    srcs = ["GreetTest.java"],
    deps = [":greet"],
)
```

`java_binary` and `java_test` infer `main_class` and `test_class` from the sources and the package path. Set them to override.

## Third-party dependencies

List the artifacts in a rules_jvm_external `maven_install.json` and run
`dx generate --apply`. The `java` language of `dx generate` writes the targets and `deps`.
A dependency is the label `@maven//:<group>_<artifact>`, with dots and dashes
turned into underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

## Format, lint and typecheck

By default `dx format` runs `google_java_format`, `dx lint` runs `checkstyle`,
`pmd` and `spotbugs`, and `dx typecheck` runs `javac`. `checkstyle` needs a
`checkstyle.xml` bound with `checkstyle_config`. `pmd` reads a ruleset bound
with `pmd_config`. Without one it reports unused private methods, fields, locals,
parameters and assignments as info. See the
[quality config table](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/quality.md).

## Platforms

See the [support matrix](../docs/support-matrix.md).

## Benchmarks

`java_benchmark` runs JMH benchmarks for `dx bench`. List `org.openjdk.jmh:jmh-core` and `jmh-generator-annprocess` in `maven_install.json`.

```python
load("@rules_dx//java/rules:bench.bzl", "java_benchmark")

java_benchmark(
    name = "sum_bench",
    srcs = ["Sum.java"],
)
```

The test is tagged `dx-bench` and `manual` and writes `dx-bench.json` with one row per `@Benchmark` method. It runs one warmup and three measurement iterations of 200 ms in the test JVM, without forking. Set `jmh` and `jmh_processor` to other labels if your artifacts are not in `@maven`. `kotlin_benchmark` in `@rules_dx//kotlin/rules:bench.bzl` takes Kotlin sources the same way.
