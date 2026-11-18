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

## Writing targets

Warnings fail the build unless a target turns that off.

`kotlin_test` infers `test_class` from the sources and the package path. Set
it to override. Other attributes pass through to `kt_jvm_library`,
`kt_jvm_binary` and `kt_jvm_test`.

## Dependencies

### Internal members

List a `kotlin_library` in `associates` to use its `internal` members. The target joins the Kotlin module of that library.

```python
kotlin_library(
    name = "greet_testing",
    srcs = ["GreetTesting.kt"],
    associates = [":greet"],
)
```

### Third-party dependencies

List the artifacts in a rules_jvm_external `maven_install.json` and run
`dx generate --apply`. The `kotlin` language of `dx generate` writes the targets and `deps`.
A dependency is the label `@maven//:<group>_<artifact>`, with dots and dashes
turned into underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

## Quality

By default `dx format` runs `ktfmt`, `dx lint` runs `detekt` and `dx typecheck`
runs `kotlinc`. None of them needs a config file.

## Bench and fuzz

`kotlin_fuzz_test` builds a Jazzer fuzz target for `dx fuzz`, like `java_fuzz_test`.

```python
load("@rules_dx//kotlin:defs.bzl", "kotlin_fuzz_test")

kotlin_fuzz_test(
    name = "parse_fuzz",
    srcs = ["ParseFuzz.kt"],
    target_class = "pkg.ParseFuzz",
)
```

The target class needs a static `fuzzerTestOneInput(data: ByteArray)`, so put it in an `object` with `@JvmStatic`.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
