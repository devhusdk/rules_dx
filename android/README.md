# Android

Acquire pinned Android SDK packages on an opt-in basis. Disabled setups
fetch no Android payload.

```python
android_sdk = use_extension("@rules_dx//android/sdk:repos.bzl", "android_sdk")

android_sdk.configure(
    accept_licenses = ["android-sdk-license"],
    # api_level = 31,                       # 31-34, default 34
    # extra_packages = ["platform-tools"],  # and/or "emulator"
)

use_repo(
    android_sdk,
    "dx_android_sdk",
    "dx_android_ndk_linux_x86_64",
    "dx_android_build_tools_linux_x86_64",
    "dx_android_platform",
)
```

What is fetched, each only when a build needs it:

- NDK r27 (`dx_android_ndk_linux_x86_64`)
- build-tools 34.0.0 (`dx_android_build_tools_linux_x86_64`)
- `platforms;android-<api_level>` for `api_level` 31 to 34, default 34
  (`dx_android_platform`; the name does not depend on the level)
- opt-in through `extra_packages`: `platform-tools` r37.0.1
  (`dx_android_platform_tools_linux_x86_64`) and `emulator` 16428233
  (`dx_android_emulator_linux_x86_64`, about 350 MB). The emulator ships
  no system image.

Any other `extra_packages` entry, a repeated entry, or an `api_level`
other than 31 to 34 fails with an error naming the allowed values.

The `dx_android_sdk` hub exposes `ndk`, `zipalign`, `apksigner_jar`,
`bundletool_jar`, `android_jar`, `adb` and `emulator`. Targets for packages
that were not requested are empty. Without a `configure` tag only the hub
exists. Pass `mirror` to override the download host.

`local_path` uses an installed SDK with an accepted license instead of
downloading. The NDK must be present. An explicit `api_level` or a requested
extra package must exist in the SDK, otherwise the error names the missing
path. Without them a missing android-34 platform leaves `android_jar` empty.
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

The package links one `lib/<abi>/lib<name>_bin.so` per ABI from the
static native deps and signs the APK with the debug key. Builds never
use a release key. Build `:app_apk` for the installable `app_apk.apk`;
it selects the platforms named in `abis`, and an unknown ABI fails. The
native deps need `alwayslink = True` so `ANativeActivity_onCreate` stays
exported, and `-landroid -llog` as link options when they use the NDK
APIs. The default manifest launches a `NativeActivity` loading that
library; pass `manifest` to package a custom one. Pass Java `srcs` and a
`manifest` naming your own activity to package a Java bridge, and load
the library with `System.loadLibrary("<name>_bin")`. A custom
`NativeActivity` manifest must set `android.app.lib_name` to
`<name>_bin`. `app_id`, `permissions`, `abis`, `api_level`,
`version_code`, and `version_name` stay with the consumer; unknown
ABIs, empty ABI sets, malformed app IDs, downgraded API levels,
non-positive version codes, and empty launch activities fail at
analysis, as do adb launchers without a serial.
Package outputs are explicit-label only. See [App Bundles](#app-bundles)
for the `.aab` output.

`dx init --apply` writes the rules_android override; see
[patches/README.md](../patches/README.md).

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
the manifest value, at least 31). Unknown or duplicate schemes, `v4`
without `v2` or `v3`, and a low `min_sdk` fail at analysis. A run with
`v4` also writes the signature next to the APK as `<DX_SIGN_OUT>.idsig`;
a run without `v4` deletes a stale one.

Pass the key through the environment of the run, never through BUILD
files:

```sh
export DX_SIGN_KEYSTORE=release.jks DX_SIGN_STORE_PASS_FILE=store.pass
export DX_SIGN_KEY_ALIAS=release DX_SIGN_KEY_PASS_FILE=key.pass DX_SIGN_OUT=app-release.apk
bazel run //path:sign --java_runtime_version=remotejdk_21
```

Passwords are read from the named files only. `DX_SIGN_KEY_ALIAS` and
`DX_SIGN_KEY_PASS_FILE` are optional. Relative paths resolve against
the directory you ran from. The launcher aligns the APK with the
acquired build-tools `zipalign`, signs it with `apksigner` on the Java
runtime that `--java_runtime_version` selects, verifies the result, and
only then writes `DX_SIGN_OUT`. Set `--java_runtime_version`, for
example to `remotejdk_21`; the default `local_jdk` is the host JDK.
Neither `ANDROID_HOME` nor `PATH` supplies a tool. Missing inputs exit
2. Missing build-tools (no `configure` tag) or no Java runtime exit 127.
A wrong password, wrong alias or malformed package exits 1 without
writing an APK. Apksigner enables the schemes the
minimum SDK needs, so an API 31 package carries a v3 signature.

Run `bazel test //android/tests/fixtures/rust_apk:sign_test` to sign a
package with a generated test key and verify it.

## App Bundles

`rust_android_apk` also declares `:<name>_aab_file`. It builds the
unsigned App Bundle `<name>_bundle_unsigned.aab` from the same
`android_binary`, so the bundle keeps the app ID, version, API floor,
ABIs, resources, assets and native library of the APK. For the fixture,
`:app_apk_aab_file` builds `app_apk_bundle_unsigned.aab`.

Check a bundle with a test:

```python
load("@rules_dx//android/packaging:defs.bzl", "android_aab_check")

android_aab_check(
    name = "aab_check",
    aab = ":app_apk_aab_file",
    abis = ["arm64-v8a", "x86_64"],
    resources = ["string/app_name"],
)
```

| Attribute | Default | Meaning |
| --- | --- | --- |
| `aab` | required | The `.aab` to check. |
| `abis` | `[]` | ABIs that must ship a `base/lib/<abi>/*.so`. |
| `resources` | `[]` | `type/name` resources the bundle must resolve. |
| `size`, `tags`, `visibility` | `"small"`, none, none | Passed to the test. |

The test runs `bundletool validate`, reads the base module manifest,
checks each ABI entry, and resolves each resource with
`bundletool dump resources`. It exits 0 for a valid bundle, 1 when the
bundle is rejected, and 2 for a missing bundle, a missing bundletool or
bad arguments. Unknown ABIs and resources that are not `type/name` fail
at analysis.

The check runs the pinned bundletool 1.15.5 as
`//android/packaging:bundletool` on the rules_java JDK. The jar is
downloaded with a checksum through the `android_sdk` extension.

Build and check the fixture bundles on a qualified host:

```sh
bazel build //android/tests/fixtures/rust_apk:app_apk_aab_file \
    --extra_toolchains=@android_ndk_toolchain//:ndk_cc_toolchain_device,@android_ndk_toolchain//:ndk_cc_toolchain_emulator
bazel test //android/tests/fixtures/rust_apk:check_app_aab_test \
    //android/tests/fixtures/rust_apk:check_aab_missing_abi_test \
    //android/tests/fixtures/rust_apk:check_aab_missing_resource_test \
    //android/tests/fixtures/rust_apk:inspect_aab_test \
    //android/tests/fixtures/rust_apk:inspect_custom_aab_test \
    --test_env=ANDROID_HOME \
    --extra_toolchains=@android_ndk_toolchain//:ndk_cc_toolchain_device,@android_ndk_toolchain//:ndk_cc_toolchain_emulator
```

The inspect tests read the package, version, SDK floors and library
name from the bundle manifest, and check the asset, the resource table,
the dex and the ELF machine of each `base/lib/<abi>/lib<name>.so`.

### Upload signing

Sign a bundle with your upload key through an explicit launcher:

```python
load("@rules_dx//android/packaging:defs.bzl", "android_aab_release_sign")

android_aab_release_sign(name = "sign_aab", aab = ":app_apk_aab_file")
```

Pass the key through the environment of the run:

```sh
export DX_SIGN_KEYSTORE=upload.jks DX_SIGN_STORE_PASS_FILE=store.pass
export DX_SIGN_KEY_ALIAS=upload DX_SIGN_KEY_PASS_FILE=key.pass DX_SIGN_OUT=app-release.aab
bazel run //path:sign_aab --java_runtime_version=remotejdk_21
```

The variables match APK signing, except that `DX_SIGN_KEY_ALIAS` is
required. `DX_SIGN_KEY_PASS_FILE` is optional. Passwords are read from
the named files only, and relative paths resolve against the directory
you ran from.

App Bundles carry a JAR signature, so the launcher uses `jarsigner`
and `keytool` from the selected JDK, not `apksigner`. It rejects a
bundle that is already signed, runs `bundletool validate`, signs with
`SHA256withRSA` or `SHA256withECDSA` and a SHA-256 digest, verifies the
result with `jarsigner -verify -strict`, and only then writes
`DX_SIGN_OUT`. Upload keys are self-signed, so the self-signed warning
passes; an expired or not yet valid certificate fails. The upload key
must be RSA or EC.

Missing inputs or variables exit 2. A missing bundletool or
`jarsigner` exits 127. A wrong password, wrong alias, already-signed or
invalid bundle exits 1 without writing a bundle.

Run `bazel test //android/tests/fixtures/rust_apk:sign_aab_test` to
sign a generated test bundle with a disposable key and verify it.

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
App Bundles come from the upstream `android_application` rule.

APK assembly and device install/launch need a qualified SDK host (see
above). Selection, negative, and adapter coverage runs everywhere:

```sh
bazel test //android/...
```

## Example

See [`examples/adopt-android`](../examples/adopt-android/README.md) for a
consumer that adopts these rules end to end.
