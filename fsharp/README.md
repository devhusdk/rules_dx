# F#

Build and test F# with a pinned .NET SDK 10.0.400. Nothing is installed on the
host and nothing is fetched until a target needs it. rules_dx registers the
toolchain, so there is no setup in `MODULE.bazel`.

## Start

```sh
bazel run @rules_dx//:dx -- init --apply demo
cd demo
bazel run @rules_dx//:dx -- generate
bazel build //...
```

Add a `demo.fsproj` and its sources before `dx generate`. `dx generate`
wires the language and writes the targets; you write no `BUILD.bazel`.

## Rules

Load the rules from `@rules_dx//fsharp/rules:defs.bzl`. They wrap
[rules_dotnet](https://github.com/bazel-contrib/rules_dotnet) and take the same
attributes.

```python
load("@rules_dx//fsharp/rules:defs.bzl", "fsharp_binary", "fsharp_library", "fsharp_test")

fsharp_library(
    name = "greeter_lib",
    srcs = ["Greeter.fs"],
    deps = ["@paket.main//fsharp.core"],
)

fsharp_test(
    name = "greeter_test",
    srcs = ["GreeterTest.fs"],
    deps = [
        ":greeter_lib",
        "@paket.main//fsharp.core",
    ],
)
```

| Rule | What it makes |
| --- | --- |
| `fsharp_library` | Assembly from `srcs` |
| `fsharp_binary` | Executable run with `bazel run`. `srcs` is optional |
| `fsharp_test` | Test executable run with `bazel test` |

- `target_frameworks` defaults to `["net10.0"]`. Set it to change it.
- `treat_warnings_as_errors` defaults to `True`. Set it to `False` to allow
  warnings.
- `FSharp.Core` is a NuGet package. List it in `deps`.

## Compile order

F# compiles files in the order they are listed. List `srcs` in that order and
tell buildifier not to sort the list:

```python
fsharp_library(
    name = "order_lib",
    srcs = [
        # buildifier: disable=sort
        "Helper.fs",
        "Demo.fs",
    ],
    deps = ["@paket.main//fsharp.core"],
)
```

`dx generate` works out the order from the sources themselves; it does not
read `.fsproj` files. A file comes after every sibling it uses:

- a type, exception or nested module that a sibling declares, named plainly
  (`{ b: Item }`) or qualified (`Helper.suffix`);
- a sibling's file-level module (`module Demo.Types`), by `open` or by name;
- a sibling found by `open`, matched by file name.

A `Name.fsi` signature file goes right before its `Name.fs`. A lone `.fsi` is
refused. When two files need each other, generation stops and names one edge,
for example `A.fs -> B.fs (B)`; split the directory or list `srcs` by hand.

## NuGet packages

Keep a `paket.dependencies` file in your module and generate the Bazel
repository from it with rules_dotnet:

```sh
bazel run @rules_dotnet//tools/paket2bazel -- --dependencies-file third_party/dotnet/paket.dependencies --output-folder third_party/dotnet/deps
```

Load the generated extension in your `MODULE.bazel`:

```python
dotnet_nuget = use_extension("//third_party/dotnet/deps:paket.main_extension.bzl", "main_extension")
use_repo(dotnet_nuget, "paket.main")
```

Depend on a package as `@paket.main//<lowercase package id>`. `dx update nuget`
bumps the pins.

## Formatting and linting

`fantomas` formats and `fsharplint` lints. Both run from managed pins. See
the Quality page of the command reference for the family setup.

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- lint //app/...
```

## Platforms

| Cell | Format and lint |
| --- | --- |
| `linux_x86_64` | Supported |
| Other cells | Not supported |

The command reference covers `dx generate`. The `examples/adopt-fsharp` tree shows a full run.

## Coverage

`bazel coverage` and `dx coverage` run F# tests under coverlet and write LCOV. Coverage covers the first-party assemblies that match `--instrumentation_filter`. The test assembly counts only with `--instrument_test_targets`. Tag a test `no-coverage` to skip it under coverage.
