# Consumer CI example

```sh
cp caller.yml .github/workflows/ci.yml
```

Starter caller for the reusable consumer workflow. Pin to a reviewed commit.

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

Every job reads that declaration before `dx` runs. The version must be a
quoted string, and no other dependency may declare `rules_dx`.

A `validate` job runs before the checks and rejects a bad input with the
accepted values. Only `parallel` scheduling, `false` for `code_scanning_opt_in`
and a whole `min_coverage` from 0 to 100 are accepted.

Set approval policy, branch protection, and least-privilege permissions manually.

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

The full input contract lives in the GitHub CI docs.
