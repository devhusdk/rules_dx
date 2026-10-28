"""fortitude standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

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
                "name": "fortitude-x86_64-unknown-linux-musl",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "fortitude-x86_64-unknown-linux-musl/fortitude",
                "size": 18619416,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "fortitude-x86_64-unknown-linux-musl/fortitude",
    "executable_sha256": "6417e5da56f63e6150a872efc87f19c572a91d5f5fc3a98b2f16a8334945af47",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.0/LICENSE",
        },
    ],
    "linkage": "static-pie",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "97eb0ed8a7d5fe5424483ec4c0e65ee277477300abc6ba2d8564de9d6053e4fa",
    "size": 5084903,
    "tool": "fortitude",
    "upstream_version": "0.10.0",
    "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-x86_64-unknown-linux-musl.tar.gz",
}
