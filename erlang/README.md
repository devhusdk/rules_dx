# Erlang

This directory helps you build, run and test Erlang code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//erlang:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Erlang page](../docs/languages/erlang.md) shows
setup, every rule and the tools dx runs.

Run the Erlang tests from a rules_dx checkout:

```sh
tools/dx test //erlang/...
```

## Dependencies

### Locked Hex packages

1. Run `dx update --apply`. It turns `rebar.lock` into `hex.lock.json`.
2. Depend on a package through the `hex` hub, for example `@hex//cowboy`.
   A hub is one Bazel repository that holds a target for every locked
   package.

The lock is also a `hex` dependency set for `dx security`, `dx license` and
`dx update`. `dx update` refreshes it with the pinned rebar3.

## Generate

`dx generate` writes an `erlang_app` for each directory that holds a
`src/*.app.src` or a `rebar.config`.

- `srcs` lists `src/**/*.erl`, `src/**/*.hrl` and `include/**/*.hrl`.
- The name, `vsn`, `description` and `mod` come from the `.app.src`.
- Remote calls, `-behaviour`, `-import` and `-parse_transform` resolve to
  other `erlang_app` rules.
- OTP applications other than kernel and stdlib go to `extra_apps`.
- Sources under `c_src` become one `erlang_nif` in the same package, wired
  through `nifs`.

`dx generate` writes no test rules and does not read `rebar.config`
dependencies. Write those by hand.

## Bench and fuzz

`erlang_bench` is a `ct_suite` that `dx bench` runs. rules_dx brings
erlperf 2.3.0.

```python
load("@rules_dx//erlang:defs.bzl", "erlang_bench")

erlang_bench(
    name = "sum_bench_SUITE",
    app_name = "sum",
)
```

Like `ct_suite`, it needs the package's `erlang_app` and `test_erlang_app`.
In a test case, call `dx_bench:run/1,2` with a map of names to funs. It
measures each fun with erlperf and writes `dx-bench.json`, one row per job
in ops/s. Pass erlperf run options as the second argument. The target is
tagged `dx-bench`.

There is no `erlang_fuzz_test`: the BEAM has no coverage-guided fuzzer.

## API docs

`dx docs` documents an `erlang_app` with `erlang_doc_shard(name, app, package)`
from `@rules_dx//erlang:defs.bzl`. It runs `edoc` from the pinned
OTP over the app's `srcs` and writes a DocIr shard.

- Functions that are not exported are left out.
- It does not run on Windows execution platforms.

## Toolchain

### Runtime

Where the pinned OTP comes from, and what the host needs:

| Platform | OTP build | Host needs |
| --- | --- | --- |
| Linux | `builds.hex.pm`, Ubuntu 22.04 | glibc 2.35 or newer |
| macOS | `erlef/otp_builds`, OpenSSL linked in | nothing extra |
| Windows | upstream `otp_win64` zip | the MSVC runtime (`vc_redist`) |

On Linux, the shared libraries OTP loads (`libtinfo`, `zlib`, `libgcc_s`,
`libstdc++`, `libcrypto`) come from pinned Ubuntu packages. Their license
texts ship in the OTP runtime tree under `dx_closure/licenses`.

No platform ships the `wx` application.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms.

- `windows_x86_64`: tools and escripts run through a native launcher.
- `windows_arm64`: the x64 OTP runs under emulation, and NIFs are built for
  x64 Windows.

On other platforms, Erlang and Elixir targets are skipped.
