"""fortitude standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

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
                "name": "fortitude-aarch64-unknown-linux-musl",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "fortitude-aarch64-unknown-linux-musl/fortitude",
                "size": 16799904,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "fortitude-aarch64-unknown-linux-musl/fortitude",
    "executable_sha256": "377c86ee08c5bf278a44013730adba35b135135e8fa2e057d0c5931604fefbc5",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "6d3fc6d653746c6fc0249f7fb64babafb19a8b69e41e17bdbdf6354008441574",
    "size": 4660830,
    "tool": "fortitude",
    "upstream_version": "0.10.0",
    "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-unknown-linux-musl.tar.gz",
}
