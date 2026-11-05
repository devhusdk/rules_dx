"""Pinned cargo-semver-checks release and its per-platform checksums."""

SEMVER_CHECKS_VERSION = "v0.51.0"

SEMVER_CHECKS_PLATFORMS = {
    "linux_x86_64": {
        "cpu": "x86_64",
        "exe": "cargo-semver-checks",
        "os": "linux",
        "sha256": "396c8508d973c83de52660943a057e971b586f7511c3ba695762373452b43d5b",
        "target": "x86_64-unknown-linux-musl",
    },
    "linux_arm64": {
        "cpu": "aarch64",
        "exe": "cargo-semver-checks",
        "os": "linux",
        "sha256": "a7355f6d36edab9f76ea3fe565b9f5727394ca64f6ad751b8be24ac65a902136",
        "target": "aarch64-unknown-linux-musl",
    },
    "macos_arm64": {
        "cpu": "aarch64",
        "exe": "cargo-semver-checks",
        "os": "macos",
        "sha256": "3cec5da41eee9a3336d75ad43d1f30a5c9680818f4ba209024786f452cabab5b",
        "target": "aarch64-apple-darwin",
    },
    "windows_x86_64": {
        "cpu": "x86_64",
        "exe": "cargo-semver-checks.exe",
        "os": "windows",
        "sha256": "9bd49e09c7487c323a9f4479c5d2b0b50d29a45662ba28344b7a0f5423eb0444",
        "target": "x86_64-pc-windows-msvc",
    },
}
