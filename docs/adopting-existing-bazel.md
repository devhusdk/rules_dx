# Adopting rules_dx In An Existing Bazel Repo

Use this when your repo already builds with upstream rules such as
`rules_rust` or `rules_python`. You keep your targets and move to `rules_dx`
one package at a time.

## Before you start

- Commit a clean tree and run `bazel test //...`. It must pass.
- Note the versions of `rules_rust` and any other module your `MODULE.bazel`
  declares. `rules_dx` pins some modules for the whole module graph. See
  `patches/README.md` in the rules_dx repository for the list.

## Add rules_dx

Add `bazel_dep(name = "rules_dx", version = "...")` to `MODULE.bazel`. Then
check what `dx init` would write:

```sh
bazel run @rules_dx//:dx -- init
```

Read the drift output and `MODULE.bazel.snippet` before you apply. Then run:

```sh
bazel run @rules_dx//:dx -- init --apply
```

For the files init writes, see [Init](cli/commands/init.md). The parts that
touch Bazel are:

- `MODULE.bazel` gets `include("//third_party/rules_dx:rules_dx.MODULE.bazel")`
  after the `rules_dx` dep.
- `third_party/rules_dx/` holds the overrides. dx owns it. Do not edit it.
- `MODULE.bazel.lock` changes. Commit it.
- `.bazelignore` lists `.dx`.
- `.bazelrc` is not written. Your own flags stay.

## What must move

The overrides pin these modules for your whole graph. If your repo needs
another version of one of them, you must move to the pinned one:
`rules_rust`, `rules_android`, `rules_dotnet`, `rules_ruby`, `aspect_rules_py`,
`aspect_rules_ts`, `aspect_rules_jest`. The versions are in
`patches/README.md` in the rules_dx repository.

If your `MODULE.bazel` already overrides one of them, init leaves the
`include()` out and says so on stderr. Merge the override by hand from
`third_party/rules_dx/rules_dx.MODULE.bazel`, or remove yours.

Only the root module's overrides count.

## Run dx on your existing targets

`dx` works on any Bazel target, so you can run it before you change any
`BUILD` file:

```sh
bazel run @rules_dx//:dx -- build //pkg/...
bazel run @rules_dx//:dx -- test //pkg/...
bazel run @rules_dx//:dx -- lint //pkg/...
```

See [Scope defaults](cli/commands/scope-defaults.md) for what a scope selects.

## Move one package

Change the `load()` in one package. Rust keeps its rule names:

```starlark
# before
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

# after
load("@rules_dx//rust/rules:defs.bzl", "rust_library", "rust_test")
```

Python changes names. The rules wrap `aspect_rules_py`:

```starlark
# before
load("@rules_python//python:defs.bzl", "py_library", "py_test")

# after
load("@rules_dx//python/rules:defs.bzl", "python_library", "python_test")
```

`python_test` runs pytest and rejects `main`. Use `python_py_test` for a
custom entrypoint.

Build and test the package, then move the next one:

```sh
bazel run @rules_dx//:dx -- test //pkg/...
```

Other languages follow the same pattern. Each language's rules are listed in
[Supported Surface](cli/commands/README.md#supported-surface).

## Roll back

- To undo a package, restore its `load()` lines.
- To go back to an earlier `rules_dx` version, see
  [Upgrade, rollback](cli/commands/upgrade-rollback.md). Rollback changes the
  version pin. It does not remove `rules_dx`.
- To remove `rules_dx`, delete the `bazel_dep`, the `include()` line, and
  `third_party/rules_dx/`, and restore your own overrides.
