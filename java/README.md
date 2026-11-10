# Java

Build, run and test Java. Warnings fail the build unless a target turns that off.

```python
load("@rules_dx//java:defs.bzl", "java_binary", "java_library", "java_test")

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

`java_binary` and `java_test` infer `main_class` and `test_class` from the sources and the package path. Set them to override. Without `srcs`, or with `select()` srcs, the class comes from the package path; if the package path has no Java root, the build fails and asks for `test_class` or `main_class`.

## Third-party dependencies

List the artifacts in a rules_jvm_external `maven_install.json` and run
`dx generate --apply`. The `java` language of `dx generate` writes the targets and `deps`.
A dependency is the label `@maven//:<group>_<artifact>`, with dots and dashes
turned into underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

## Format, lint and typecheck

By default `dx format` runs `google_java_format`, `dx lint` runs `pmd`,
`spotbugs` and `typos`, and `dx typecheck` runs `javac`. `pmd` reads a ruleset bound
with `pmd_config`. Without one it reports unused private methods, fields, locals,
parameters and assignments as info. See the
[quality config table](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/quality.md).

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).

## Benchmarks

`java_bench` runs JMH benchmarks for `dx bench`. JMH 1.37 comes with rules_dx, so no Maven hub entries are needed.

```python
load("@rules_dx//java:defs.bzl", "java_bench")

java_bench(
    name = "sum_bench",
    srcs = ["Sum.java"],
)
```

The test is tagged `dx-bench` and `manual` and writes `dx-bench.json` with one row per `@Benchmark` method. It runs one warmup and three measurement iterations of 200 ms in the test JVM, without forking. `kotlin_bench` in `@rules_dx//kotlin:defs.bzl` and `scala_bench` in `@rules_dx//scala:defs.bzl` take Kotlin and Scala sources the same way.
