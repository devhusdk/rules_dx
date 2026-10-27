# Elixir

Build and test Elixir with a pinned Elixir 1.20.4 on the pinned OTP 29.1.1
runtime. Nothing is installed on the host and nothing is fetched until a
target needs it. rules_dx registers the toolchain, so there is no setup in
`MODULE.bazel`.

## Start

```sh
bazel run @rules_dx//:dx -- new --apply elixir demo
cd demo
bazel test //...
bazel run :demo_bin
```

## Rules

Load the rules from `@rules_dx//elixir/rules:defs.bzl`. They wrap
[rules_elixir](https://github.com/bazelverse/rules_elixir) and take the same
attributes.

```python
load("@rules_dx//elixir/rules:defs.bzl", "elixir_app", "escript_archive", "escript_headers", "ex_unit_test")

elixir_app(
    app_name = "stock",
    app_version = "1.0.0",
    app_module = "Stock.Application",
    app_env = "[{max_units, 60}]",
    deps = ["//catalog:erlang_app"],
)

ex_unit_test(
    name = "ledger_test",
    srcs = [
        "test/ledger_test.exs",
        "test/test_helper.exs",
    ],
    deps = [":erlang_app"],
)

escript_archive(
    name = "stockctl",
    app = ":erlang_app",
    headers = escript_headers("Stockctl"),
)
```

| Rule | What it makes |
| --- | --- |
| `elixir_app` | OTP application from `lib/**/*.ex`, compiled with `--warnings-as-errors` |
| `ex_unit_test` | ExUnit test; `test_helper.exs` in `srcs` runs first |
| `escript_archive` | Escript with the app and its deps, runnable with `bazel run` |

- `elixir_app` is named `erlang_app` unless you pass `name`.
- `testonly = True` marks the app and everything it builds test-only.
- `tags` reach the app and every target it builds; the generated targets are
  `manual`, so `//...` builds only the app.
- `app_module` names the Elixir module that starts the application.
- `elixirc_opts` replaces `DEFAULT_ELIXIRC_OPTS`.
- `escript_headers("Mod")` starts `Mod.main/1` with UTF-8 file names and
  stdio.
- A test that runs no tests fails.

## Hex packages

Declare each Hex package once, pinned to a version and the tarball digest.
Nothing runs `mix deps.get` and no Hex or Mix home is read.

```python
hex_package = use_repo_rule("@rules_dx//elixir/rules:hex.bzl", "hex_package")

hex_package(
    name = "jason",
    build_file = "//third_party:jason.BUILD.bazel",
    package = "jason",
    sha256 = "c5eb0cab91f094599f94d55bc63409236a8ec69a21a67814529e8d5f6cc90b3b",
    version = "1.4.4",
)
```

The build file declares the package as an `elixir_app` with
`visibility = ["//visibility:public"]`. Depend on it as
`@jason//:erlang_app`. A wrong digest fails the fetch.

## Adopt a Mix project

1. Add `bazel_dep(name = "rules_dx", version = "...")` to `MODULE.bazel`.
2. For each app under `apps/` or the project root, add a `BUILD.bazel` with
   an `elixir_app` that matches `mix.exs`: `app_name`, `app_version`,
   `app_module` for `mod:`, `app_env` for `env:`, and `deps`.
3. For each Hex entry in `mix.lock`, add a `hex_package`. Its `sha256` is the
   last checksum in the `mix.lock` entry.
4. Add an `ex_unit_test` per test file or group.
5. Run `bazel test //...`.

`mix.exs`, `mix.lock` and `config/*.exs` are not read. Put runtime settings
in `app_env`.

## Formatting

`dx format` checks Elixir sources with `mix format` from the pinned Elixir.
Select `mix_format` in an `elixir` family and pass the tool flag:

```sh
dx format -- --@rules_dx//config:tool_mix_format=@rules_dx//elixir/tools:mix_format
dx format --apply -- --@rules_dx//config:tool_mix_format=@rules_dx//elixir/tools:mix_format
```

The compiler is the type and warning check. No Elixir linter runs yet.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified |
| `linux_arm64` | Not qualified |
| `macos_x86_64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |

On other platforms Elixir targets fail toolchain resolution.

## Coverage

Elixir tests have no coverage collector yet. `dx coverage` fails with `no_coverage_collector` when one runs. Tag a test `no-coverage` to skip it under coverage.
