"""Pinned cosign release and its per-platform checksums."""

COSIGN_VERSION = "v2.6.5"
COSIGN_SHA256_LINUX_AMD64 = "c3b4f5410e608af03a5eb0aaac84a4313d8da131248e08ff1759ac70c79d1644"
COSIGN_SHA256_LINUX_ARM64 = "426193b4c5da4d4d643e822f48fe0cc8a476ca1782a272704831f5a0cef716d7"
COSIGN_SHA256_DARWIN_ARM64 = "4d41cc18f0563907c0c785b51db76e1d1af10db4422b605ba876b1758e1771ab"
COSIGN_SHA256_WINDOWS_AMD64 = "19ec346d0050f59ffad9694da97ef9704be34ec60ddc41c5f1d16d8619385825"

COSIGN_PLATFORMS = {
    "linux_x86_64": {
        "asset": "cosign-linux-amd64",
        "cpu": "x86_64",
        "exe": "cosign",
        "os": "linux",
        "sha256": COSIGN_SHA256_LINUX_AMD64,
    },
    "linux_arm64": {
        "asset": "cosign-linux-arm64",
        "cpu": "aarch64",
        "exe": "cosign",
        "os": "linux",
        "sha256": COSIGN_SHA256_LINUX_ARM64,
    },
    "macos_arm64": {
        "asset": "cosign-darwin-arm64",
        "cpu": "aarch64",
        "exe": "cosign",
        "os": "macos",
        "sha256": COSIGN_SHA256_DARWIN_ARM64,
    },
    "windows_x86_64": {
        "asset": "cosign-windows-amd64.exe",
        "cpu": "x86_64",
        "exe": "cosign.exe",
        "os": "windows",
        "sha256": COSIGN_SHA256_WINDOWS_AMD64,
    },
}
