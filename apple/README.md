# Apple

Declare one Xcode selection with its execution route. Loading these rules
changes no toolchain.

```python
load("@rules_dx//apple/provisioning:defs.bzl", "apple_provisioning")

apple_provisioning(
    name = "xcode",
    accept_licenses = ["apple-xcode-license"],
    host = "macos_arm64",
    route = "local",
    sdk = "iphoneos",
    xcode_version = "16.2",
)
```

The qualified cell is Xcode 16 on a macOS 14.5 arm64 executor building
against `iphoneos`, `iphonesimulator`, or `macosx`. Every field stays
explicit: unknown routes, hosts, and SDKs fail instead of falling back.

Use `route = "managed"` with `endpoint` and `credential_file` for a
preprovisioned macOS executor. Credentials travel by file only; inline
tokens fail. Xcode has no unattended download, so automatic acquisition
fails with that diagnostic.

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

Simulator and device execution stay pending: run on a qualified macOS
executor with Xcode. No launch is qualified from any other host.

## Simulator apps

Compose one simulator application over a qualified library cell.
Loading these rules builds and launches nothing.

```python
load("@rules_dx//apple/ios:app.bzl", "apple_ios_app")

apple_ios_app(
    name = "share_app",
    environment = "simulator",
    arch = "arm64",
    bundle_id = "com.example.share",
    version = "1.0",
    deployment = "17.0",
    xcode_version = "16.2",
    library = "share",
    resources = ["Assets.xcassets"],
)
```

The cell fields keep the library contract: simulator and device cells,
triples, SDKs, deployment floor and ceiling, and Xcode lineage fail the
same way. The bundle identifier stays reverse-DNS and the version stays
dotted. The simulator runs unsigned, so signing inputs fail there;
device distribution needs a provisioning profile file and a signing
identity. Exporting an IPA is a distinct explicitly applied operation:
`export_ipa` fails under Check. Launch stays pending for a qualified
macOS executor.

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
