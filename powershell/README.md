# PowerShell

This directory helps you build, run and test PowerShell code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//powershell:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [PowerShell page](../docs/languages/powershell.md) shows
setup, every rule and the tools dx runs.

Run the PowerShell tests from a rules_dx checkout:

```sh
tools/dx test //powershell/...
```

## Dependencies

### Gallery modules

List each PowerShell Gallery module with its exact version in
`PSGallery.requirements.psd1`:

```powershell
@{
    Pester = '5.7.1'
}
```

1. Run `dx update --apply`. It writes `powershell.lock.json` with the digest
   of each package. Nothing runs `Install-Module`.
2. Load the lock in `MODULE.bazel`:

   ```python
   powershell = use_extension("@rules_dx//powershell:extensions.bzl", "powershell")
   powershell.lock(name = "psgallery", lock = "//:powershell.lock.json")
   use_repo(powershell, "psgallery")
   ```

3. Depend on a module as `@psgallery//Pester`.

A wrong digest fails the fetch.

### Runfiles

Runfiles are the files Bazel places beside a program when it runs, such as
its `data`. Put `@rules_dx//powershell:runfiles` in `deps` and look a file up
by its runfiles path:

```powershell
Import-Module BazelRunfiles

$path = Resolve-BazelRunfile '_main/app/data.txt'
```

- `Resolve-BazelRunfile` works with a runfiles directory and with a manifest
  only (`RUNFILES_MANIFEST_ONLY=1`).
- It returns `$null` when the manifest does not list the path.
- An apparent repository name, like `my_dep/file.txt`, maps through the repo
  mapping. Pass `-SourceRepository` when the calling code lives in another
  repository.
- `Get-BazelRunfilesEnvironment` returns the environment a child Bazel
  program needs to find the same runfiles.

## Generate

`dx generate --apply` writes a `powershell_library` per directory of `.psm1`
modules and a `powershell_test` per `*.Tests.ps1` script. The
`powershell/tests/fixtures/adopt_powershell` tree shows a full layout.

## Quality

### Pester

To test with the pinned Pester 5.7.1, add `@rules_dx//powershell:pester` to
the `deps` of a `powershell_test` and import it by version:

```powershell
Import-Module Pester -RequiredVersion 5.7.1
```

### Formatting and linting

The default policy runs only `typos` for the `powershell` family. To format
and lint with `psscriptanalyzer`, list it in your `dx.toml`:

```toml
[families.powershell]
format = ["psscriptanalyzer"]
lint = ["psscriptanalyzer"]
```

```sh
bazel run @rules_dx//dx/launch:dx -- format //scripts/...
bazel run @rules_dx//dx/launch:dx -- lint //scripts/...
```

- Format runs `Invoke-Formatter`. `dx check --apply` rewrites scripts that
  would change.
- A script that does not parse is reported and never rewritten.
- Lint only checks; it never rewrites.

The [quality commands page](../docs/cli/commands/quality.md) covers the
config rules.

## Coverage

Pester tests write LCOV under `bazel coverage` and `dx coverage`:

1. Add `@rules_dx//powershell:dx_coverage` to the test's `deps`.
2. Build the Pester configuration with `New-DxPesterConfiguration`.
3. After `Invoke-Pester`, call `Write-DxPesterCoverage -Result $result`.

Only the first-party `.ps1` and `.psm1` sources the test depends on are
measured. A line counts as covered when Pester ran a command on it. A test
that does not call these functions writes no coverage.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms.
