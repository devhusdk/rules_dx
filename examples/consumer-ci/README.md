# Consumer CI example

```sh
cp caller.yml .github/workflows/ci.yml
```

Starter caller for the reusable consumer workflow. Pin to a reviewed commit.

rules_dx is not released yet. Install starts with the first BCR release.

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

Set the pin, inputs and repository settings as [GitHub CI](../../docs/github-ci.md) shows.

## Default C/C++ and Rust setup

Initialise a new directory:

```sh
bazel run @rules_dx//:dx -- init --apply my_project
```

It writes `my_project/MODULE.bazel` with the `rules_dx` pin and the include of
the dx overrides. In an existing tree, `init --apply` adds that include to
`MODULE.bazel`.

Only Bazel/Bazelisk is required. Ordinary `cc_*` and `rust_*` rules need
no linker paths or per-target flags. Native host builds are the default.
Cross-target and Windows ABI choices are explicit advanced setup.
Windows ARM64 builds stay unqualified.

## Own toolchains

Register your own toolchains in `MODULE.bazel`. Loading the rules never replaces consumer
toolchains:

```python
cc_configure = use_extension("@rules_cc//cc:extensions.bzl", "cc_configure_extension")
use_repo(cc_configure, "local_config_cc", "local_config_cc_toolchains")
register_toolchains("@local_config_cc_toolchains//:all")
```

See [Use your own C/C++ toolchain](../../docs/own-cc-toolchain.md) for a custom GCC and for precedence.

A consumer that renames the module keeps working. Declare the rename once:

```python
bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "my_dx")
```

Then load through the new name:

```python
load("@my_dx//cc/rules:defs.bzl", "cc_binary")
```

## Minimal targets

Load the public rules through the module name. A Rust library looks like
this:

```python
load("@rules_dx//rust/rules:defs.bzl", "rust_library")

rust_library(
    name = "greet",
    srcs = ["lib.rs"],
    crate_name = "greet",
    crate_root = "lib.rs",
)
```

A C++ binary that depends on another repository looks like this:

```python
load("@rules_dx//cc/rules:defs.bzl", "cc_binary")

cc_binary(
    name = "greet",
    srcs = ["main.cc"],
    deps = ["@greet_lib//:greeting"],
)
```

Run the checks from the module root with the public launcher:

```sh
bazel run @rules_dx//:dx -- lint //...
bazel run @rules_dx//:dx -- format //...
```

Inputs, pin and repository settings: [GitHub CI](../../docs/github-ci.md).
