"""mdbook standalone artifact metadata -- GENERATED, do not edit."""

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
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "mdbook",
                    "size": 10474816,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "mdbook",
        "executable_sha256": "207696773a2ce4f37c58782d0853f4179694a289cb7508eaa93e0dc595407294",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.4.52/LICENSE",
            },
        ],
        "linkage": "static",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "7273dda980915a1e2f114d63d432aa6284551e37f0358e3ce7653d1e49e6fa3f",
        "size": 4534216,
        "tool": "mdbook",
        "upstream_version": "0.4.52",
        "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-aarch64-unknown-linux-musl.tar.gz",
    },
    "linux_x86_64": {
        "abi_floor": {
            "kernel": "3.2.0",
            "libc": "2.34",
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.gz",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "mdbook",
                    "size": 11136872,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "mdbook",
        "executable_sha256": "29b7e5ba01321e3bbc4487012544cea516f01377ffd1e26b4adeed9b8ed875a9",
        "interpreter": "/lib64/ld-linux-x86-64.so.2",
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.4.52/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "libc.so.6",
            "libgcc_s.so.1",
            "libm.so.6",
        ],
        "os": "linux",
        "runtime_files": [],
        "sha256": "c0b903f01dd8f4edc644372ad2b80b1fdddd12552d37b6a098657cbd8eddd768",
        "size": 4658310,
        "tool": "mdbook",
        "upstream_version": "0.4.52",
        "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-x86_64-unknown-linux-gnu.tar.gz",
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
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "mdbook",
                    "size": 9419632,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "mdbook",
        "executable_sha256": "80ca3d4d6dfc1a12d231b14ece0dd6bd9cf6233f74b45f3c7ff915b6923eeac5",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.4.52/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "4aee6a8ff54dc59fbe9bcf1c38b8f58583fba63535dff7a5ec8205f0cffc3fd4",
        "size": 4249801,
        "tool": "mdbook",
        "upstream_version": "0.4.52",
        "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-aarch64-apple-darwin.tar.gz",
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
                    "name": "mdbook.exe",
                    "size": 10447872,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "mdbook.exe",
        "executable_sha256": "8400c017423686ad8025532baaf4b1683b8caf7147fbe6fc4cddf7a85902cbde",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.4.52/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "33510a5745c593e06f4b8e21f531d9204c6f4444846b4bd57994e82a7bdfec98",
        "size": 4256111,
        "tool": "mdbook",
        "upstream_version": "0.4.52",
        "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-x86_64-pc-windows-msvc.zip",
    },
}
