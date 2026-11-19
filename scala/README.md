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
`dx generate --apply`. It writes the targets and their `deps`. Each artifact
is the label `@maven//:<group>_<artifact>`, with dots and dashes turned into
underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

### Writing targets

Warnings fail the build unless a target turns that off. Other attributes pass
through to `rules_scala`.

## Quality

| Check | Default tools |
| --- | --- |
| Format | `scalafmt` |
| Lint | `typos` |
| Typecheck | `scalac` |

`scalafmt` needs a `.scalafmt.conf` bound with `scalafmt_config`. See the
[quality config table](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/quality.md).

## Bench and fuzz

### Fuzzing

`scala_fuzz_test` builds a Jazzer fuzz target for `dx fuzz`, like
`java_fuzz_test`:

```python
load("@rules_dx//scala:defs.bzl", "scala_fuzz_test")

scala_fuzz_test(
    name = "parse_fuzz",
    srcs = ["ParseFuzz.scala"],
    target_class = "pkg.ParseFuzz",
)
```

The target class needs a static `fuzzerTestOneInput(data: Array[Byte])`. A
method on a top-level `object` is static.

### Benchmarks

`scala_bench` runs JMH benchmarks and writes `dx-bench.json` for `dx bench`,
like `java_bench`:

```python
load("@rules_dx//scala:defs.bzl", "scala_bench")

scala_bench(
    name = "sum_bench",
    srcs = ["Sum.scala"],
)
```

Write each benchmark as a non-final `class` with a public no-argument
constructor, not as an `object`. scalac runs no Java annotation processors,
so the rule runs the JMH bytecode generator over the compiled classes and
compiles the Java it writes.

## API docs

`dx docs` documents Scala 2.13 libraries only. On any other Scala version,
`dx docs` skips your Scala libraries, and `scala_doc_shard` fails with
`only Scala 2.13 is supported`.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
