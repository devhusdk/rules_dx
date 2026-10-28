# Julia

Build, run and test Julia with the pinned `rules_julia` 0.8.0 and Julia 1.12.5.
Nothing needs to be installed.

## Start

`dx new --apply julia demo-app` scaffolds a package with `DemoApp`,
`DemoApp_bin` and `DemoApp_test` targets. The library is named after the
module because rules_julia only precompiles when the label matches
`src/<label>.jl`.

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

## Formatting

`dx format` runs JuliaFormatter 2.2.0 when a `julia` family selects
`julia_format`. The formatter and its packages are fetched only for Julia
targets. Syntax errors are reported with line and column and are never written.

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
