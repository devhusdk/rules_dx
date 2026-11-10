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
                    "size": 16865488,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "fortitude-aarch64-unknown-linux-musl/fortitude",
        "executable_sha256": "01b7ec2b40173b301b133f1beb890ccc0f658c55c0be302a40b1f6e681c72270",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.1/LICENSE",
            },
        ],
        "linkage": "static",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "5f8082794b5ed2927b73f260992ed1e5eac5e24cd5c0d473e262a1ac0cac0339",
        "size": 4675997,
        "tool": "fortitude",
        "upstream_version": "0.10.1",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.1/fortitude-aarch64-unknown-linux-musl.tar.gz",
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
                    "size": 18734136,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "fortitude-x86_64-unknown-linux-musl/fortitude",
        "executable_sha256": "144fa9bb65a40e35cf08e5ea13bfb8d04d0a2ebb7fcb5e127cbc452aed664790",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.1/LICENSE",
            },
        ],
        "linkage": "static-pie",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "bf54fbf1daac0f08e0ae61339d70bd4a244e3ab0386d96184155048c858fb816",
        "size": 5104929,
        "tool": "fortitude",
        "upstream_version": "0.10.1",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.1/fortitude-x86_64-unknown-linux-musl.tar.gz",
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
                    "size": 17159424,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "fortitude-aarch64-apple-darwin/fortitude",
        "executable_sha256": "7551ec0d6ffa916da5039f70a6e9250ec171bf8c1d2e6fb7b6e21fbb43427808",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.1/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "ae1b3f2da7fac7acaa0a3b27c8bab3547034f42f1fccc751d5dfac8ef0b5f232",
        "size": 4599756,
        "tool": "fortitude",
        "upstream_version": "0.10.1",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.1/fortitude-aarch64-apple-darwin.tar.gz",
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
                    "size": 17671168,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "fortitude.exe",
        "executable_sha256": "4f5902727c270734c081889410a391a82024c86bf252167b950cf3a5589ead99",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.1/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "30ae9ba2b51061b361fb053e8a62a25ab938861222ef285992c114f124cfb59a",
        "size": 4615649,
        "tool": "fortitude",
        "upstream_version": "0.10.1",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.1/fortitude-aarch64-pc-windows-msvc.zip",
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
                    "size": 19280896,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "fortitude.exe",
        "executable_sha256": "feb9750722d1963c6ad29ac2cba85d74981a22ee246f7293d5f1276d60a6cd30",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/PlasmaFAIR/fortitude/blob/v0.10.1/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "b7d1dcb5a02946f6e38a60b7f7b60dfd3a8f6b82441d6c5afeaa3fbdad56a0c9",
        "size": 4936753,
        "tool": "fortitude",
        "upstream_version": "0.10.1",
        "url": "https://github.com/PlasmaFAIR/fortitude/releases/download/v0.10.1/fortitude-x86_64-pc-windows-msvc.zip",
    },
}
