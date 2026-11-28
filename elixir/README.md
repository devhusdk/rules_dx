# Elixir

This directory helps you build, run and test Elixir code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//elixir:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Elixir page](../docs/languages/elixir.md) shows
setup, every rule and the tools dx runs.

Run the Elixir tests from a rules_dx checkout:

```sh
tools/dx test //elixir/...
```

## Dependencies

### Hex packages from a lock

This is the usual way to get Hex packages. With `mix.exs` and a `mix.lock`
or `rebar.lock` in the set:

1. Run `dx update --apply`. It writes `hex.lock.json`.
2. Run `dx generate --apply`. It adds the `hex.lock` stanza for the `@hex`
   hub to `MODULE.bazel`. A hub is one Bazel repository that holds a target
   for every locked package.
3. Depend on a package as `@hex//jason`.

The hub holds one `elixir_app` per locked package. Packages from
`rebar.lock` get no dependencies. An entry that is not from `hexpm`, such as
a `:git` or `:path` dependency, fails with a message that names it.

### One Hex package by hand

To pin a single package yourself, declare it once in `MODULE.bazel` with its
version and the tarball sha256. Nothing runs `mix deps.get`, and no Hex or
Mix home is read.

```python
hex_package = use_repo_rule("@rules_dx//elixir:repositories.bzl", "hex_package")

hex_package(
    name = "jason",
    build_file = "//third_party:jason.BUILD.bazel",
    package = "jason",
    sha256 = "c5eb0cab91f094599f94d55bc63409236a8ec69a21a67814529e8d5f6cc90b3b",
    version = "1.4.4",
)
```

Your build file declares the package as an `elixir_app` with
`visibility = ["//visibility:public"]`. Depend on that target, for example
`@jason//:jason` when the build file names it `jason`. A wrong sha256 fails
the fetch.

### Adopt a Mix project

1. Add `bazel_dep(name = "rules_dx", version = "...")` to `MODULE.bazel`.
2. Run `bazel run @rules_dx//dx/launch:dx -- generate --apply`.
3. Run `bazel test //...`.

Each directory with a `mix.exs` gets:

- one `elixir_app` from `lib/**/*.ex`;
- one `elixir_test` from `test/**/*_test.exs` and `test/test_helper.exs`.

An umbrella root gets no rule. A module used in code resolves to the
`elixir_app` that defines it, or to the locked Hex package named after it:
`Jason` is `jason`, `Phoenix.LiveView` is `phoenix_live_view`. Modules that
ship with Elixir need no dependency.

What `dx generate` reads and skips:

- From `mix.exs` it reads only `app:`, `version:` and `mod:`. They fill
  `app_name`, `app_version` and `app_module` when the rule has none.
- It does not read `config/*.exs`. Put runtime settings in `app_env`.
- It compiles only `lib/`. `test/support` is not compiled.
- An existing `elixir_app` with the same `app_name` keeps its name and other
  attributes. It must have a `name`.
- It does not find Erlang dependencies such as `:jsx`. Add them to `deps`
  with a `# keep` comment.
- It leaves directories without a `mix.exs` as you wrote them.
- `deps/` and `_build/` next to a `mix.exs` get no rules, so a checkout
  after `mix deps.get` generates cleanly. It does not read a custom
  `deps_path` or `build_path`. Add `# gazelle:exclude <dir>` for it.

## Quality

### Formatting

`dx format` checks Elixir sources with `mix format` from the pinned Elixir.
Select `mix_format` in an `elixir` family in `dx.toml`:

```toml
[families.elixir]
format = ["mix_format"]
```

```sh
dx format
dx format --apply
```

To use another `mix format` binary, pass its label to
`--@rules_dx//config:tool_mix_format`.

### Linting

`dx lint` runs Credo 1.7.19 on the pinned Elixir. Select `credo` in the
`lint` list of an `elixir` family.

- Each finding is a warning named `credo/<Category.Check>`.
- A file Credo cannot parse is an error named `credo/syntax`.
- The default checks are Credo's own non-strict checks. Bind a
  `credo_config` to change them. `requires` and `plugins` are not
  supported.
- The compiler still checks types and warnings.

To use another Credo binary, pass its label to
`--@rules_dx//config:tool_credo`.

### Type checking

`dx typecheck` runs the pinned OTP dialyzer on the compiled `.beam` files of
each `elixir_app`. Select `dialyzer` in the `typecheck` list of an `elixir`
family. Dialyzer checks code against its `@spec` contracts, which the
compiler does not.

- Bazel builds the Elixir PLT from the pinned `elixir` and `logger`
  applications.
- Each finding is a warning named `dialyzer/<rule>`.
- Silence one with `@dialyzer {:nowarn_function, name: arity}`.

To use another dialyzer, pass its label to
`--@rules_dx//config:tool_dialyzer`. Erlang uses the same flag.

## Coverage

Under `dx coverage`, `elixir_test` writes LCOV. The apps in the test's
`deps` that live in your workspace are cover-compiled, and their line hits
are written when the VM exits. Apps from other repositories are not counted.
Tag a test `no-coverage` to skip it.

## Bench and fuzz

`elixir_bench` is an `elixir_test` that `dx bench` runs. rules_dx brings
Benchee 1.5.1.

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

In the test, call `DxBench.run/2` once with a map of names to funs. It runs
Benchee without formatters and writes `dx-bench.json`, one row per job in
ns. The target is tagged `dx-bench`.

There is no `elixir_fuzz_test`: the BEAM has no coverage-guided fuzzer.

## API docs

`dx docs` documents an `elixir_app` with `elixir_doc_shard(name, app, package)`
from `@rules_dx//elixir:defs.bzl`. It reads each module's docs
with `Code.fetch_docs/1` on the pinned Elixir and writes a DocIr shard.

- Modules marked `@moduledoc false` and functions marked `@doc false` are
  left out.
- It does not run on Windows execution platforms.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms. On other
platforms, Elixir targets fail toolchain resolution: Bazel finds no Elixir
toolchain for that platform.
