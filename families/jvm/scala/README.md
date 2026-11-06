# Scala

Build, run and test Scala with `rules_scala`. Warnings fail the build unless a target turns that off.

```python
load("@rules_dx//scala/rules:defs.bzl", "scala_binary", "scala_library", "scala_test")

scala_library(
    name = "greet",
    srcs = ["Greet.scala"],
)

scala_binary(
    name = "hello",
    srcs = ["Hello.scala"],
    main_class = "demo.Hello",
    deps = [":greet"],
)

scala_test(
    name = "GreetTest",
    srcs = ["GreetTest.scala"],
    deps = [":greet"],
)
```

Other attributes pass through to `rules_scala`.

## Third-party dependencies

List the artifacts in a rules_jvm_external `maven_install.json` and run
`dx generate --apply`. The `scala` language of `dx generate` writes the targets and `deps`.
A dependency is the label `@maven//:<group>_<artifact>`, with dots and dashes
turned into underscores, for example `@maven//:com_google_guava_guava`. See
[generate](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/generate.md).

## Format, lint and typecheck

By default `dx format` runs `scalafmt` and `dx typecheck` runs `scalac`. No
lint tool is on by default. `scalafmt` needs a `.scalafmt.conf` bound with
`scalafmt_config`. See the
[quality config table](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/quality.md).

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
