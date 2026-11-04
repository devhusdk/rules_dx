# Julia

Build, run and test Julia with the pinned `rules_julia` 0.8.0 and Julia 1.12.5.
Nothing needs to be installed.

## Start

Run `dx init --apply demo-app` and add sources; see `tests/fixtures/julia-app`. Name
each library after its module and keep the source in `src/<label>.jl`: that is the layout rules_julia needs
to precompile. Libraries precompile at build time, so a syntax error fails
`bazel build`.

```python
load("@rules_dx//julia/rules:defs.bzl", "julia_binary", "julia_library", "julia_test")

julia_library(
    name = "Greet",
    srcs = ["Greet.jl"],
)

julia_binary(
    name = "hello",
    main = "main.jl",
    deps = [":Greet"],
)

julia_test(
    name = "greet_test",
    main = "greet_test.jl",
    deps = [":Greet"],
)
```

The wrappers forward to `rules_julia` and add the `julia` quality class.
[`julia-app`](tests/fixtures/julia-app/README.md) is a multi-package app
with a lockfile hub, a `Bzip2_jll` binary dependency, runfiles data and a
diagnostics package.

`julia_standalone_binary(name, binary)` compiles a `julia_library` into a
system image and a launcher that runs after the runfiles are copied out of
`bazel-bin`. The library's module must define `julia_main()::Cint` and return
the exit code. See `tests/fixtures/hello/app`.

## Generate

`dx generate` reads `Project.toml` only. A `Project.toml` with a `name` and a
`src/<name>.jl` gets a `julia_library` named `<name>` from `src/**/*.jl`, with
`Project.toml` as `data` and `[deps]` as dependencies. When `test/runtests.jl`
exists it also gets a `julia_test` named `<name>_test`. It never generates a
`julia_binary`, `ext/` or `[weakdeps]`, and it leaves a `Project.toml` without
a name alone. Third-party packages need a `# gazelle:resolve julia` line.

## Formatting

`dx format` runs JuliaFormatter 2.2.0 when a `julia` family selects
`julia_format`. The formatter and its packages are fetched only for Julia
targets. Syntax errors are reported with line and column and are never written.

## Type checking

`dx typecheck` runs JET 0.13.1 when a `julia` family selects `jet`.
`--@rules_dx//config:tool_jet` overrides the pinned binary. Findings
are reported at their line. See `docs/cli/commands/quality.md`.

## Platforms

| Platform | Support |
| --- | --- |
| linux_x86_64 | yes |
| linux_arm64 | yes |
| macos_arm64 | yes |
| windows_x86_64 | yes |
| windows_arm64 | no Julia build exists |

## Remote execution

Julia package precompiles fail under remote execution for packages with
non-stdlib dependencies. Build the formatter locally with `--remote_executor=`.
