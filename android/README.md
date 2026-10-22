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
    cxx_runtime = "static",
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

The upstream `android_binary` links one `lib/<abi>/lib<name>.so` per ABI
from the static native deps and signs the APK with the debug key. Builds
never use a release key. Build
`:app_apk_file` for the installable `app_apk.apk`; it selects the
`arm64-v8a` and `x86_64` platforms from `abis`. The native deps need
`alwayslink = True` so `ANativeActivity_onCreate` stays exported, and
`-landroid -llog` as link options when they use the NDK APIs. The
default manifest launches a `NativeActivity` loading that library;
pass `manifest` to package a custom one. `app_id`, `permissions`,
`abis`, `api_level`, `version_code`, and `version_name` stay with the
consumer; unknown ABIs, empty ABI sets, malformed app IDs, downgraded
API levels, non-positive version codes, and empty launch activities
fail at analysis, as do adb launchers without a serial. Package outputs are explicit-label only. AAB
distribution is a later stage.

The upstream resource tool crashes on its pinned protobuf-java. A
module that depends on `rules_dx` applies the patch in its own
`MODULE.bazel`, because overrides only work in the root module:

```python
single_version_override(
    module_name = "rules_android",
    patch_strip = 1,
    patches = ["@rules_dx//patches/rules_android:rules_android.patch"],
    version = "0.6.6",
)
```

Device operations are explicit launchers that never run at build time:

```python
load("@rules_dx//android/packaging:defs.bzl", "android_adb_install", "android_adb_log", "android_adb_start")

android_adb_install(name = "install", apk = ":app_apk_file", serial = "emulator-5554")
android_adb_start(name = "start", app_id = "com.example.app", serial = "emulator-5554")
android_adb_log(name = "log", serial = "emulator-5554")
```

Run one with `bazel run //path:install`. `serial` names the target
device or emulator; `ADB` selects the adb binary and defaults to
`PATH`.

## Release signing

`:app_apk_unsigned_file` is the unsigned package. Sign it with an
explicit launcher that runs only when you ask:

```python
load("@rules_dx//android/packaging:defs.bzl", "android_apk_check", "android_apk_release_sign")

android_apk_release_sign(name = "sign", apk = ":app_apk_unsigned_file")
android_apk_check(name = "sign_check", apk = ":app_apk_unsigned_file")
```

`bazel test :sign_check` validates the package without signing: it must
be a readable zip with an `AndroidManifest.xml` and no existing
signature. `schemes` picks from `v1`, `v2`, `v3` and `v4` (default
`v1`, `v2`, `v3`) and `min_sdk` sets the signing minimum SDK (default
the manifest value, at least 31). Unknown or duplicate schemes and a
low `min_sdk` fail at analysis.

Pass the key through the environment of the run, never through BUILD
files:

```sh
export DX_SIGN_KEYSTORE=release.jks DX_SIGN_STORE_PASS_FILE=store.pass
export DX_SIGN_KEY_ALIAS=release DX_SIGN_KEY_PASS_FILE=key.pass DX_SIGN_OUT=app-release.apk
export ANDROID_HOME="$HOME/Android/Sdk"
bazel run //path:sign
```

Passwords are read from the named files only. `DX_SIGN_KEY_ALIAS` and
`DX_SIGN_KEY_PASS_FILE` are optional. Relative paths resolve against
the directory you ran from. The launcher aligns the APK with the pinned
build-tools `zipalign`, signs it with `apksigner`, verifies the result,
and only then writes `DX_SIGN_OUT`. Missing inputs exit 2, missing build
tools exit 127, and a wrong password, wrong alias or malformed package
exits 1 without writing an APK. Apksigner enables the schemes the
minimum SDK needs, so an API 31 package carries a v3 signature.

Run `bazel test //android/tests/fixtures/rust_apk:sign_test
--test_env=ANDROID_HOME` on an SDK host to sign a package with a
generated test key and verify it.

## Qualified SDK for packaging builds

Packaging builds read the installed SDK through the upstream
`androidsdk` repository, which stays empty without `ANDROID_HOME`.
Build one APK for both tuples on a qualified host:

```sh
export ANDROID_HOME="$HOME/Android/Sdk" ANDROID_SDK_ROOT="$HOME/Android/Sdk" ANDROID_NDK_REVISION="27.2.12479018"
bazel build //android/tests/fixtures/rust_apk:app_apk_file \
    --extra_toolchains=@android_ndk_toolchain//:ndk_cc_toolchain_device,@android_ndk_toolchain//:ndk_cc_toolchain_emulator
```

Inspect the artifact with the SDK `aapt`, or run the manual
`inspect_apk_test` and `inspect_custom_apk_test` in
`//android/tests/fixtures/rust_apk` with `--test_env=ANDROID_HOME` and
the same `--extra_toolchains`. They assert the package ID, version, SDK
floors, label, activity, library name, permissions, the requested ABI
directories, the exported `ANativeActivity_onCreate`, system-only
`NEEDED` entries, the asset bundle, and the debug signature. The custom
target packages `x86_64` only at API 33 with its own ID and permissions.

The fixture app logs `lifecycle=`, `input=` and `asset=` lines under the
`DxRustApk` tag. After `bazel run //android/tests/fixtures/rust_apk:install`
and `:start`, touch the screen and run `:log` to see them.

## Backend decision

Packaging stays on the upstream backend: `rust_android_apk` composes
`android_binary` from `rules_android` 0.6.6, so manifest merge,
resources, native ABI placement, DEX, and signing come from the
upstream rules plus the installed SDK. There is no owned APK
packager: a native-only reimplementation would still need the same
package and signature semantics without shedding supported behavior.

APK assembly and device install/launch need a qualified SDK host (see
above). Selection, negative, and adapter coverage runs everywhere:

```sh
bazel test //android/...
```
