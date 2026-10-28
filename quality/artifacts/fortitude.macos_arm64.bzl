"""fortitude standalone artifact metadata (macos_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": None,
        "libc": None,
        "libstdcxx": None,
    },
    "archive": {
        "format": "tar.gz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "fortitude-aarch64-apple-darwin",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "fortitude-aarch64-apple-darwin/fortitude",
                "size": 16958960,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "fortitude-aarch64-apple-darwin/fortitude",
    "executable_sha256": "f24b58b7896d4b580b9d2ec4021c60f8b5882c36176e971b451771714d9ef01a",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "d0972dcf32e542fd064893d2f258345cd1e0f6f73e7928b52cd04fb94929ded7",
    "size": 4548822,
    "tool": "fortitude",
    "upstream_version": "0.10.0",
    "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-apple-darwin.tar.gz",
}
