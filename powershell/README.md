# PowerShell

Build, run and test PowerShell with a pinned pwsh 7.6.9. Nothing is installed
on the host and nothing is fetched until a target needs it. rules_dx registers
the toolchain, so there is no setup in `MODULE.bazel`.

## Dependencies

### Gallery modules

List each PowerShell Gallery module with its exact version in
`PSGallery.requirements.psd1`. Nothing runs `Install-Module`.

```powershell
@{
    Pester = '5.7.1'
}
```

`dx update` runs `bazel run @rules_dx//powershell/private/tools/lock -- --dir <set directory>`,
which writes `powershell.lock.json` with the digest of each package.

```python
powershell = use_extension("@rules_dx//powershell:extensions.bzl", "powershell")
powershell.lock(name = "psgallery", lock = "//:powershell.lock.json")
use_repo(powershell, "psgallery")
```

Depend on `@psgallery//Pester`. A wrong digest fails the fetch.

## Generate

`dx generate --apply` writes a `powershell_library` per directory of `.psm1`
modules and a `powershell_test` per `*.Tests.ps1` script. The
`powershell/tests/fixtures/adopt-powershell` tree shows a full layout.

## Quality

### Pester

To test with the pinned Pester 5.7.1, add `@rules_dx//powershell:pester`
to the `deps` of a `powershell_test` and import it by version:

```powershell
Import-Module Pester -RequiredVersion 5.7.1
```

### Formatting and linting

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

## Coverage

Pester tests write LCOV under `bazel coverage` and `dx coverage`. Depend on `//powershell:dx_coverage` and build the Pester configuration with `New-DxPesterConfiguration`. After `Invoke-Pester`, call `Write-DxPesterCoverage -Result $result`. Only the first-party `.ps1` and `.psm1` sources the test depends on are measured, and a line counts as covered when Pester ran a command on it. A test that does not call these functions writes no coverage.

## Platforms

| Cell | Format | Lint |
| --- | --- | --- |
| `linux_x86_64` | Supported | Supported |
| Other cells | Not supported | Not supported |
