# PowerShell

Build, run and test PowerShell with a pinned pwsh 7.5.13. Nothing is installed
on the host and nothing is fetched until a target needs it. rules_dx registers
the toolchain, so there is no setup in `MODULE.bazel`.

## Start

```sh
bazel run @rules_dx//:dx -- init --apply demo
cd demo
```

Add sources, then run `bazel run @rules_dx//:dx -- generate`. The language is
wired when its sources appear.

The targets are Linux only. They set `target_compatible_with` to
`@platforms//os:linux`.

## Rules

Load the rules from `@rules_dx//powershell/rules:defs.bzl`. They wrap
[rules_powershell](https://github.com/bazel-contrib/rules_powershell) and take
the same attributes.

```python
load("@rules_dx//powershell/rules:defs.bzl", "powershell_binary", "powershell_library", "powershell_test")

powershell_library(
    name = "hello_lib",
    srcs = [
        "Hello.psd1",
        "Hello.psm1",
    ],
)

powershell_binary(
    name = "hello",
    srcs = ["Main.ps1"],
    target_compatible_with = ["@platforms//os:linux"],
    deps = [":hello_lib"],
)

powershell_test(
    name = "hello_test",
    srcs = ["Hello.Tests.ps1"],
    target_compatible_with = ["@platforms//os:linux"],
    deps = [":hello_lib"],
)
```

| Rule | What it makes |
| --- | --- |
| `powershell_library` | Module from `.ps1`, `.psm1` and `.psd1` files |
| `powershell_binary` | Script run with `bazel run`. `srcs` takes `.ps1` files and is optional |
| `powershell_test` | Script run with `bazel test`. `srcs` takes `.ps1` files |

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
to the `deps` of a `powershell_test` and import it by version:

```powershell
Import-Module Pester -RequiredVersion 5.7.1
```

## Formatting and linting

`psscriptanalyzer` formats and lints. Format runs `Invoke-Formatter`, and
`dx check --apply` rewrites scripts that would change. A script that does not parse is
reported and never rewritten. Lint is check-only. The default policy selects only
`typos` for the `powershell` family. To run `psscriptanalyzer` format and lint, list the family in
your `dx.toml`; on platforms without a PSScriptAnalyzer build they skip with a note. See the Quality page of the command reference for the
config rules.

```toml
[families.powershell]
format = ["psscriptanalyzer"]
lint = ["psscriptanalyzer"]
```

```sh
bazel run @rules_dx//:dx -- format //scripts/...
bazel run @rules_dx//:dx -- lint //scripts/...
```

## Generating BUILD files

`dx generate --apply` writes a `powershell_library` per directory of `.psm1`
modules and a `powershell_test` per `*.Tests.ps1` script. The
`tests/fixtures/adopt-powershell` tree in `rules_dx_powershell` shows a full layout.

## Platforms

| Cell | Format | Lint |
| --- | --- | --- |
| `linux_x86_64` | Supported | Supported |
| Other cells | Not supported | Not supported |

## Coverage

Pester tests write LCOV under `bazel coverage` and `dx coverage`. Depend on `//powershell/coverage:dx_coverage` and build the Pester configuration with `New-DxPesterConfiguration`. After `Invoke-Pester`, call `Write-DxPesterCoverage -Result $result`. Only the first-party `.ps1` and `.psm1` sources the test depends on are measured, and a line counts as covered when Pester ran a command on it. A test that does not call these functions writes no coverage.
