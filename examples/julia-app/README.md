# Julia App

A multi-package Julia report CLI built on the dx wrappers from
`@rules_dx//julia/rules:defs.bzl`.

| Package | What it shows |
| --- | --- |
| `third_party/` | A lockfile hub named `julia_app_deps` with a pure package, `OrderedCollections`, and a binary artifact package, `Bzip2_jll`. |
| `stats/` | `Stats`, a library over `OrderedCollections` with a `Test` suite. |
| `archive/` | `Archive`, a library that calls `libbzip2` from `Bzip2_jll` through `ccall`. |
| `data/` | A genrule that turns a TSV into the CSV the CLI reads. |
| `cli/` | `Report`, the `readings` binary, and a `select()` on the `digits` flag. |
| `diagnostics/` | Rust tests that run the pinned Julia on broken sources and the formatter on the example. |

## Run

```sh
bazel run //examples/julia-app/cli:readings
bazel test //examples/julia-app/...
bazel test //examples/julia-app/... --//examples/julia-app/cli:digits=4
```

## Dependencies

`third_party/Project.toml` lists the two packages. `Manifest.toml` pins the
resolved versions and `Manifest.bazel.json` carries the hashes and artifact
URLs that Bazel fetches. `bazel run //examples/julia-app/third_party:resolve`
regenerates both after a change, and `resolve_test` fails when they drift.
Nothing is fetched while the module graph is evaluated.

`Preferences` is a direct dependency pinned with `=1.5.0` because the module
annotation in `MODULE.bazel` patches that release for every hub. Resolve with
`JULIA_LOAD_PATH=@:@stdlib` set on `pkg_resolve.jl` if `:resolve` stops at
"Unable to locate installed package": the rules_julia 0.8.0 driver hides the
temporary environment from `Base.locate_package`.

The `julia_pkg` extension names the repository `julia_app_pkgs`, and
`use_repo` maps it to `julia_app_deps`, so BUILD files say
`@julia_app_deps//:OrderedCollections`. It is a `dev_dependency`: a project
that uses rules_dx as a dependency never sees it.

## Layout

Each library is named after its module and keeps its source in
`src/<Name>.jl` beside a `Project.toml` in `data`. That is the layout
rules_julia needs to precompile a package.

## Known gaps

Standalone output (`julia_standalone_binary`) and build-time precompile
failures do not work on a dx-wrapped target yet, because the wrapper names the
upstream target `<name>_upstream` and rules_julia looks for `<name>.jl`.
The diagnostics package covers syntax, runtime and missing-dependency
failures with the pinned Julia until #4606 is fixed. Windows arm64 has no
Julia build, so every target here is skipped there.
