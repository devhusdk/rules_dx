# F#

Build and test F# with a pinned .NET SDK 10.0.403. Nothing is installed on the
host and nothing is fetched until a target needs it. `rules_dx` registers
the toolchain.

## Dependencies

### Compile order

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

Depend on a package as `@paket.main//<lowercase package id>`. `dx update nuget`
bumps the pins.

## Quality

### Formatting and linting

`fantomas` formats and `fsharplint` lints. Both run from managed pins. See
the Quality page of the command reference for the family setup.

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- lint //app/...
```

### Typecheck

`dx typecheck` runs `fsc`, which compiles the target a second time with the
pinned F# compiler and reports its `FS` diagnostics. See the Quality page of
the command reference.

## Coverage

`bazel coverage` and `dx coverage` run F# tests under coverlet and write LCOV. Coverage covers the first-party assemblies that match `--instrumentation_filter`. The test assembly counts only with `--instrument_test_targets`. Tag a test `no-coverage` to skip it under coverage.

## Platforms

| Cell | Format and lint |
| --- | --- |
| `linux_x86_64` | Supported |
| Other cells | Not supported |

The command reference covers `dx generate`. The `fsharp/tests/fixtures/adopt-fsharp` tree shows a full run.
