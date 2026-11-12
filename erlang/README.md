# Erlang

Build and test Erlang with a pinned OTP 29.1.2 runtime. Nothing is installed
on the host and nothing is fetched until a target needs it. `rules_dx`
registers the toolchain.

## Dependencies

### Locked Hex packages

`erlang.rebar(name, lock)` in `MODULE.bazel` reads a `rebar.lock` (format
1.2.0) and declares one `hex_package` per `pkg` entry, checked against the
`pkg_hash_ext` sha256, plus a hub repository named `name` with one
`erlang_app` alias per package. Each package depends on every package locked
at a deeper level. Git entries are refused. The lock also becomes a `hex`
dependency set for `dx security`, `dx license` and `dx update`; `dx update` refreshes it with
the pinned rebar3.

## Generate

`dx generate` writes an `erlang_app` for a directory that holds a
`src/*.app.src` or a `rebar.config`. It lists `src/**/*.erl`, `src/**/*.hrl`
and `include/**/*.hrl`, reads the name, `vsn`, `description` and `mod` from
the `.app.src`, and resolves remote calls, `-behaviour`, `-import` and
`-parse_transform` to other `erlang_app` rules. OTP applications beyond
kernel and stdlib go to `extra_apps`. Sources under `c_src` become one
`erlang_nif` in the same package, wired through `nifs`. Test rules and
`rebar.config` dependencies are not generated yet.

## Bench and fuzz

`erlang_bench` is a `ct_suite` for `dx bench`. erlperf 2.3.0 comes with rules_dx. Call `dx_bench:run/1,2` in a test case with a map of names to funs. It measures each fun with erlperf and writes `dx-bench.json`, one row per job in ops/s. Pass erlperf run options as the second argument.

```python
load("@rules_dx//erlang:defs.bzl", "erlang_bench")

erlang_bench(
    name = "sum_bench_SUITE",
    app_name = "sum",
)
```

It needs the `erlang_app` and `test_erlang_app` of the package, like `ct_suite`. The target is tagged `dx-bench`. No coverage-guided fuzzer exists for the BEAM, so there is no `erlang_fuzz_test`.

## API docs

`dx docs` documents an `erlang_app` with `erlang_doc_shard(name, app, package)` from
`@rules_dx//tools/docs/erlang:defs.bzl`. It runs `edoc` from the pinned OTP over the app's `srcs` and writes a DocIr shard. Functions that are not exported are left out. Windows execution platforms are not covered.

## Toolchain

### Runtime

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

See the [support matrix](../docs/support-matrix.md).

- `windows_x86_64`: tools and escripts run through the native launcher.
- `windows_arm64`: x64 OTP runs under emulation; NIFs are built for x64 Windows.

On other platforms Erlang and Elixir targets are skipped.
