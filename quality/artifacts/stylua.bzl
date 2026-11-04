"""stylua standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_arm64": {
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
                    "mode": "0o755",
                    "name": "stylua",
                    "size": 9698464,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "stylua",
        "executable_sha256": "46294bc22bb303e82314fa7247530acc909bb8068b661edfa88b578423872f8d",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/JohnnyMorganz/StyLua/blob/v2.6.0/LICENSE.md",
            },
        ],
        "linkage": "static",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "278495597e3827417f3af7b9ad38011ad4a5ec725503553160ab69b7301059bb",
        "size": 3350659,
        "tool": "stylua",
        "upstream_version": "2.6.0",
        "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-linux-aarch64-musl.zip",
    },
    "linux_x86_64": {
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
                    "mode": "0o755",
                    "name": "stylua",
                    "size": 10252248,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "stylua",
        "executable_sha256": "443b2e59a055f8c8d31f215e912f7099c14746e3f6ca1b6c329ac7ad29485587",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/JohnnyMorganz/StyLua/blob/v2.6.0/LICENSE.md",
            },
        ],
        "linkage": "static-pie",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "48ee7db28346f96f4bd545e190ab080a88a7b5a664e97649cf5d5c60426f402d",
        "size": 3480092,
        "tool": "stylua",
        "upstream_version": "2.6.0",
        "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-linux-x86_64-musl.zip",
    },
    "macos_arm64": {
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
                    "mode": "0o755",
                    "name": "stylua",
                    "size": 8770624,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "stylua",
        "executable_sha256": "c36d9bd2299c990d4fe9420a372a76b761f4b8db054dda248f778f59ddef4149",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/JohnnyMorganz/StyLua/blob/v2.6.0/LICENSE.md",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "dab9b66d340ca5ffc8225211f1dcf89b84783e09b8cbec8c518497eba81f3c42",
        "size": 3038001,
        "tool": "stylua",
        "upstream_version": "2.6.0",
        "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-macos-aarch64.zip",
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
                    "name": "stylua.exe",
                    "size": 9057280,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "stylua.exe",
        "executable_sha256": "8be0383e979b3c699bf6bfbfc0770038534d071b0479fee188636c4b54cd338b",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/JohnnyMorganz/StyLua/blob/v2.6.0/LICENSE.md",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "fd13b4014e3b779c244bbc03f2c282e357f15bc526de6a6fa5ecbc447538313c",
        "size": 2946498,
        "tool": "stylua",
        "upstream_version": "2.6.0",
        "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-windows-x86_64.zip",
    },
}
