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
