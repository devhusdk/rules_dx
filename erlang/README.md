# Erlang/OTP

Build and test Erlang with a pinned OTP 29.1.2 runtime. Nothing is installed
on the host and nothing is fetched until a target needs it. `rules_dx`
registers the toolchain.

## Rules

Load the rules from `@rules_dx//erlang:defs.bzl`. They wrap
[rules_erlang](https://github.com/bazelverse/rules_erlang) and take the same
attributes.

```python
load("@rules_dx//erlang:defs.bzl", "erlang_app", "erlang_ct_suite_test", "erlang_escript_archive", "erlang_eunit_test", "erlang_test_app")

erlang_app(
    app_name = "kv_store",
    app_version = "1.0.0",
    app_module = "kv_store_app",
    app_env = "[{max_keys, 3}]",
    deps = ["//proto:erlang_app"],
)

erlang_test_app(
    app_name = "kv_store",
    app_version = "1.0.0",
    deps = ["//proto:erlang_app"],
)

erlang_ct_suite_test(name = "kv_store_SUITE")

erlang_escript_archive(
    name = "kvctl",
    app = ":erlang_app",
    headers = ["shebang", "{emu_args, \"-escript main kvctl\"}"],
)
```

| Rule | What it makes |
| --- | --- |
| `erlang_app` | OTP application from `src/`, `include/` and `priv/`, compiled with `-Werror` |
| `erlang_test_app` | The same application compiled with `-DTEST=1` for tests |
| `erlang_bytecode` | `.beam` files from `srcs`, with `outs` and an `erlc_opts` target |
| `erlc_opts` | Reusable `erlc` flags |
| `erlang_xrl_yrl` | `.erl` files from leex `.xrl` and yecc `.yrl` grammars |
| `erlang_eunit_test` | EUnit test over `target` and `compiled_suites` |
| `erlang_ct_suite_test` | Common Test suite from `test/<name>.erl` with `test/<name>_data/` |
| `erlang_escript_archive` | Escript with the app and its deps, runnable with `bazel run` |
| `erlang_release` | Release tree with `lib/<app>/ebin` and `lib/<app>/priv`, NIF libraries included |
| `erlang_nif` | NIF shared library from declared C or C++ `srcs` and `hdrs`, other `cc_library` attributes pass through, placed at `priv/<name>.so` (`.dll` on Windows) |

```python
erlang_nif(
    name = "kv_checksum_nif",
    srcs = ["c_src/kv_checksum_nif.c"],
    hdrs = ["c_src/fnv1a.h"],
)

erlang_app(
    app_name = "kv_checksum",
    app_version = "1.0.0",
    nifs = [":kv_checksum_nif"],
)
```

Only `<name>` is public; the helper targets are private. A `target_compatible_with` you pass is added to the NIF platform restriction. Declare the NIF in the package of the app that loads it and pass it through
`nifs` on `erlang_app` and `erlang_test_app`. The C includes `erl_nif.h` from
the pinned OTP. `erlang:load_nif/2` takes `priv/<name>` without the extension.
An `erlang_escript_archive` whose app, or any app it bundles, ships a NIF fails at
analysis time: Erlang cannot load a NIF from inside an escript archive. Run the
app with `erlang_app`; this refusal is final. An `erlang_release` carries the NIFs: it places each
app's libraries in `lib/<app>/priv`. Each NIF links its own
static libc++ on Linux and uses the SDK's libc++ on macOS. Only `nif_init` is
exported (version script on Linux, `-exported_symbol` on macOS), so the runtime
never binds to beam's libstdc++. On Windows `ERL_NIF_INIT` exports `nif_init`. C++ exceptions must not cross the NIF
boundary, and NIFs cannot rely on exporting other symbols. Under `bazel coverage` the C
lines join the Erlang lines in the report.

`erlang_app` and `erlang_test_app` pass `tags` to every target they build; the
generated targets are `manual`, so `//...` builds only the app.

`DEFAULT_TEST_ERLC_OPTS` holds the default test flags.

Tests run without `epmd` and without a distributed node. `erlang_ct_suite_test` writes its
logs to the test's undeclared outputs.

`dx format` checks Erlang sources with erlfmt 1.8.0, built from source on the
pinned runtime. Select `erlfmt` in an `erlang` family. The tool flag overrides the pinned binary:

```sh
dx format -- --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/private/tools:erlfmt
dx format --apply -- --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/private/tools:erlfmt
```

`dx lint` checks Erlang sources with Elvis 6.0.0. Select `elvis` in an
`erlang` family. The tool flag overrides the pinned binary:

```sh
dx lint -- --@rules_dx//config:tool_elvis=@rules_dx//erlang/private/tools:elvis
```

`dx typecheck` checks the compiled beams of each `erlang_app` with the OTP 29.1.2
dialyzer. Select `dialyzer` in an `erlang` family. The tool flag overrides the pinned binary:

```sh
dx typecheck -- --@rules_dx//config:tool_dialyzer=@rules_dx//erlang/private/tools:dialyzer
```

The base PLT is built by Bazel from the pinned OTP tree. See
`docs/cli/commands/quality.md` for the PLT application list and the rules.
The tool is shared with Elixir: an `elixir_app` adds its own PLT.

## Benchmarks

`erlang_bench` is a `ct_suite` for `dx bench`. erlperf 2.3.0 comes with rules_dx. Call `dx_bench:run/1,2` in a test case with a map of names to funs. It measures each fun with erlperf and writes `dx-bench.json`, one row per job in ops/s. Pass erlperf run options as the second argument.

```python
load("@rules_dx//erlang:defs.bzl", "erlang_bench")

erlang_bench(
    name = "sum_bench_SUITE",
    app_name = "sum",
)
```

It needs the `erlang_app` and `test_erlang_app` of the package, like `ct_suite`. The target is tagged `dx-bench` and `manual`. No coverage-guided fuzzer exists for the BEAM, so there is no `erlang_fuzz_test`.

## Runtime

On Linux the OTP build comes from `builds.hex.pm` for Ubuntu 22.04. Its
shared libraries (`libtinfo`, `zlib`, `libgcc_s`, `libstdc++`, `libcrypto`)
come from pinned Ubuntu packages, so the host needs only glibc 2.35 or newer.
The library license texts ship in the OTP runtime tree under
`dx_closure/licenses`.

On macOS the OTP build comes from `erlef/otp_builds`. OpenSSL is linked into
it, so it needs no extra libraries.

On Windows the OTP build is the upstream `otp_win64` zip. The host needs the
MSVC runtime (`vc_redist`); the installer in the zip is dropped.

The `wx` application, sources and docs are dropped on every cell.

## Platforms

| Cell | Status | Reason |
| --- | --- | --- |
| `linux_x86_64` | Qualified | |
| `linux_arm64` | Qualified | |
| `macos_arm64` | Qualified | |
| `macos_x86_64` | Not qualified | No CI cell. |
| `windows_x86_64` | Qualified | Tools and escripts run through the native launcher, including dialyzer. |
| `windows_arm64` | Qualified | x64 OTP under emulation; NIFs are built for x64 Windows. |

On other platforms Erlang and Elixir targets are skipped.

## Generating targets

`dx generate` writes an `erlang_app` for a directory that holds a
`src/*.app.src` or a `rebar.config`. It lists `src/**/*.erl`, `src/**/*.hrl`
and `include/**/*.hrl`, reads the name, `vsn`, `description` and `mod` from
the `.app.src`, and resolves remote calls, `-behaviour`, `-import` and
`-parse_transform` to other `erlang_app` rules. OTP applications beyond
kernel and stdlib go to `extra_apps`. Sources under `c_src` become one
`erlang_nif` in the same package, wired through `nifs`. Test rules and
`rebar.config` dependencies are not generated yet.

## Locked Hex packages

`erlang.rebar(name, lock)` in `MODULE.bazel` reads a `rebar.lock` (format
1.2.0) and declares one `hex_package` per `pkg` entry, checked against the
`pkg_hash_ext` sha256, plus a hub repository named `name` with one
`erlang_app` alias per package. Each package depends on every package locked
at a deeper level. Git entries are refused. The lock also becomes a `hex`
dependency set for `dx audit` and `dx update`; `dx update` refreshes it with
the pinned rebar3.
