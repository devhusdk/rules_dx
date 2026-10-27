# Adopt Android example

A Gradle-shaped tree with a Rust core, a C++ summary, a Java bridge and an APK.
It has no Gradle files. The Rust, C++ and Java targets build and test on any
host.

| Package | Role |
| --- | --- |
| `native` | `core` Rust static library (word count, Adler-32), `summary` C++ library, `jni_bridge` JNI entry point for Android |
| `app` | `report` Java library, `adopt_apk` APK with its own manifest, Java activity, resource and asset |

```sh
bazel build //examples/adopt-android/...
bazel test //examples/adopt-android/...
```

Three tests pass: `core_test`, `summary_test` and `report_test`.
The APK targets are `manual` and need the Android SDK.

## Android

```sh
export ANDROID_HOME=$HOME/Android/Sdk ANDROID_NDK_REVISION=27.2.12479018
NDK=--extra_toolchains=@android_ndk_toolchain//:ndk_cc_toolchain_device,@android_ndk_toolchain//:ndk_cc_toolchain_emulator
bazel test $NDK --test_env=ANDROID_HOME \
  //examples/adopt-android/app:check_apk_test \
  //examples/adopt-android/app:inspect_apk_test
bazel run $NDK //examples/adopt-android/app:install
bazel run $NDK //examples/adopt-android/app:start
bazel run $NDK //examples/adopt-android/app:log
```

`check_apk_test` checks the unsigned APK. `inspect_apk_test` checks the badging,
the dex classes, the asset, the JNI symbol in `libadopt_apk_bin.so` for both
ABIs and the debug signature.

`log` shows the activity output under the `DxAdoptAndroid` tag:

```text
native=words=3 adler32=...
resource=Adopt Android
asset=hello adopt android
bridge=jni
```

## Consumer setup

Copy `rules_android/rules_android.patch` from the rules_dx patches to
`third_party/rules_dx/rules_android.rules_android.patch`, then add this to
`MODULE.bazel`. The Rust core is built for the APK's ABIs, so add the
Android target triples to the `rust.toolchain` call that `dx init` wrote:

```python
rust.toolchain(
    # Keep the edition, rustfmt_version and versions that dx init wrote.
    extra_target_triples = [
        "aarch64-linux-android",
        "x86_64-linux-android",
    ],
)

single_version_override(
    module_name = "rules_android",
    patch_strip = 1,
    patches = ["//third_party/rules_dx:rules_android.rules_android.patch"],
    version = "0.6.6",
)

android_ndk_toolchain_repo = use_repo_rule("@rules_dx//android/toolchain:repos.bzl", "android_ndk_toolchain_repo")

android_ndk_toolchain_repo(
    name = "android_ndk_toolchain",
    api_level = 33,
    cxx_runtime = "static",
)
```

The app ID, manifest, API level, permission and ABIs belong to your app.
`rust_android_apk` takes Java `srcs` and your `manifest`. The library is named
`<name>_bin`, so `System.loadLibrary("adopt_apk_bin")` loads it. The APK is
signed with the debug key. There is no release key and no AAB.
