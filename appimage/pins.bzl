"""Pinned AppImage tool and runtime identities."""

TOOL_VERSION = "1.9.1"

TOOL_URL = "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"

TOOL_SHA256 = "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"

TOOL_LICENSE = "MIT"

RUNTIME_VERSION = "20251108"

RUNTIME_URL = "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"

RUNTIME_SHA256 = "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"

RUNTIME_LICENSE = "MIT"

MKSQUASHFS_VERSION = "4.6.1"

MKSQUASHFS_LICENSE = "GPL-2.0-or-later"

TOOL_BINARIES = ["appimagetool", "desktop-file-validate", "mksquashfs"]

SUPPORTED_CELLS = ["linux_x86_64"]

UNSUPPORTED_CELLS = {
    "linux_arm64": "no arm64 runtime and tool are pinned or measured on native arm64",
}
