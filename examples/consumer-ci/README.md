# Consumer CI example

```sh
cp caller.yml .github/workflows/ci.yml
```

Starter caller for the reusable consumer workflow. Pin to a reviewed commit.

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

Set the pin, inputs and repository settings as [GitHub CI](../../docs/github-ci.md) shows.

## Default C/C++ and Rust setup

Generate the setup snippet:

```sh
bazel run @rules_dx//:dx -- init --apply my_project
```

Copy `MODULE.bazel.snippet` into `MODULE.bazel`. The snippet pins the
hermetic LLVM and Rust toolchains and registers them:

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
bazel_dep(name = "rules_cc", version = "0.2.27")
bazel_dep(name = "llvm", version = "0.8.23")
bazel_dep(name = "rules_rust", version = "0.74.0")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    rustfmt_version = "1.98.0",
    versions = ["1.98.0"],
)
use_repo(rust, "rust_toolchains")

register_toolchains("@llvm//toolchain:all")
register_toolchains("@rust_toolchains//:all")
```

Only Bazel/Bazelisk is required. Ordinary `cc_*` and `rust_*` rules need
no linker paths or per-target flags. Native host builds are the default.
Cross-target and Windows ABI choices are explicit advanced setup.
Windows ARM64 builds stay unqualified.

## Own toolchains

Keep the `bazel_dep` lines and drop the `register_toolchains` lines, then
register selected toolchains. Loading the rules never replaces consumer
toolchains:

```python
cc_configure = use_extension("@rules_cc//cc:extensions.bzl", "cc_configure_extension")
use_repo(cc_configure, "local_config_cc", "local_config_cc_toolchains")
register_toolchains("@local_config_cc_toolchains//:all")
```

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
bazel run @rules_dx//:dx -- lint --check //...
bazel run @rules_dx//:dx -- format --check //...
```

Inputs, pin and repository settings: [GitHub CI](../../docs/github-ci.md).
