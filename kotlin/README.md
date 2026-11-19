# Kotlin

This directory helps you build, run and test Kotlin code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//kotlin:defs.bzl`.
The tests live in `tests/`. The [Kotlin page](../docs/languages/kotlin.md) shows
setup, every rule and the tools dx runs.

Run the Kotlin tests from a rules_dx checkout:

```sh
tools/dx test //kotlin/...
```

## Dependencies

### Internal members

To use the `internal` members of a `kotlin_library`, list it in `associates`.
The target then joins the Kotlin module of that library:

```python
kotlin_library(
    name = "greet_testing",
    srcs = ["GreetTesting.kt"],
    associates = [":greet"],
)
```

### Third-party dependencies

List the artifacts in a rules_jvm_external `maven_install.json` and run
`dx generate --apply`. It writes the targets and their `deps`. Each artifact
is the label `@maven//:<group>_<artifact>`, with dots and dashes turned into
underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

### Writing targets

Warnings fail the build unless a target turns that off.

`kotlin_test` works out `test_class` from the sources and the package path.
Set it to override. Other attributes pass through to `kt_jvm_library`,
`kt_jvm_binary` and `kt_jvm_test`.

## Quality

| Check | Default tools |
| --- | --- |
| Format | `ktfmt` |
| Lint | `detekt`, `typos` |
| Typecheck | `kotlinc` |

None of them needs a config file.

## Bench and fuzz

`kotlin_fuzz_test` builds a Jazzer fuzz target for `dx fuzz`, like
`java_fuzz_test`:

```python
load("@rules_dx//kotlin:defs.bzl", "kotlin_fuzz_test")

kotlin_fuzz_test(
    name = "parse_fuzz",
    srcs = ["ParseFuzz.kt"],
    target_class = "pkg.ParseFuzz",
)
```

The target class needs a static `fuzzerTestOneInput(data: ByteArray)`. Put it
in an `object` and mark it `@JvmStatic`.

`kotlin_bench` runs JMH benchmarks for `dx bench`, like `java_bench`.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
