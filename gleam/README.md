# Gleam

Build, run and test Gleam with a pinned Gleam 1.19.2. Erlang targets run on
the pinned OTP 29.1.2 runtime. JavaScript targets run on the Node.js runtime
from rules_nodejs. Nothing is installed on the host and nothing is fetched
until a target needs it. rules_dx registers the toolchain, so there is no
setup in `MODULE.bazel`.

## Start

```sh
bazel run @rules_dx//:dx -- new --apply gleam demo
cd demo
bazel test //...
bazel run :demo_bin
bazel run :demo_js_bin
```

## Rules

Load the rules from `@rules_dx//gleam/rules:defs.bzl`. The Erlang rules wrap
[rules_gleam](https://github.com/iocat/rules_gleam) and take the same
attributes.

```python
load("@rules_dx//gleam/rules:defs.bzl", "gleam_binary", "gleam_js_binary", "gleam_js_library", "gleam_js_test", "gleam_library", "gleam_test")

gleam_library(
    name = "stock",
    deps = ["@rules_dx//gleam/lib:gleam_stdlib"],
)

gleam_test(
    name = "stock_test",
    deps = [":stock"],
)

gleam_binary(
    name = "stockctl",
    srcs = ["src/stockctl.gleam"],
    deps = [":stock"],
)

gleam_js_library(
    name = "stock_js",
    deps = ["@rules_dx//gleam/lib:gleam_stdlib_js"],
)

gleam_js_test(
    name = "stock_js_test",
    deps = [":stock_js"],
)

gleam_js_binary(
    name = "stockctl_js",
    srcs = ["src/stockctl.gleam"],
    deps = [":stock_js"],
)
```

| Rule | What it makes |
| --- | --- |
| `gleam_library` | Gleam package compiled to Erlang from `src/**/*.gleam` |
| `gleam_erl_library` | Erlang FFI modules from `src/**/*.erl` |
| `gleam_test` | gleeunit test on Erlang from `test/**/*_test.gleam` |
| `gleam_test_library` | Testonly helper modules from `test/**/*.gleam` that are not `*_test.gleam` |
| `gleam_binary` | Runs `main` of one module on Erlang with `bazel run` |
| `gleam_js_library` | Gleam package compiled to JavaScript from `src/**/*.gleam` and `src/**/*.mjs` |
| `gleam_js_test` | gleeunit test on Node.js from `test/**/*.gleam` |
| `gleam_js_binary` | Runs `main` of one module on Node.js with `bazel run` |

- Module names come from the path below `src/` or `test/`. Set
  `strip_src_prefix` to use another directory.
- Library `tags` reach the library; the generated upstream target is
  `manual`, so `//...` builds only the library.
- A warning fails the build. Set `warnings_as_errors = False` to allow it.
- Tests get the pinned gleeunit. A test that runs no tests fails.
- Erlang tests take helper modules through a `gleam_test_library` in `deps`.
  `gleam_js_test` takes them in `srcs`. `dx generate` makes
  `<name>_test_support`.
- `gleam_js_binary` with more than one source needs `main_module`.
- A JavaScript package is named after its label. Set `package` to rename it.
- Program arguments reach `main` unchanged, UTF-8 included.

`@rules_dx//gleam/lib` has the pinned `gleam_stdlib`, `gleam_stdlib_js`,
`gleeunit` and `gleeunit_js`.

## Hex packages

Declare each Hex package once, pinned to a version and the tarball digest.
Nothing runs `gleam deps download` and no Hex cache is read.

```python
gleam_hex_package = use_repo_rule("@rules_dx//gleam/rules:hex.bzl", "gleam_hex_package")

gleam_hex_package(
    name = "gleam_json",
    package = "gleam_json",
    sha256 = "44fdaa8847be8fc48ca7a1c089706bd54badcc4c45b237a992eddf9f2cdb2836",
    version = "3.1.0",
    deps = ["@rules_dx//gleam/lib:gleam_stdlib"],
)
```

Depend on `@gleam_json//:gleam_json` for Erlang and
`@gleam_json//:gleam_json_js` for JavaScript. `deps` lists the Erlang labels
of the packages it requires. A wrong digest fails the fetch. A package not
built with Gleam is rejected.

## Adopt a Gleam project

1. Add `bazel_dep(name = "rules_dx", version = "...")` to `MODULE.bazel`.
2. Run `bazel run @rules_dx//:dx -- generate`. It writes a `gleam_library` and
   a `gleam_test` next to each `gleam.toml`. Add `gleam_js_library` and
   `gleam_js_test` by hand for the JavaScript target.
3. For each package in `manifest.toml`, add a `gleam_hex_package`. Its
   `sha256` is the `outer_checksum` in lowercase.
4. Add a `gleam_binary` or `gleam_js_binary` for each module with a `main`.
5. Run `bazel test //...`.

Generation resolves `gleam/*` imports through `gleam_stdlib`. Map it once
and map each Hex module you import:

```python
# gazelle:resolve gleam gleam_stdlib @rules_dx//gleam/lib:gleam_stdlib
# gazelle:resolve gleam gleam/json @gleam_json//:gleam_json
```

The build does not read `gleam.toml` or `manifest.toml`.

## Formatting

`dx format` checks Gleam sources with `gleam format` from the pinned Gleam.
Select `gleam_format` in a `gleam` family and pass the tool flag:

```sh
dx format -- --@rules_dx//config:tool_gleam_format=@rules_dx//gleam/tools:gleam_format
dx format --apply -- --@rules_dx//config:tool_gleam_format=@rules_dx//gleam/tools:gleam_format
```

The compiler is the type and warning check. No Gleam linter runs yet.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified |
| `linux_arm64` | Not qualified |
| `macos_x86_64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |

On other platforms Gleam targets fail toolchain resolution.

## Coverage

Gleam tests have no coverage collector yet. `dx coverage` fails with `no_coverage_collector` when one runs. Tag a test `no-coverage` to skip it under coverage.
