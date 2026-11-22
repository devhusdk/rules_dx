# Julia

This directory helps you build, run and test Julia code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//julia:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Julia page](../docs/languages/julia.md) shows
setup, every rule and the tools dx runs.

Run the Julia tests from a rules_dx checkout:

```sh
tools/dx test //julia/...
```

## Dependencies

### Packages

Pin your packages in a `Manifest.bazel.json` and load it in `MODULE.bazel`:

```python
julia = use_extension("@rules_dx//julia:extensions.bzl", "julia")
julia.from_manifest(
    name = "app_pkgs",
    lockfile = "//:Manifest.bazel.json",
)
use_repo(julia, "app_pkgs")
```

## Generate

`dx generate` reads `Project.toml` only.

- A `Project.toml` with a `name` and a `src/<name>.jl` gets a
  `julia_library` named `<name>` from `src/**/*.jl`. `Project.toml` becomes
  `data`, and `[deps]` become dependencies.
- When `test/runtests.jl` exists, it also gets a `julia_test` named
  `<name>_test`.
- It leaves a `Project.toml` without a `name` alone.
- It never generates a `julia_binary`, and it ignores `ext/` and
  `[weakdeps]`.
- A third-party package needs a `# gazelle:resolve julia` line.

## Quality

Select the tools in a `julia` family in `dx.toml`:

```toml
[families.julia]
format = ["julia_format"]
lint = ["julialint"]
typecheck = ["jet"]
```

| Command | Tool | Override flag |
| --- | --- | --- |
| `dx format` | JuliaFormatter 2.2.0 | |
| `dx lint` | the Julia VS Code linter (JuliaWorkspaces.jl 13.5.0) | `--@rules_dx//config:tool_julialint` |
| `dx typecheck` | JET 0.13.1 | `--@rules_dx//config:tool_jet` |

- The formatter and its packages are fetched only for Julia targets.
- A syntax error is reported with its line and column, and the file is never
  rewritten.
- Lint findings are named `julialint/<code>` at their line and column. The
  linter works offline and reads only the files dx passes.
- JET findings are reported at their line.

The [quality commands page](../docs/cli/commands/quality.md) covers the
flags and family setup.

## API docs

`dx docs` documents every `julia_library` with no setup. It loads the library
under the pinned Julia, reads the docstrings of the exported and `public`
names through the pinned Documenter.jl 1.20.0, and writes a DocIr shard.

- The library is named after its root module.
- Its dependencies must be `julia_library` targets in the same workspace.
- To name the shard yourself, use `julia_doc_shard(name, lib, module, package)`
  from `@rules_dx//julia:defs.bzl`.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms. There is
no Julia build for `windows_arm64`.

On Linux x86_64, Julia runs on a source-built `libjulia-internal` that
consumers download as the prebuilt `dx-julia-internal-linux-x86_64.so`
release asset. With that download, a Linux x86_64 target gets the
source-built library from any exec host. A source build of the library
needs a Linux exec host.
