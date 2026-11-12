# C#

Build and test C# with a pinned .NET SDK 10.0.403. Nothing is installed on the
host and nothing is fetched until a target needs it. `rules_dx` registers
the toolchain.

## Dependencies

### NuGet packages

Keep a `paket.dependencies` file in your module and generate the Bazel
repository from it with rules_dotnet:

```sh
bazel run @rules_dx//dotnet:paket2bazel -- --dependencies-file third_party/dotnet/paket.dependencies --output-folder third_party/dotnet/deps
```

Load the generated extension in your `MODULE.bazel`:

```python
dotnet_nuget = use_extension("//third_party/dotnet/deps:paket.main_extension.bzl", "main_extension")
use_repo(dotnet_nuget, "paket.main")
```

For `csharp_proto_library`, add `nuget Google.Protobuf 3.36.2` to your `paket.dependencies` to get `@paket.main//google.protobuf`.

Depend on a package as `@paket.main//<lowercase package id>`. `dx update nuget`
bumps the pins.

## Generate

`dx generate` writes `csharp_library`, `csharp_binary` and `csharp_test`
targets from your `.cs` files and `using` directives. A `using` of a namespace
that a local library declares resolves to that library. Any other
namespace resolves to the package in `paket.lock` that is named after it
(`Newtonsoft.Json.Linq` is `newtonsoft.json`).

The command reference covers `dx generate`. The `csharp/tests/fixtures/adopt-csharp` tree shows a full run.

## Quality

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

`roslyn` reads a `.globalconfig` bound with `roslyn_config`. A root
`.globalconfig` is the profile: it spells out every severity, so `dx` and the
C# Dev Kit, which picks it up by its name, report the same findings. The dx
bases are `standard.globalconfig` and `strict.globalconfig` in
`core/quality/tiers/roslyn/`.

`dx typecheck` runs `csc`, which reports only the compiler diagnostics (`CS*`)
from the same per-framework compilation that `roslyn` uses, so no extra
compile runs. Analyzer findings stay with `roslyn`.

## Coverage

`bazel coverage` and `dx coverage` run C# tests under coverlet and write LCOV. Coverage covers the first-party assemblies that match `--instrumentation_filter`. The test assembly counts only with `--instrument_test_targets`. Tag a test `no-coverage` to skip it under coverage.

## Platforms

| Cell | Format and lint |
| --- | --- |
| `linux_x86_64` | Supported |
| Other cells | Not supported |
