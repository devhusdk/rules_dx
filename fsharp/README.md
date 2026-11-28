# F#

This directory helps you build, run and test F# code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//fsharp:defs.bzl`.
The tests live in `tests/`. The [F# page](../docs/languages/fsharp.md) shows
setup, every rule and the tools dx runs.

Run the F# tests from a rules_dx checkout:

```sh
tools/dx test //fsharp/...
```

## Dependencies

### Compile order

F# compiles files in the order you list them. List `srcs` in that order and
tell buildifier not to sort the list:

```python
fsharp_library(
    name = "order_lib",
    srcs = [
        # buildifier: disable=sort
        "Helper.fs",
        "Demo.fs",
    ],
    deps = ["@nuget//fsharp.core"],
)
```

`dx generate` works out the order from the sources. It does not read
`.fsproj` files. A file comes after every sibling it uses:

- a type, exception or nested module that a sibling declares, named plainly
  (`{ b: Item }`) or qualified (`Helper.suffix`);
- a sibling's file-level module (`module Demo.Types`), by `open` or by name;
- a sibling found by `open`, matched by file name.

A `Name.fsi` signature file goes right before its `Name.fs`. A `.fsi` without
a matching `.fs` fails. When two files need each other, generation stops and
names one edge, for example `A.fs -> B.fs (B)`. Split the directory or list
`srcs` by hand. The `fsharp/tests/fixtures/adopt_fsharp` tree shows a full
run.

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

## Quality

`fantomas` formats, `fsharplint` lints and `fsc` typechecks. dx fetches all
three, so you install nothing:

```sh
bazel run @rules_dx//dx/launch:dx -- format //app/...
bazel run @rules_dx//dx/launch:dx -- lint //app/...
bazel run @rules_dx//dx/launch:dx -- typecheck //app/...
```

`fsc` compiles each target a second time with the pinned F# compiler and
reports its `FS` diagnostics. See the
[quality config table](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/quality.md).

## Coverage

`bazel coverage` and `dx coverage` run F# tests under coverlet and write LCOV.
They cover the first-party assemblies that match `--instrumentation_filter`.
The test assembly counts only with `--instrument_test_targets`. Tag a test
`no-coverage` to skip it under coverage.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
