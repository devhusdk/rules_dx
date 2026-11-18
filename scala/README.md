# Scala

This directory helps you build, run and test Scala code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//scala:defs.bzl`.
The tests live in `tests/`. The [Scala page](../docs/languages/scala.md) shows
setup, every rule and the tools dx runs.

Run the Scala tests from a rules_dx checkout:

```sh
tools/dx test //scala/...
```

## Dependencies

List the artifacts in a rules_jvm_external `maven_install.json` and run
`dx generate --apply`. The `scala` language of `dx generate` writes the targets and `deps`.
A dependency is the label `@maven//:<group>_<artifact>`, with dots and dashes
turned into underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

### Writing targets

Warnings fail the build unless a target turns that off. Other attributes pass
through to `rules_scala`.

## Quality

By default `dx format` runs `scalafmt` and `dx typecheck` runs `scalac`. No
lint tool is on by default. `scalafmt` needs a `.scalafmt.conf` bound with
`scalafmt_config`. See the
[quality config table](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/quality.md).

## Bench and fuzz

### Fuzzing

`scala_fuzz_test` builds a Jazzer fuzz target for `dx fuzz`, like `java_fuzz_test`.

```python
load("@rules_dx//scala:defs.bzl", "scala_fuzz_test")

scala_fuzz_test(
    name = "parse_fuzz",
    srcs = ["ParseFuzz.scala"],
    target_class = "pkg.ParseFuzz",
)
```

The target class needs a static `fuzzerTestOneInput(data: Array[Byte])`, which a top-level `object` method provides.

### Benchmarks

`scala_bench` runs JMH benchmarks and writes `dx-bench.json` for `dx bench`, like `java_bench`.

```python
load("@rules_dx//scala:defs.bzl", "scala_bench")

scala_bench(
    name = "sum_bench",
    srcs = ["Sum.scala"],
)
```

scalac runs no Java annotation processors, so the rule compiles the sources, runs the JMH bytecode generator (reflection mode) over the classes, and compiles the generated Java. Benchmark classes must be non-final classes with a public no-argument constructor, not `object`s.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
