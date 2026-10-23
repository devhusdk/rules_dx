# Erlang/OTP

Build and test Erlang with a pinned OTP 29.1.1 runtime. Nothing is installed
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

`DEFAULT_ERLC_OPTS` and `DEFAULT_TEST_ERLC_OPTS` hold the default flags.

Tests run without `epmd` and without a distributed node. `ct_suite` writes its
logs to the test's undeclared outputs.

`dx format` checks Erlang sources with erlfmt 1.8.0, built from source on the
pinned runtime. Select `erlfmt` in an `erlang` family and pass the tool flag:

```sh
dx format -- --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/tools:erlfmt
dx format --apply -- --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/tools:erlfmt
```

No Erlang linter runs yet.

## Runtime

The OTP build comes from `builds.hex.pm` for Ubuntu 22.04. Its shared
libraries (`libtinfo`, `zlib`, `libgcc_s`, `libstdc++`, `libcrypto`) come
from pinned Ubuntu packages, so the host needs only glibc 2.35 or newer. The
`wx` application, sources and docs are dropped. The library license texts
ship in the OTP runtime tree under `dx_closure/licenses`.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified |
| `linux_arm64` | Not qualified |
| `macos_x86_64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |

On other platforms Erlang targets fail toolchain resolution.
