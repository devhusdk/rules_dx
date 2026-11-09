"""Pinned opam release and its per-platform checksums."""

OPAM_VERSION = "2.6.1"

OPAM_PLATFORMS = {
    "linux_x86_64": {
        "asset": "x86_64-linux",
        "cpu": "x86_64",
        "exe": "opam",
        "os": "linux",
        "sha256": "b533d393a6967150d6ef41cb3d71e147e93ecbabeb59e921aa412c0ae79ebe3d",
    },
    "linux_arm64": {
        "asset": "arm64-linux",
        "cpu": "aarch64",
        "exe": "opam",
        "os": "linux",
        "sha256": "8b24ef2f3f44a31763f54878454fb52afa1f2db7e00abed58b8d4e7ee7e290a6",
    },
    "macos_x86_64": {
        "asset": "x86_64-macos",
        "cpu": "x86_64",
        "exe": "opam",
        "os": "macos",
        "sha256": "c5bb19c813ae9a4317b5ff2845ba52a51c96648c08faa0ecc14900fb61b198c7",
    },
    "macos_arm64": {
        "asset": "arm64-macos",
        "cpu": "aarch64",
        "exe": "opam",
        "os": "macos",
        "sha256": "3e7f67bda3ba0bf72f597917b17dc337ed5a1bd48224df8b747bb35ce05c569e",
    },
    "windows_x86_64": {
        "asset": "x86_64-windows.exe",
        "cpu": "x86_64",
        "exe": "opam.exe",
        "os": "windows",
        "sha256": "2548f0592abfa728432c85ef6d08bf97c295c1859ddca61876e33884336c130c",
    },
}
