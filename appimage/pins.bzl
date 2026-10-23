"""Pinned AppImage tool and runtime identities."""

TOOL_VERSION = "1.9.1"

TOOLS = {
    "aarch64": {
        "sha256": "f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158",
        "url": "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-aarch64.AppImage",
    },
    "x86_64": {
        "sha256": "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0",
        "url": "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage",
    },
}

TOOL_LICENSE = "MIT"

RUNTIME_VERSION = "20251108"

RUNTIMES = {
    "aarch64": {
        "sha256": "00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444",
        "url": "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-aarch64",
    },
    "x86_64": {
        "sha256": "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d",
        "url": "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64",
    },
}

RUNTIME_LICENSE = "MIT"

MKSQUASHFS_VERSION = "4.6.1"

MKSQUASHFS_LICENSE = "GPL-2.0-or-later"

TOOL_BINARIES = ["appimagetool", "desktop-file-validate", "mksquashfs"]

ARCHES = ["aarch64", "x86_64"]

SUPPORTED_CELLS = ["linux_arm64", "linux_x86_64"]
