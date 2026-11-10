"""fortitude standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_arm64": {
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
        "sha256": "6d3fc6d653746c6fc0249f7fb64babafb19a8b69e41e17bdbdf6354008441574",
        "size": 4660830,
        "tool": "fortitude",
        "upstream_version": "0.10.0",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-unknown-linux-musl.tar.gz",
    },
    "linux_x86_64": {
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
        "sha256": "97eb0ed8a7d5fe5424483ec4c0e65ee277477300abc6ba2d8564de9d6053e4fa",
        "size": 5084903,
        "tool": "fortitude",
        "upstream_version": "0.10.0",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-x86_64-unknown-linux-musl.tar.gz",
    },
    "macos_arm64": {
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
        "sha256": "d0972dcf32e542fd064893d2f258345cd1e0f6f73e7928b52cd04fb94929ded7",
        "size": 4548822,
        "tool": "fortitude",
        "upstream_version": "0.10.0",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-apple-darwin.tar.gz",
    },
    "windows_arm64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "zip",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o0",
                    "name": "fortitude.exe",
                    "size": 17522688,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "fortitude.exe",
        "executable_sha256": "d057a16669d88de0b8f9bb5f116004b09524974dd51d0f362be1306e75f3061f",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.0/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "61aca874ac0ea5c5d0485a9d7968bcc77ba30f0b8db98a0d9b02672d781b1a82",
        "size": 4587749,
        "tool": "fortitude",
        "upstream_version": "0.10.0",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-aarch64-pc-windows-msvc.zip",
    },
    "windows_x86_64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "zip",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o0",
                    "name": "fortitude.exe",
                    "size": 19124224,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "fortitude.exe",
        "executable_sha256": "982ad425be3d2a14be1cef50935cb09fdfd6037f48d05e525dc45d3411584c46",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.0/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "f2146fa8331c4accfb98f6844c80aa1cd74faf0c9effc977341a3546e0f702e2",
        "size": 4907243,
        "tool": "fortitude",
        "upstream_version": "0.10.0",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.0/fortitude-x86_64-pc-windows-msvc.zip",
    },
}
