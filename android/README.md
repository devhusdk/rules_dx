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

## Application packaging

`//android/packaging` builds one installable APK from static native
deps through the qualified upstream backend (`rules_android` 0.6.6):

```python
load("@rules_dx//android/packaging:defs.bzl", "rust_android_apk")

rust_android_apk(
    name = "app_apk",
    app_id = "com.example.app",
    deps = [":app_jni"],
    permissions = ["android.permission.INTERNET"],
    resource_files = glob(["res/**"]),
    assets = glob(["assets/**"]),
    assets_dir = "assets",
)
```

The upstream `android_binary` links one `lib/<name>.so` per ABI from
the static native deps and signs the APK with the debug key. The
default manifest launches a `NativeActivity` loading that library;
pass `manifest` to package a custom one. `app_id`, `permissions`,
`abis`, `api_level`, `version_code`, and `version_name` stay with the
consumer; unknown ABIs, malformed app IDs, and downgraded API levels
fail at analysis. Package outputs are explicit-label only. Custom
release signing and AAB distribution are later stages.

Device operations are explicit launchers that never run at build time:

```python
load("@rules_dx//android/packaging:defs.bzl", "android_adb_install", "android_adb_log", "android_adb_start")

android_adb_install(name = "install", apk = ":app_apk", serial = "emulator-5554")
android_adb_start(name = "start", app_id = "com.example.app", serial = "emulator-5554")
android_adb_log(name = "log", serial = "emulator-5554")
```

Run one with `bazel run //path:install`. `serial` names the target
device or emulator; `ADB` selects the adb binary and defaults to
`PATH`.

## Qualified SDK for packaging builds

Packaging builds read the installed SDK through the upstream
`androidsdk` repository, which stays empty without `ANDROID_HOME`.
Build one APK for both tuples on a qualified host:

```sh
export ANDROID_HOME="$HOME/Android/Sdk" ANDROID_SDK_ROOT="$HOME/Android/Sdk" ANDROID_NDK_REVISION="27.2.12479018"
bazel build //android/tests/fixtures/rust_apk:app_apk \
    --extra_toolchains=@android_ndk_toolchain//:ndk_cc_toolchain_device,@android_ndk_toolchain//:ndk_cc_toolchain_emulator \
    --android_platforms=//android/platforms:android_device,//android/platforms:android_emulator
```

Inspect the artifact with the SDK `aapt`, or run the manual
`inspect_apk_test` in `//android/tests/fixtures/rust_apk`, which
asserts the package ID, SDK floors, label, activity, permission, both
native libraries, the asset bundle, and the debug signature.
