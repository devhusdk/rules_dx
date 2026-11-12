# Elixir

Build and test Elixir with a pinned Elixir 1.20.5 on the pinned OTP 29.1.2
runtime. Nothing is installed on the host and nothing is fetched until a
target needs it. `rules_dx` registers the toolchain.

## Dependencies

### Hex packages

Declare each Hex package once, pinned to a version and the tarball digest.
Nothing runs `mix deps.get` and no Hex or Mix home is read.

```python
hex_package = use_repo_rule("@rules_dx//elixir:extensions.bzl", "hex_package")

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

#### From mix.lock

The `mix` tag reads a `mix.lock` and declares one `hex_package` per entry,
named `<name>_<package>`, with the last checksum as `sha256`. The hub repo
`<name>` has one alias per package.

```python
hex = use_extension("@rules_dx//elixir:extensions.bzl", "hex")
hex.lock(name = "hex_deps", lock = "//:mix.lock")
use_repo(hex, "hex_deps")
```

Depend on `@hex_deps//:jason`. Each package is an `elixir_app` with its
required Hex dependencies; optional ones are skipped. An entry that is not
from `hexpm` (`:git`, `:path`) fails with a message naming it. Erlang
`rebar.lock` is not read yet.

### Adopt a Mix project

rules_dx is not released yet. Install starts with the first BCR release.

1. Add `bazel_dep(name = "rules_dx", version = "...")` to `MODULE.bazel`.
2. Run `bazel run @rules_dx//:dx -- generate --apply`.
3. Add the `hex.lock` tag for your `mix.lock` (see above).
4. Run `dx generate --apply` again. A module resolves to the `mix.lock` package named after it (`Jason` is `jason`, `Phoenix.LiveView` is `phoenix_live_view`).
5. Run `bazel test //...`.

Each directory with a `mix.exs` gets one `elixir_app` from `lib/**/*.ex` and
one `elixir_test` from `test/**/*_test.exs` and `test/test_helper.exs`. An
umbrella root gets no rule. A module named in code resolves to the
`elixir_app` that defines it. Modules that ship with Elixir need no dependency.

- `mix.exs` is read only for `app:`, `version:` and `mod:`. They fill
  `app_name`, `app_version` and `app_module` when the rule lacks them.
- `mix.lock` and `config/*.exs` are not read. Put runtime settings in
  `app_env`.
- Only `lib/` is compiled. `test/support` is not.
- An existing `elixir_app` with the same `app_name` keeps its name and other
  attributes. It must have a `name`.
- Erlang dependencies such as `:jsx` are not detected. Add them to `deps`
  with a `# keep` comment.
- Directories without a `mix.exs` are left as written.
- `deps/` and `_build/` next to a `mix.exs` get no rules, so a checkout after
  `mix deps.get` generates cleanly. A custom `deps_path` or `build_path` is
  not read. Add `# gazelle:exclude <dir>` for it.

## Quality

### Formatting

`dx format` checks Elixir sources with `mix format` from the pinned Elixir.
Select `mix_format` in an `elixir` family. The tool flag overrides the pinned binary:

```sh
dx format -- --@rules_dx//config:tool_mix_format=@rules_dx//elixir/private/tools:mix_format
dx format --apply -- --@rules_dx//config:tool_mix_format=@rules_dx//elixir/private/tools:mix_format
```

### Linting

`dx lint` runs Credo 1.7.19 on the pinned Elixir. Select `credo` in the `lint`
list of an `elixir` family. The tool flag overrides the pinned binary:

```sh
dx lint -- --@rules_dx//config:tool_credo=@rules_dx//elixir/private/tools:credo
```

Findings are warnings named `credo/<Category.Check>`. A file Credo cannot parse
is an error named `credo/syntax`. The defaults are Credo's own non-strict
checks. Bind a `credo_config` to change them; `requires` and `plugins` are not
supported. The compiler is still the type and warning check.

### Type checking

`dx typecheck` runs the OTP 29.1.2 dialyzer on the compiled beams of each
`elixir_app`. Select `dialyzer` in the `typecheck` list of an `elixir` family
The tool flag, shared with Erlang, overrides the pinned binary:

```sh
dx typecheck -- --@rules_dx//config:tool_dialyzer=@rules_dx//erlang/private/tools:dialyzer
```

Dialyzer checks code against its `@spec` contracts, which the compiler's own
type checker does not. The Elixir PLT is built by Bazel from the pinned Elixir
`elixir` and `logger` applications. Findings are warnings named
`dialyzer/<rule>`. Silence one with `@dialyzer {:nowarn_function, name: arity}`.
See `docs/cli/commands/quality.md`.

## Coverage

`elixir_test` writes LCOV under `dx coverage`. A script cover-compiles the workspace apps in the test's `deps` and writes their line hits when the VM exits. Apps from other repositories are not counted. Tag a test `no-coverage` to skip it.

## Bench and fuzz

`elixir_bench` is an `elixir_test` for `dx bench`. Benchee 1.5.1 comes with rules_dx. Call `DxBench.run/2` once in the test with a map of names to funs. It runs Benchee without formatters and writes `dx-bench.json`, one row per job in ns.

```python
load("@rules_dx//elixir:defs.bzl", "elixir_bench")

elixir_bench(
    name = "sum_bench",
    srcs = [
        "sum_bench.exs",
        "test_helper.exs",
    ],
)
```

The target is tagged `dx-bench`. Benchee has no coverage-guided fuzzer on the BEAM, so there is no `elixir_fuzz_test`.

## API docs

`dx docs` documents an `elixir_app` with `elixir_doc_shard(name, app, package)` from
`@rules_dx//tools/docs/elixir:defs.bzl`. It reads each module's docs chunk with `Code.fetch_docs/1` on the pinned Elixir and writes a DocIr shard. Modules marked `@moduledoc false` and functions marked `@doc false` are left out. Windows execution platforms are not covered.

## Platforms

See the [support matrix](../docs/support-matrix.md).

On other platforms Elixir targets fail toolchain resolution.
