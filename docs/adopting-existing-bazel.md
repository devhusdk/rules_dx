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

rules_dx is not released yet. Install starts with the first BCR release.

Add `bazel_dep(name = "rules_dx", version = "...")` to `MODULE.bazel`. Then
check what `dx init` would write:

```sh
bazel run @rules_dx//:dx -- init
```

Read the drift output before you apply. Then run:

```sh
bazel run @rules_dx//:dx -- init --apply
```

For the files init writes, see [Init](cli/commands/init.md). The parts that
touch Bazel are:

- `MODULE.bazel` gets `include("//third_party/rules_dx:rules_dx.MODULE.bazel")`
  after the `rules_dx` dep.
- `third_party/rules_dx/` holds the overrides. dx owns it. Do not edit it.
- `MODULE.bazel.lock` changes. Commit it.
- CI fails when `MODULE.bazel.lock` is stale. Run `bazel mod deps` and commit the lock.
- `.bazelignore` lists `.dx`.
- `.bazelrc` is not written. Your own flags stay.

## What must move

The overrides pin these modules for your whole graph. If your repo needs
another version of one of them, you must move to the pinned one:
`rules_rust`, `rules_android`, `rules_dotnet`, `rules_py`,
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

Change the `load()` in one package. Rust keeps its upstream rule names:

```starlark
# before
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

# after
load("@rules_dx//rust/rules:defs.bzl", "rust_library", "rust_test")
```

Python changes names. The rules wrap `rules_py`:

```starlark
# before
load("@rules_python//python:defs.bzl", "py_library", "py_test")

# after
load("@rules_dx//python/rules:defs.bzl", "python_library", "python_test")
```

`python_test` runs pytest. Set `runner = "plain"` and `main` for a custom entrypoint.

Build and test the package, then move the next one:

```sh
bazel run @rules_dx//:dx -- test //pkg/...
```

dx names rules `<language>_<kind>` everywhere. For any other language, find the
upstream rule in this table and use the dx load and rule next to it. The full
list is in [Supported Surface](cli/commands/README.md#supported-surface).

| Upstream load | Upstream rule | dx load | dx rule |
| --- | --- | --- | --- |
| `@rules_android//rules:rules.bzl` | `android_library` | `@rules_dx_android//rules:defs.bzl` | `java_android_library` |
| `@rules_kotlin//kotlin:android.bzl` | `kt_android_library` | `@rules_dx_android//rules:defs.bzl` | `kotlin_android_library` |
| `@rules_apple//apple:ios.bzl` | `ios_application` | `@rules_dx//apple/ios:app.bzl` | `apple_ios_app` |
| `@rules_apple//apple:ios.bzl` | `ios_unit_test` | `@rules_dx//apple/ios:app.bzl` | `apple_ios_app_test` |
| `@rules_apple//apple:apple.bzl` | `apple_xcframework` | `@rules_dx//apple/ios:app.bzl` | `apple_ios_xcframework` |
| `@rules_apple//apple:macos.bzl` | `macos_application` | `@rules_dx//apple/macos:app.bzl` | `apple_macos_app` |
| `@rules_cc//cc:defs.bzl` | `cc_binary`, `cc_library`, `cc_test` | `@rules_dx//cc/rules:defs.bzl` | `cc_binary`, `cc_library`, `cc_test` |
| `@rules_cc//cc:objc_library.bzl` | `objc_library` | `@rules_dx//cc/rules:defs.bzl` | `objc_library` |
| `@rules_dotnet//dotnet:defs.bzl` | `csharp_binary`, `csharp_library`, `csharp_test` | `@rules_dx//csharp/rules:defs.bzl` | `csharp_binary`, `csharp_library`, `csharp_test` |
| `@rules_cuda//cuda:defs.bzl` | `cuda_library`, `cuda_objects` | `@rules_dx//cuda/rules:defs.bzl` | `cuda_library`, `cuda_objects` |
| `@rules_d//d:defs.bzl` | `d_binary`, `d_library`, `d_test` | `@rules_dx//d/rules:defs.bzl` | `d_binary`, `d_library`, `d_test` |
| `@rules_elixir//:elixir_app.bzl` | `elixir_app` | `@rules_dx//elixir/rules:defs.bzl` | `elixir_app` |
| `@rules_elixir//:ex_unit_test.bzl` | `ex_unit_test` | `@rules_dx//elixir/rules:defs.bzl` | `ex_unit_test` |
| `@rules_erlang//:erlang_app.bzl` | `erlang_app`, `test_erlang_app` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_app`, `test_erlang_app` |
| `@rules_erlang//:ct.bzl` | `ct_suite` | `@rules_dx//erlang/rules:defs.bzl` | `ct_suite` |
| `@rules_erlang//:erlang_bytecode2.bzl` | `erlang_bytecode` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_bytecode` |
| `@rules_erlang//:escript.bzl` | `escript_archive` | `@rules_dx//erlang/rules:defs.bzl` | `escript_archive` |
| `@rules_erlang//:eunit2.bzl` | `eunit` | `@rules_dx//erlang/rules:defs.bzl` | `eunit` |
| `@rules_dotnet//dotnet:defs.bzl` | `fsharp_binary`, `fsharp_library`, `fsharp_test` | `@rules_dx//fsharp/rules:defs.bzl` | `fsharp_binary`, `fsharp_library`, `fsharp_test` |
| `@rules_go//go:def.bzl` | `go_binary`, `go_library`, `go_test` | `@rules_dx//go/rules:defs.bzl` | `go_binary`, `go_library`, `go_test` |
| `@rules_haskell//haskell:defs.bzl` | `haskell_binary`, `haskell_library`, `haskell_test` | `@rules_dx//haskell/rules:defs.bzl` | `haskell_binary`, `haskell_library`, `haskell_test` |
| `@rules_java//java:defs.bzl` | `java_binary`, `java_library`, `java_test` | `@rules_dx//java/rules:defs.bzl` | `java_binary`, `java_library`, `java_test` |
| `@aspect_rules_js//js:defs.bzl` | `js_binary` | `@rules_dx//javascript/rules:defs.bzl` | `javascript_binary` |
| `@aspect_rules_js//js:defs.bzl` | `js_library` | `@rules_dx//javascript/rules:defs.bzl` | `javascript_library` |
| `@aspect_rules_js//js:defs.bzl` | `js_test` | `@rules_dx//javascript/rules:defs.bzl` | `javascript_test` |
| `@aspect_rules_jest//jest:defs.bzl` | `jest_test` | `@rules_dx//javascript/rules:defs.bzl` | `javascript_test` |
| `@rules_julia//julia:defs.bzl` | `julia_binary`, `julia_library`, `julia_test` | `@rules_dx//julia/rules:defs.bzl` | `julia_binary`, `julia_library`, `julia_test` |
| `@rules_kotlin//kotlin:jvm.bzl` | `kt_jvm_binary`, `kt_jvm_library`, `kt_jvm_test` | `@rules_dx//kotlin/rules:defs.bzl` | `kotlin_binary`, `kotlin_library`, `kotlin_test` |
| `@rules_perl//perl:defs.bzl` | `perl_binary`, `perl_library`, `perl_test` | `@rules_dx//perl/rules:defs.bzl` | `perl_binary`, `perl_library`, `perl_test` |
| `@rules_powershell//powershell:defs.bzl` | `pwsh_binary`, `pwsh_library`, `pwsh_test` | `@rules_dx//powershell/rules:defs.bzl` | `pwsh_binary`, `pwsh_library`, `pwsh_test` |
| `@rules_py//py:defs.bzl` | `py_binary` | `@rules_dx//python/rules:defs.bzl` | `python_binary` |
| `@rules_py//py:defs.bzl` | `py_library` | `@rules_dx//python/rules:defs.bzl` | `python_library` |
| `@rules_py//py:defs.bzl` | `py_test`, `py_pytest_test` | `@rules_dx//python/rules:defs.bzl` | `python_test` |
| `@rules_ruby//ruby:defs.bzl` | `rb_binary`, `rb_library`, `rb_test` | `@rules_dx//ruby/rules:defs.bzl` | `ruby_binary`, `ruby_library`, `ruby_test` |
| `@rules_rust//rust:defs.bzl` | `rust_binary`, `rust_clippy_test`, `rust_library`, `rust_proc_macro`, `rust_shared_library`, `rust_static_library`, `rust_test`, `rustfmt_test` | `@rules_dx//rust/rules:defs.bzl` | same names |
| `@rules_scala//scala:scala.bzl` | `scala_binary`, `scala_library`, `scala_test` | `@rules_dx//scala/rules:defs.bzl` | `scala_binary`, `scala_library`, `scala_test` |
| `@aspect_rules_ts//ts:defs.bzl` | `ts_project` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_project` |
| `@aspect_rules_js//js:defs.bzl` | `js_test` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_test` |
| `@aspect_rules_jest//jest:defs.bzl` | `jest_test` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_test` |
| `@aspect_rules_js//js:defs.bzl` | `js_library` | `@rules_dx//vue/rules:defs.bzl` | `vue_library` |
| `@rules_zig//zig:defs.bzl` | `zig_binary`, `zig_library`, `zig_test` | `@rules_dx//zig/rules:defs.bzl` | `zig_binary`, `zig_library`, `zig_test` |

## Generated target names

A dx macro adds targets next to yours. See
[Generated targets dx adds](bazel-for-dx-users.md#generated-targets-dx-adds).

## Roll back

- To undo a package, restore its `load()` lines.
- To go back to an earlier `rules_dx` version, see
  [Upgrade](cli/commands/upgrade.md). It changes the
  version pin. It does not remove `rules_dx`.
- To remove `rules_dx`, delete the `bazel_dep`, the `include()` line, and
  `third_party/rules_dx/`, and restore your own overrides.
