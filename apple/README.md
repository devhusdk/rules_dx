# Apple

Declare one Xcode selection with its execution route, then probe it.
Loading these rules changes no toolchain.

```python
load("@rules_dx//apple/provisioning:defs.bzl", "apple_provisioning")
load("@rules_dx//apple/provisioning:probe.bzl", "apple_toolchain_probe")

apple_provisioning(
    name = "simulator",
    accept_licenses = ["apple-xcode-license"],
    deployment = "17.0",
    host = "macos_arm64",
    route = "local",
    sdk = "iphonesimulator",
    xcode_version = "16.4",
)

apple_toolchain_probe(
    name = "simulator_probe",
    provisioning = ":simulator",
    target_compatible_with = ["@platforms//os:ios"],
)
```

Select the cell with upstream `apple_support` flags:

```sh
bazel build //:simulator_probe \
  --platforms=@apple_support//platforms:ios_sim_arm64 \
  --ios_minimum_os=17.0 \
  --xcode_version=16.4
```

Use `macos_arm64` with `--macos_minimum_os` for `macosx`. For the
`macosx` cell, register the Xcode C/C++ toolchain ahead of other C/C++
toolchains in your `MODULE.bazel`:

```python
apple_cc_configure = use_extension("@apple_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")
```

The probe writes `bazel-bin/<package>/<name>.txt` with the executor's real
identities: route, endpoint, host, arch, macOS version and build, Xcode
version and build, `DEVELOPER_DIR`, SDK version, build, and path,
deployment target, `clang` path, compiler identity, and license state.
It never reads `credential_file`.

The probe fails instead of substituting anything when:

- no Xcode resolves for the build, as on a Linux host;
- the declared Xcode differs from the one Bazel resolved;
- the build targets another SDK or no Apple platform;
- the deployment target differs from the declared one or exceeds the SDK;
- no C/C++ toolchain targets the SDK from the executor;
- the compiler is not Xcode's Apple clang;
- the action runs on a non-macOS host, the wrong CPU, or macOS below 14.5;
- the Xcode license is unaccepted on the executor.

The qualified cell is Xcode 16 on a macOS 14.5 arm64 executor building
against `iphoneos`, `iphonesimulator`, or `macosx`. Every field stays
explicit: unknown routes, hosts, and SDKs fail instead of falling back.
The local route is verified on GitHub-hosted `macos-15` runners with
Xcode preinstalled.

Use `route = "managed"` with `endpoint` and `credential_file` for a
preprovisioned macOS remote executor. Point `--xcode_version_config` at
an `xcode_config` naming the executor's Xcode and register a
`macos_arm64` execution platform. The managed probe never runs locally: a build with no remote executor
fails instead of probing the developer's Mac. Credentials travel by file only;
inline tokens fail. Xcode has no unattended download, so automatic
acquisition fails with that diagnostic. Accepting the Xcode license is
the executor administrator's step.

Build, launch, signing, and publication stay separate operations with
their own prerequisites.

## iOS libraries

Declare one Rust library selection per device or simulator cell. Loading
these rules builds and launches nothing.

```python
load("@rules_dx//apple/ios:defs.bzl", "apple_ios_lib")

apple_ios_lib(
    name = "share",
    environment = "simulator",
    arch = "arm64",
    deployment = "17.0",
    xcode_version = "16.2",
    frameworks = ["Foundation"],
    crate_features = ["share"],
    rustc_flags = ["--cfg", "dx_ios"],
)
```

The device cell is `arm64` with triple `aarch64-apple-ios` against
`iphoneos`. The simulator cell is `arm64` or `x86_64` with the matching
`-sim` triple against `iphonesimulator`. Device triples, SDKs, and
architectures fail on the simulator cell and vice versa instead of
falling back. Deployment targets stay between the SDK floor (17.0) and
the Xcode 16 ceiling (18.0); Xcode stays on lineage 16. Features, flags,
and frameworks pass through unchanged for the consuming build.

To run Rust code on a simulator, link it into an app with
`apple_ios_app`. Device execution stays pending.

## Simulator apps

Build one iOS application bundle over a Rust static library. The app
target is an upstream `ios_application`; building it never boots,
installs, or launches a simulator.

```python
load("@rules_dx//apple/ios:app.bzl", "apple_ios_app", "apple_ios_app_test")
load("@rules_rust//rust:defs.bzl", "rust_static_library")

rust_static_library(
    name = "ledger_rust",
    srcs = ["ledger.rs"],
)

apple_ios_app(
    name = "ledger_app",
    srcs = ["main.m", "LedgerAppDelegate.m"],
    hdrs = ["LedgerAppDelegate.h"],
    environment = "simulator",
    arch = "arm64",
    bundle_id = "com.example.ledger",
    version = "1.0",
    deployment = "17.0",
    xcode_version = "16.4",
    library = ":ledger_rust",
    resources = ["entries.txt"],
)

apple_ios_app_test(
    name = "ledger_app_test",
    size = "large",
    srcs = ["LedgerAppTests.m"],
    app = ":ledger_app",
    deployment = "17.0",
    device_type = "iPhone 16",
    os_version = "18.5",
)
```

`srcs` holds the native entry point that calls `UIApplicationMain`.
`library` is any target with `CcInfo` that links one static archive,
such as `rust_static_library`. Without `infoplists`, the app gets a
default `Info.plist`.

Your `MODULE.bazel` needs the Rust simulator triple and the Xcode C/C++
toolchain:

```python
rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    extra_target_triples = ["aarch64-apple-ios-sim"],
)
use_repo(rust, "rust_toolchains")

register_toolchains("@rust_toolchains//:all")

apple_cc_configure = use_extension("@apple_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")
```

Build and test on a macOS host with Xcode:

```sh
bazel build //:ledger_app --ios_multi_cpus=sim_arm64
bazel test //:ledger_app_test --ios_multi_cpus=sim_arm64
```

The build writes `bazel-bin/<package>/<name>.ipa` and
`bazel-bin/<package>/<name>_library.txt`, which records the cell, SDK,
SDK version, deployment target, Xcode, and each archive's arch,
platform, and minimum OS. Linking waits for that check.

`apple_ios_app_test` creates and boots a new simulator of `device_type`
on the exact iOS `os_version` runtime, runs XCTest cases inside the app,
then deletes the simulator. The runtime must be installed. A missing
runtime fails the test and creates no simulator; it never falls back to
another runtime. `os_version` below `deployment`
fails analysis.

The build fails before linking when:

- the library links no static archive or lacks `CcInfo`;
- an archive misses the cell's arch, is built for another platform
  (`iphoneos`, `macosx`), or needs a newer iOS than `deployment`;
- the build targets another cell; pass the `--ios_multi_cpus` value the
  error names;
- no Xcode resolves, as on a Linux host, or Bazel resolved a different
  Xcode than `xcode_version`;
- `srcs`, `deployment`, `bundle_id`, `version`, or `library` is missing
  or malformed.

The simulator signs ad hoc, so `provisioning_profile` fails there;
device builds need one. `export_ipa` fails: export is a separate
operation. On non-Apple hosts the app targets are incompatible and skip.

### Simulator operations

Boot, install, and launch are explicit `bazel run` launchers:

```python
load(
    "@rules_dx//apple/ios:simulator.bzl",
    "apple_simulator_boot",
    "apple_simulator_install",
    "apple_simulator_launch",
)

apple_simulator_boot(name = "boot", simulator = "iPhone 16")

apple_simulator_install(name = "install", app = ":ledger_app", simulator = "iPhone 16")

apple_simulator_launch(name = "launch", bundle_id = "com.example.ledger", simulator = "iPhone 16")
```

```sh
bazel run //:boot
bazel run //:install
bazel run //:launch
```

`simulator` is an available simulator's name or UDID. Each launcher
exits with:

- `0` when `xcrun simctl` succeeds;
- `2` when no available simulator matches, a name matches several, or
  an input is missing;
- `127` when `xcrun` is missing;
- `simctl`'s own code when it fails.

## Embeddable frameworks

Compose one embeddable framework over device and simulator slices.

```python
load("@rules_dx//apple/ios:app.bzl", "apple_ios_framework")

apple_ios_framework(
    name = "share_framework",
    bundle_id = "com.example.share",
    version = "1.0",
    slices = [
        "device/arm64",
        "simulator/arm64",
        "simulator/x86_64",
    ],
    resources = ["Assets.xcassets"],
)
```

Every slice names an `environment/arch` cell from the qualified set.
The framework needs the `device/arm64` slice and at least one simulator
slice; unknown environments, wrong architectures, duplicates, and
missing coverage fail. Resources stay relative workspace paths with
non-empty names. Build, signing, and publication stay separate
operations with their own prerequisites.
