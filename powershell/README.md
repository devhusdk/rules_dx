# PowerShell

Build, run and test PowerShell with a pinned pwsh 7.5.11. Nothing is installed
on the host and nothing is fetched until a target needs it. rules_dx registers
the toolchain, so there is no setup in `MODULE.bazel`.

## Start

```sh
bazel run @rules_dx//:dx -- new --apply powershell demo
cd demo
bazel build //...
bazel run :demo_bin
bazel test //...
```

The targets are Linux only. They set `target_compatible_with` to
`@platforms//os:linux`.

## Rules

Load the rules from `@rules_dx//powershell/rules:defs.bzl`. They wrap
[rules_powershell](https://github.com/bazel-contrib/rules_powershell) and take
the same attributes.

```python
load("@rules_dx//powershell/rules:defs.bzl", "pwsh_binary", "pwsh_library", "pwsh_test")

pwsh_library(
    name = "hello_lib",
    srcs = [
        "Hello.psd1",
        "Hello.psm1",
    ],
)

pwsh_binary(
    name = "hello",
    srcs = ["Main.ps1"],
    target_compatible_with = ["@platforms//os:linux"],
    deps = [":hello_lib"],
)

pwsh_test(
    name = "hello_test",
    srcs = ["Hello.Tests.ps1"],
    target_compatible_with = ["@platforms//os:linux"],
    deps = [":hello_lib"],
)
```

| Rule | What it makes |
| --- | --- |
| `pwsh_library` | Module from `.ps1`, `.psm1` and `.psd1` files |
| `pwsh_binary` | Script run with `bazel run`. `srcs` takes `.ps1` files and is optional |
| `pwsh_test` | Script run with `bazel test`. `srcs` takes `.ps1` files |

## Gallery modules

Pin each PowerShell Gallery module to a package URL and its digest. Nothing
runs `Install-Module`.

```python
psgallery_module = use_repo_rule("@rules_dx//powershell/rules:gallery.bzl", "psgallery_module")

psgallery_module(
    name = "psgallery_pester",
    module = "Pester",
    sha256 = "4a27904c6814a5fbe4758f8e49861f6a1994aee77b71165a5c43c0371ba6c580",
    urls = ["https://www.powershellgallery.com/api/v2/package/Pester/5.7.1"],
)
```

Depend on `@psgallery_pester//:pester`. The target name is the lowercased
module name. A wrong digest fails the fetch.

## Pester

To test with the pinned Pester 5.7.1, add `@rules_dx//powershell/gallery:pester`
to the `deps` of a `pwsh_test` and import it by version:

```powershell
Import-Module Pester -RequiredVersion 5.7.1
```

## Formatting and linting

`psscriptanalyzer` formats and lints. Format runs `Invoke-Formatter`, and
`dx fix` rewrites scripts that would change. A script that does not parse is
reported and never rewritten. Lint is check-only. Select both in the
`powershell` family. See the Quality page of the command reference for the
config rules.

```python
quality_family(
    name = "powershell_family",
    family_id = "powershell",
    format = ["psscriptanalyzer"],
    lint = ["psscriptanalyzer"],
)
```

```sh
bazel run @rules_dx//:dx -- format //scripts/...
bazel run @rules_dx//:dx -- lint //scripts/...
```

## Generating BUILD files

`dx generate --apply` writes a `pwsh_library` per directory of `.psm1`
modules and a `pwsh_test` per `*.Tests.ps1` script. The
`examples/adopt-powershell` tree shows a full layout.

## Platforms

| Cell | Format | Lint |
| --- | --- | --- |
| `linux_x86_64` | Supported | Supported |
| Other cells | Not supported | Not supported |

## Coverage

PowerShell tests have no coverage collector yet. `dx coverage` fails with `no_coverage_collector` when one runs. Tag a test `no-coverage` to skip it under coverage.
