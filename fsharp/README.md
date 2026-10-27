# F#

Build and test F# with a pinned .NET SDK 10.0.201. Nothing is installed on the
host and nothing is fetched until a target needs it. rules_dx registers the
toolchain, so there is no setup in `MODULE.bazel`.

## Start

```sh
bazel run @rules_dx//:dx -- new --apply fsharp demo
cd demo
bazel run @rules_dx//:dx -- generate
bazel build //...
```

The template writes `demo.fsproj` and `Library.fs` and no `BUILD.bazel`.
`dx generate` writes the targets.

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

`dx generate` takes the order from the `<Compile Include="...">` items of the
`.fsproj` file.

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
