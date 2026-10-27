# C#

Build and test C# with a pinned .NET SDK 10.0.201. Nothing is installed on the
host and nothing is fetched until a target needs it. rules_dx registers the
toolchain, so there is no setup in `MODULE.bazel`.

## Start

```sh
bazel run @rules_dx//:dx -- new --apply csharp demo
cd demo
bazel run @rules_dx//:dx -- generate
bazel build //...
```

The template writes `demo.csproj` and `Hello.cs` and no `BUILD.bazel`.
`dx generate` writes the targets.

## Rules

Load the rules from `@rules_dx//csharp/rules:defs.bzl`. They wrap
[rules_dotnet](https://github.com/bazel-contrib/rules_dotnet) and take the same
attributes.

```python
load("@rules_dx//csharp/rules:defs.bzl", "csharp_binary", "csharp_library", "csharp_test")

csharp_library(
    name = "hello_lib",
    srcs = ["Hello.cs"],
)

csharp_binary(
    name = "hello",
    srcs = ["Program.cs"],
    deps = [":hello_lib"],
)

csharp_test(
    name = "hello_test",
    srcs = ["HelloTest.cs"],
    deps = [":hello_lib"],
)
```

| Rule | What it makes |
| --- | --- |
| `csharp_library` | Assembly from `srcs` |
| `csharp_binary` | Executable run with `bazel run`. `srcs` is optional |
| `csharp_test` | Test executable run with `bazel test` |

- `target_frameworks` defaults to `["net10.0"]`. Set it to change it.
- `treat_warnings_as_errors` defaults to `True`. Set it to `False` to allow
  warnings.

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

`csharpier` formats and `roslyn` lints. Both run from managed pins. See
the Quality page of the command reference for the family setup.

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- lint //app/...
```

`roslyn` analyzes every framework in a `target_frameworks` list, for the
host runtime only. A finding that only some frameworks report ends with
those frameworks, such as `[net8.0]`. When `target_frameworks` is a
`select()`, only the framework it resolves to is analyzed.

## Generating BUILD files

`dx generate` writes `csharp_library`, `csharp_binary` and `csharp_test`
targets from your `.cs` files and `using` directives. A `using` of a namespace
that a local library declares resolves to that library. Map any other
namespace by its full name with a directive:

```python
# gazelle:resolve csharp Newtonsoft.Json @paket.main//newtonsoft.json
```

The command reference covers `dx generate`. The `examples/adopt-csharp` tree shows a full run.

## Platforms

| Cell | Format and lint |
| --- | --- |
| `linux_x86_64` | Supported |
| Other cells | Not supported |

## Coverage

`bazel coverage` and `dx coverage` run C# tests under coverlet and write LCOV. Coverage covers the first-party assemblies that match `--instrumentation_filter`. The test assembly counts only with `--instrument_test_targets`. Tag a test `no-coverage` to skip it under coverage.
