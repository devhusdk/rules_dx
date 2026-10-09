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
