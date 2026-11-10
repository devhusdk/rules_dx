"""Pinned conan release and its per-platform checksums."""

CONAN_VERSION = "2.34.0"

CONAN_PLATFORMS = {
    "linux_x86_64": {
        "asset": "linux-x86_64.tgz",
        "cpu": "x86_64",
        "exe": "bin/conan",
        "os": "linux",
        "sha256": "655177bf39c901ea6b532e02f55813eb03abaced931b5627307f1c8edc3627b6",
    },
    "linux_arm64": {
        "asset": "linux-aarch64.tgz",
        "cpu": "aarch64",
        "exe": "bin/conan",
        "os": "linux",
        "sha256": "1779d4b5fc62697a03bfb24b9ea502faf28cfb1a2801e243c192934446caf255",
    },
    "macos_x86_64": {
        "asset": "macos-x86_64.tgz",
        "cpu": "x86_64",
        "exe": "bin/conan",
        "os": "macos",
        "sha256": "9884e36f3aff37d69932a8e5b1dfc19737c684862049604eb419ec71203f2abb",
    },
    "macos_arm64": {
        "asset": "macos-arm64.tgz",
        "cpu": "aarch64",
        "exe": "bin/conan",
        "os": "macos",
        "sha256": "09f06ed0d4a0debb37020698741fdcd6917fe295fe1c6a14878809b23c00fe6c",
    },
    "windows_x86_64": {
        "asset": "windows-x86_64.zip",
        "cpu": "x86_64",
        "exe": "conan.exe",
        "os": "windows",
        "sha256": "4e8c75dfb5f9913fe3eec6c0c982986f13041eae6e8c3b116268489b21951232",
    },
}
