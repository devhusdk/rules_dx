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

Unknown routes, hosts, and SDKs fail instead of falling back.

Use `route = "managed"` with `endpoint` and `credential_file` for a
preprovisioned macOS remote executor. Point `--xcode_version_config` at
an `xcode_config` naming the executor's Xcode and register a
`macos_arm64` execution platform. The managed probe never runs locally: a build with no remote executor
fails instead of probing the developer's Mac. Credentials travel by file only;
inline tokens fail. Automatic Xcode acquisition fails.

## iOS libraries

Build one Rust static library for one device or simulator cell.

```python
load("@rules_dx//apple/ios:defs.bzl", "apple_ios_lib")

apple_ios_lib(
    name = "share",
    srcs = ["share.rs"],
    hdrs = ["share_api.h"],
    environment = "simulator",
    arch = "arm64",
    deployment = "17.0",
    xcode_version = "16.4",
    frameworks = ["Foundation"],
    crate_features = ["share"],
    rustc_flags = ["--cfg", "dx_ios"],
)
```

`srcs`, `deps`, `proc_macro_deps`, `crate_root`, `crate_name`, and
`edition` pass to `rust_static_library`. Building `:share` on a macOS
host with Xcode writes the static archive and `<name>_cell.txt`, which
records the cell, SDK, deployment, Xcode, and the archive's Mach-O
platform and minimum OS. Consumers get the archive, `hdrs`, and one
`-Wl,-framework,<name>` link flag per framework through `CcInfo`.

| Cell | Triple | SDK |
| --- | --- | --- |
| `device/arm64` | `aarch64-apple-ios` | `iphoneos` |
| `simulator/arm64` | `aarch64-apple-ios-sim` | `iphonesimulator` |
| `simulator/x86_64` | `x86_64-apple-ios` | `iphonesimulator` |

Add the triples and the Xcode C++ toolchain to `MODULE.bazel`:

```python
apple_cc_configure = use_extension("@xcode_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    extra_target_triples = [
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "x86_64-apple-ios",
    ],
)
```

Outside an iOS build the library builds for its own cell at `deployment`
(default 17.0, the SDK floor). Inside an app or framework it builds in
that cell. Linux hosts skip it. The build fails when:

- a device triple, SDK, or arch is set on a simulator cell, or the reverse;
- `deployment` is below 17.0 or above the Xcode 16 ceiling (18.0), or
  above the iOS version of the build;
- Xcode is outside lineage 16, or another Xcode resolved;
- the library links no static archive;
- the archive carries another arch or platform, or needs a newer iOS
  than the deployment;
- the app or framework builds another cell than the library declares.

One `apple_ios_lib` serves one cell, so pass a single-slice
`apple_ios_framework` or an app in that cell.

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
such as `rust_static_library`. `frameworks` takes XCFramework imports;
see [Consume an XCFramework](#consume-an-xcframework). The app needs
`library`, `frameworks`, or both. Without `infoplists`, the app gets a
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
then deletes the simulator. A simulator left by an interrupted or
timed-out run is deleted by the next test run on that host. The runtime must be installed. A missing
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
- `srcs`, `deployment`, `bundle_id`, or `version` is missing or
  malformed, or both `library` and `frameworks` are empty.

Simulator builds sign ad hoc; `provisioning_profile` fails there. On
non-Apple hosts the app targets are incompatible and skip.

### Device apps

Set `environment = "device"` and `provisioning_profile` to a
`.mobileprovision` file:

```python
apple_ios_app(
    name = "ledger_device",
    srcs = ["main.m"],
    bundle_id = "com.example.ledger",
    deployment = "17.0",
    environment = "device",
    library = ":ledger_rust",
    provisioning_profile = "ledger_dev.mobileprovision",
    version = "1.0",
)
```

```sh
bazel build //:ledger_device --ios_multi_cpus=arm64
```

Device builds always sign ad hoc and embed the profile. They never read
a keychain, even when `signing_certificate_name` is set. The build fails
when the profile has expired, does not cover `bundle_id`, or does not
allow an entitlement. Rust needs the `aarch64-apple-ios` triple.

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
- `127` only when `xcrun` is missing;
- `simctl`'s own code when it fails.

### Export a signed IPA

Export is an explicit `bazel run` launcher over one device app:

```python
load("@rules_dx//apple/ios:device.bzl", "apple_ios_export")

apple_ios_export(name = "export", app = ":ledger_device")
```

```sh
bazel run //:export --ios_multi_cpus=arm64 -- \
    --certificate dev.p12 --password-file dev.p12.pass --out dist/Ledger.ipa
```

Flags:

- `--certificate`: the signing identity `.p12` file.
- `--password-file`: the file holding the `.p12` password.
- `--out`: the `.ipa` to write, outside `bazel-out`.

Relative paths resolve against the directory you ran `bazel` from.
`apple_ios_export` takes only a device `apple_ios_app`; other targets
fail analysis.

The launcher checks that:

- the IPA embeds the declared profile;
- the profile has not expired and covers the bundle id;
- the app's entitlements are in the profile;
- the certificate is in the profile's `DeveloperCertificates`.

It signs `Frameworks/*`, then the app, and verifies the result with
`codesign --verify --deep --strict`, the team, `arm64` only, the
`iphoneos` platform, and an unchanged file list. Apps with extensions
fail.

It writes `<out>` and `<out>.txt`, which records the bundle id, team,
profile name, UUID and expiry, certificate SHA-1, signing authority,
entitlement keys, archs, platform, and minimum OS.

The certificate and password are never Bazel inputs. The identity goes
into a temporary keychain that is deleted on exit, and the keychain
search list is restored. Override tools with `XCRUN`, `SECURITY`,
`CODESIGN`, `OPENSSL`, `PLUTIL`, `DX_UNZIP`, `DITTO`, or `DX_ZIP`.

The launcher exits with:

- `0` when the IPA and report are written;
- `1` when the signed app fails verification;
- `2` when an input is missing or bad, or the profile or certificate is
  rejected;
- `127` when a tool is missing;
- `codesign`'s own code when signing fails.

## XCFrameworks

Build one XCFramework over a Rust static library and public headers. The
bundle is an upstream `apple_xcframework`; each slice is built for its own
platform, never merged by architecture name.

```python
load("@rules_dx//apple/ios:app.bzl", "apple_ios_framework")
load("@rules_rust//rust:defs.bzl", "rust_static_library")

rust_static_library(
    name = "share_rust",
    srcs = ["share.rs"],
)

apple_ios_framework(
    name = "share",
    bundle_id = "com.example.share",
    bundle_name = "Share",
    version = "1.4",
    deployment = "17.0",
    xcode_version = "16.4",
    hdrs = ["share_api.h"],
    library = ":share_rust",
    resources = ["rates.txt"],
    slices = [
        "device/arm64",
        "simulator/arm64",
        "simulator/x86_64",
    ],
)
```

Add every slice triple to the Rust toolchain:

```python
rust.toolchain(
    edition = "2021",
    extra_target_triples = [
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "x86_64-apple-ios",
    ],
)
```

Build on a macOS host with Xcode:

```sh
bazel build //:share
```

The build writes `bazel-bin/<package>/<name>.xcframework.zip` with
`<bundle_name>.xcframework` inside, and
`bazel-bin/<package>/<name>_xcframework.txt`, which records each slice's
library identifier, platform, minimum OS, and install name. The archive
appears only after every check passes.

Each slice is a cell: `device/arm64`, `simulator/arm64`, or
`simulator/x86_64`. The framework needs `device/arm64` and at least one
simulator slice.

The build fails when:

- a slice is unknown, malformed, or listed twice, or `device/arm64` or
  every simulator slice is missing;
- `hdrs`, `deployment`, `bundle_id`, `version`, or `library` is missing
  or malformed, or a resource path is empty or absolute;
- a public header has the umbrella header's name in any case, such as
  `share.h` next to `Share.h`;
- the library links no static archive, or an archive misses the slice's
  arch, is built for another platform, or needs a newer iOS than
  `deployment`;
- the XCFramework misses a declared slice, carries an undeclared one, or
  lists one twice;
- a slice binary is built for another platform, needs a newer iOS than
  `deployment`, or has the wrong install name;
- a slice misses its module map, a public header, a resource, or the
  declared bundle identifier, version, or minimum OS;
- no Xcode resolves, or Bazel resolved a different Xcode than
  `xcode_version`.

### Consume an XCFramework

Import the unpacked XCFramework with upstream
`apple_dynamic_xcframework_import` and pass it in `frameworks`:

```python
load("@rules_apple//apple:apple.bzl", "apple_dynamic_xcframework_import")
load("@rules_dx//apple/ios:app.bzl", "apple_ios_app")

apple_dynamic_xcframework_import(
    name = "share",
    xcframework_imports = glob(["Share.xcframework/**"]),
)

apple_ios_app(
    name = "till_app",
    srcs = ["main.m"],
    environment = "simulator",
    arch = "arm64",
    bundle_id = "com.example.till",
    version = "2.0",
    deployment = "17.0",
    frameworks = [":share"],
)
```

Your `MODULE.bazel` needs `bazel_dep(name = "rules_apple", version =
"5.2.0")`. Sources import `<Share/share_api.h>`. The app embeds the
framework under `Frameworks/`, and its resources load from
`[NSBundle bundleWithIdentifier:]`.

`bazel-bin/<package>/<name>_library.txt` records each framework binary's
arch, platform, and minimum OS. The app build fails when the imported
XCFramework has no slice for the cell, or the slice binary misses the
arch, is built for another platform, or needs a newer iOS than the app
`deployment`.

## macOS signing and notarization

Signing and notarization are explicit `bazel run` launchers over one
macOS application. The application target must provide rules_apple's
`AppleBundleInfo`, as `macos_application` does:

```python
load("@rules_dx//apple/signing:defs.bzl", "apple_macos_notarize", "apple_macos_sign")

apple_macos_sign(name = "sign", app = ":ledger")

apple_macos_notarize(name = "notarize", app = ":ledger")
```

```sh
bazel run //:sign -- \
    --certificate developer_id.p12 --password-file developer_id.p12.pass \
    --out dist/Ledger.app
bazel run //:notarize -- --app dist/Ledger.app \
    --key AuthKey_2X9R4HXF34.p8 --key-id 2X9R4HXF34 --issuer <issuer-uuid>
```

`bazel build` and `dx check` only write a small metadata file for these
targets. They never sign anything or contact Apple. Both launchers need a
macOS host with Xcode. Relative paths resolve against the directory you
ran `bazel` from. Other targets than a macOS `.app` fail analysis.

### Sign

Flags:

- `--certificate`: the Developer ID Application identity `.p12` file.
- `--password-file`: the file holding the `.p12` password.
- `--identity`: instead of the two flags above, the SHA-1 or name of a
  code signing identity that is already in a keychain. A name must match
  exactly one identity.
- `--keychain`: with `--identity`, the keychain to search. It must be
  unlocked. Without it, the user keychain search list is used.
- `--out`: the `.app` to write, outside `bazel-out`.

When no credential flag is given, the launcher reads the identity from
`DX_APPLE_SIGN_IDENTITY` and the keychain from `DX_APPLE_KEYCHAIN`.

The launcher unpacks the built app and signs it inside out. In each
bundle it signs, in order:

1. each entry in `Contents/Frameworks`;
2. each entry in `Contents/PlugIns`, `Contents/XPCServices`,
   `Contents/Helpers`, and `Contents/Library/LoginItems`,
   `SystemExtensions`, `QuickLook` and `Spotlight`, signing the nested
   code of each nested bundle first;
3. any other Mach-O file in `Contents/MacOS`;
4. the bundle itself.

Nested code keeps its own entitlements through
`--preserve-metadata=entitlements`. The app itself is signed with the
target's entitlements. Every call uses `--force --options runtime
--timestamp`, so the ad hoc build signature is replaced, the hardened
runtime is on, and Apple's secure timestamp is attached. It never signs
with `--deep`.

Input that fails before signing:

- entitlements on the app or on nested code that set
  `com.apple.security.get-task-allow`;
- a nested bundle without `Contents/Info.plist`;
- code nested inside a framework;
- a Mach-O file anywhere else, such as `Contents/Resources`, since it
  would keep its ad hoc signature and fail notarization.

After signing, the launcher checks `codesign --verify --deep --strict`
and that `codesign -dvv` shows the runtime flag, a timestamp, and a
signing authority. It then writes `<out>` and `<out>.txt`, which records
the bundle id, team, signing authority, certificate SHA-1, timestamp,
entitlement keys, archs, and the runtime flag.

The certificate and password are never Bazel inputs. A `.p12` identity
goes into a temporary keychain that is deleted on exit, and the keychain
search list is restored. An `--identity` is used where it is and no
keychain is changed.

### Notarize

Flags:

- `--app`: the signed `.app` to notarize and staple.
- `--keychain-profile`: a profile stored with
  `xcrun notarytool store-credentials`.
- `--key`, `--key-id`, `--issuer`: an App Store Connect API key file,
  its key id, and the issuer id. Use these three or `--keychain-profile`,
  not both.
- `--timeout`: how long `notarytool` waits, `2h` by default.

The launcher checks that the app's bundle id is the target's bundle id
and that the app carries a hardened runtime signature that is not ad hoc.
It zips the app with `ditto`, submits it with `xcrun notarytool submit
--wait`, and reads the submission status from the JSON output. It always
fetches the notary log to `<app>.notary.json`. Only an `Accepted`
submission is stapled; the launcher then runs `xcrun stapler validate`
and `spctl --assess --type execute`.

Only file paths of secrets are passed to the tools. The launchers never
print the password or key contents.

Override tools with `XCRUN`, `SECURITY`, `CODESIGN`, `OPENSSL`, `PLUTIL`,
`DITTO`, or `SPCTL`.

Both launchers exit with:

- `0` when the signed app and report are written, or the app is
  notarized and stapled;
- `1` when the signed app fails verification, notarization returns
  `Invalid` or `Rejected`, or stapling or assessment fails;
- `2` when an input or credential is missing or bad;
- `3` when the Apple timestamp or notary service is unreachable, or the
  submission is still `In Progress` after `--timeout`;
- `127` when a tool is missing;
- `codesign`'s or `notarytool`'s own code for other failures.
