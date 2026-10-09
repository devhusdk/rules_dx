# Android

Acquire one pinned NDK archive plus the minimum SDK package set on an
opt-in basis. Disabled setups fetch no Android payload.

```python
android_sdk = use_extension("@rules_dx//android/sdk:repos.bzl", "android_sdk")

android_sdk.configure(
    accept_licenses = ["android-sdk-license"],
)

use_repo(android_sdk, "dx_android_sdk", "dx_android_ndk_linux_x86_64")
```

Without a `configure` tag only the `dx_android_sdk` hub exists and its
`ndk` target is empty. Pass `mirror` to override the download host,
`local_path` to use an installed SDK with an accepted license instead of
downloading, and `extra_packages` for `platform-tools` or `emulator`.
The qualified host is Linux x86_64 with NDK r27 targeting API 31.

## Native platforms

`//android/platforms` names the two qualified tuples: `device`
(`aarch64-linux-android`) and `emulator` (`x86_64-linux-android`) at API
floor 31. Build one mixed Rust and C library for both tuples with
`//android/rules`:

```python
load("@rules_dx//android/rules:defs.bzl", "android_native_library")

android_native_library(
    name = "native_lib",
    rust_srcs = ["src/lib.rs"],
    c_srcs = ["native.c"],
    hdrs = ["native.h"],
)
```

Each tuple gets a `:name_<tuple>` output carrying `AndroidNativeInfo`
with the triple, API level, and shared library. Unknown tuples,
downgraded API levels, and unknown C++ runtimes fail at analysis.
`crate_features`, `rustc_flags`, `copts`, and `linkopts` reach the
underlying Rust and C actions; build scripts stay on the host toolchain.

## NDK toolchain

`//android/toolchain` defines NDK clang toolchains for both tuples from
an installed SDK:

```python
_android_ndk_toolchain_repo = use_repo_rule("@rules_dx//android/toolchain:repos.bzl", "android_ndk_toolchain_repo")

_android_ndk_toolchain_repo(
    name = "android_ndk_toolchain",
    api_level = 31,
    cxx_runtime = "shared",
)
```

Point it at an SDK with `local_path` or the `ANDROID_SDK_ROOT` and
`ANDROID_NDK_REVISION` environment names, then register
`@android_ndk_toolchain//:ndk_cc_toolchain_device` and
`@android_ndk_toolchain//:ndk_cc_toolchain_emulator`. Without an SDK the
repository stays empty: host tests and metadata still run, while tuple
links and the readelf architecture inspection stay manual until an SDK
host runs them.
