# C#

This directory helps you build, run and test C# code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//csharp:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [C# page](../docs/languages/csharp.md) shows
setup, every rule and the tools dx runs.

Run the C# tests from a rules_dx checkout:

```sh
tools/dx test //csharp/...
```

## Dependencies

### NuGet packages

List your packages in a `paket.dependencies` file. `dx update --apply` locks
them into `nuget.lock.json`. Then declare the lock in your `MODULE.bazel`:

```python
nuget = use_extension("@rules_dx//csharp:extensions.bzl", "nuget")
nuget.lock(
    name = "nuget",
    lock = "//third_party/dotnet:nuget.lock.json",
)
use_repo(nuget, "nuget")
```

Depend on a package as `@nuget//<lowercase package id>`. `dx update nuget`
moves the pins to newer versions.

`csharp_proto_library` needs Google.Protobuf. Add
`nuget Google.Protobuf 3.37.0-rc3` to `paket.dependencies` and put
`@nuget//google.protobuf` in `deps`.

## Generate

`dx generate` writes `csharp_library`, `csharp_binary` and `csharp_test`
targets from your `.cs` files and their `using` directives. A `using` of a
namespace that a local library declares resolves to that library. Any other
namespace resolves to the package in `paket.lock` named after it, so
`Newtonsoft.Json.Linq` resolves to `newtonsoft.json`.

The `csharp/tests/fixtures/adopt_csharp` tree shows a full run.

## Quality

`csharpier` formats, `roslyn` lints and `csc` typechecks. dx fetches all
three, so you install nothing:

```sh
bazel run @rules_dx//dx/launch:dx -- format //app/...
bazel run @rules_dx//dx/launch:dx -- lint //app/...
bazel run @rules_dx//dx/launch:dx -- typecheck //app/...
```

`roslyn` analyzes every framework in a `target_frameworks` list, for the host
runtime only. A finding that only some frameworks report ends with those
frameworks, such as `[net8.0]`. When `target_frameworks` is a `select()`,
only the framework it resolves to is analyzed.

`roslyn` reads a `.globalconfig` bound with `roslyn_config`. Name it
`.globalconfig` and put it at the root: the C# Dev Kit picks it up by that
name, so your editor and `dx` report the same findings. Start from
`standard.globalconfig` or `strict.globalconfig` in
`core/quality/tiers/roslyn/`.

`csc` reports only compiler diagnostics (`CS*`), from the same per-framework
compile that `roslyn` uses, so no extra compile runs. Analyzer findings come
from `roslyn`. See the
[quality config table](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/quality.md).

## Coverage

`bazel coverage` and `dx coverage` run C# tests under coverlet and write LCOV.
They cover the first-party assemblies that match `--instrumentation_filter`.
The test assembly counts only with `--instrument_test_targets`. Tag a test
`no-coverage` to skip it under coverage.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
