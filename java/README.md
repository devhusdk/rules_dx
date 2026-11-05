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
