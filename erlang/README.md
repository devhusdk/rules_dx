# Erlang/OTP

Build and test Erlang with a pinned OTP 29.1.2 runtime. Nothing is installed
on the host and nothing is fetched until a target needs it. rules_dx
registers the toolchain, so there is no setup in `MODULE.bazel`.

## Rules

Load the rules from `@rules_dx//erlang/rules:defs.bzl`. They wrap
[rules_erlang](https://github.com/bazelverse/rules_erlang) and take the same
attributes.

```python
load("@rules_dx//erlang/rules:defs.bzl", "ct_suite", "erlang_app", "escript_archive", "eunit", "test_erlang_app")

erlang_app(
    app_name = "kv_store",
    app_version = "1.0.0",
    app_module = "kv_store_app",
    app_env = "[{max_keys, 3}]",
    deps = ["//proto:erlang_app"],
)

test_erlang_app(
    app_name = "kv_store",
    app_version = "1.0.0",
    deps = ["//proto:erlang_app"],
)

ct_suite(name = "kv_store_SUITE")

escript_archive(
    name = "kvctl",
    app = ":erlang_app",
    headers = ["shebang", "{emu_args, \"-escript main kvctl\"}"],
)
```

| Rule | What it makes |
| --- | --- |
| `erlang_app` | OTP application from `src/`, `include/` and `priv/`, compiled with `-Werror` |
| `test_erlang_app` | The same application compiled with `-DTEST=1` for tests |
| `erlang_bytecode` | `.beam` files from `srcs`, with `outs` and an `erlc_opts` target |
| `erlc_opts` | Reusable `erlc` flags |
| `erlang_xrl_yrl` | `.erl` files from leex `.xrl` and yecc `.yrl` grammars |
| `eunit` | EUnit test over `target` and `compiled_suites` |
| `ct_suite` | Common Test suite from `test/<name>.erl` with `test/<name>_data/` |
| `escript_archive` | Escript with the app and its deps, runnable with `bazel run` |
| `erlang_nif` | NIF shared library from declared C or C++ `srcs` and `hdrs`, placed at `priv/<name>.so` |

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

Declare the NIF in the package of the app that loads it and pass it through
`nifs` on `erlang_app` and `test_erlang_app`. The C includes `erl_nif.h` from
the pinned OTP. `erlang:load_nif/2` takes `priv/<name>` without the extension.
An `escript_archive` whose app, or any app it bundles, ships a NIF fails at
analysis time: Erlang cannot load a NIF from inside an escript archive. Run the
app with `erlang_app`; this refusal is final, releases that carry NIFs are
tracked separately. Each NIF links its own
static libc++ on Linux and uses the SDK's libc++ on macOS. Only `nif_init` is
exported (version script on Linux, `-exported_symbol` on macOS), so the runtime
never binds to beam's libstdc++. C++ exceptions must not cross the NIF
boundary, and NIFs cannot rely on exporting other symbols. Under `bazel coverage` the C
lines join the Erlang lines in the report.

`erlang_app` and `test_erlang_app` pass `tags` to every target they build; the
generated targets are `manual`, so `//...` builds only the app.

`DEFAULT_ERLC_OPTS` and `DEFAULT_TEST_ERLC_OPTS` hold the default flags.

Tests run without `epmd` and without a distributed node. `ct_suite` writes its
logs to the test's undeclared outputs.

`dx format` checks Erlang sources with erlfmt 1.8.0, built from source on the
pinned runtime. Select `erlfmt` in an `erlang` family and pass the tool flag:

```sh
dx format -- --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/tools:erlfmt
dx format --apply -- --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/tools:erlfmt
```

`dx lint` checks Erlang sources with Elvis 6.0.0. Select `elvis` in an
`erlang` family and pass the tool flag:

```sh
dx lint -- --@rules_dx//config:tool_elvis=@rules_dx//erlang/tools:elvis
```

`dx typecheck` checks the compiled beams of each `erlang_app` with the OTP 29.1.2
dialyzer. Select `dialyzer` in an `erlang` family and pass the tool flag:

```sh
dx typecheck -- --@rules_dx//config:tool_dialyzer=@rules_dx//erlang/tools:dialyzer
```

The base PLT is built by Bazel from the pinned OTP tree. See
`docs/cli/commands/quality.md` for the PLT application list and the rules.
The tool is shared with Elixir: an `elixir_app` adds its own PLT.

## Runtime

On Linux the OTP build comes from `builds.hex.pm` for Ubuntu 22.04. Its
shared libraries (`libtinfo`, `zlib`, `libgcc_s`, `libstdc++`, `libcrypto`)
come from pinned Ubuntu packages, so the host needs only glibc 2.35 or newer.
The library license texts ship in the OTP runtime tree under
`dx_closure/licenses`.

On macOS the OTP build comes from `erlef/otp_builds`. OpenSSL is linked into
it, so it needs no extra libraries.

The `wx` application, sources and docs are dropped on every cell.

## Platforms

| Cell | Status | Reason |
| --- | --- | --- |
| `linux_x86_64` | Qualified | |
| `linux_arm64` | Qualified | |
| `macos_arm64` | Qualified | |
| `macos_x86_64` | Not qualified | No CI cell. |
| `windows_x86_64` | Not qualified | The `erl` launcher is POSIX sh and rules_erlang is not ported to Windows. |
| `windows_arm64` | Not qualified | No upstream OTP build. |

Elixir targets still run only on `linux_x86_64`. On other platforms
Erlang and Elixir targets are skipped.

## Generating targets

`dx generate` writes an `erlang_app` for a directory that holds a
`src/*.app.src` or a `rebar.config`. It lists `src/**/*.erl`, `src/**/*.hrl`
and `include/**/*.hrl`, reads the name, `vsn`, `description` and `mod` from
the `.app.src`, and resolves remote calls, `-behaviour`, `-import` and
`-parse_transform` to other `erlang_app` rules. OTP applications beyond
kernel and stdlib go to `extra_apps`. Sources under `c_src` become one
`erlang_nif` in the same package, wired through `nifs`. Test rules and
`rebar.config` dependencies are not generated yet.
