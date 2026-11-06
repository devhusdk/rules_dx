# Kotlin

Build, run and test Kotlin on the JVM with `rules_kotlin`. Warnings fail the build unless a target turns that off.

```python
load("@rules_dx//kotlin/rules:defs.bzl", "kotlin_binary", "kotlin_library", "kotlin_test")

kotlin_library(
    name = "greet",
    srcs = ["Greet.kt"],
)

kotlin_binary(
    name = "hello",
    srcs = ["Hello.kt"],
    main_class = "demo.HelloKt",
    deps = [":greet"],
)

kotlin_test(
    name = "GreetTest",
    srcs = ["GreetTest.kt"],
    deps = [":greet"],
)
```

`kotlin_test` infers `test_class` from the sources and the package path. Set it to override. Other attributes pass through to `kt_jvm_library`, `kt_jvm_binary` and `kt_jvm_test`.

## Third-party dependencies

List the artifacts in a rules_jvm_external `maven_install.json` and run
`dx generate --apply`. The `kotlin` language of `dx generate` writes the targets and `deps`.
A dependency is the label `@maven//:<group>_<artifact>`, with dots and dashes
turned into underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

## Format, lint and typecheck

By default `dx format` runs `ktfmt`, `dx lint` runs `detekt` and `dx typecheck`
runs `kotlinc`. None of them needs a config file.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
