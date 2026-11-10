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
                    "size": 12078560,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "mdbook",
        "executable_sha256": "c3526481bf31515c75019ddeb2ac49bec1ed24b53f1b4d7c169e140f5d770c92",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.5.4/LICENSE",
            },
        ],
        "linkage": "static",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "753e5c5c363ee8a56972344dcf91466f005a51db84a7aeffe427ae3ef83d6d44",
        "size": 4758117,
        "tool": "mdbook",
        "upstream_version": "0.5.4",
        "url": "https://github.com/rust-lang/mdBook/releases/download/v0.5.4/mdbook-v0.5.4-aarch64-unknown-linux-musl.tar.gz",
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
                    "size": 12665904,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "mdbook",
        "executable_sha256": "dc9020903f60cf632c0cc01bf8c12b57237649c2ab64c7a85a60e12103e356ce",
        "interpreter": "/lib64/ld-linux-x86-64.so.2",
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.5.4/LICENSE",
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
        "sha256": "3f28de05dafca9d0f2eab99c662116b0e37b89b1d96a08f8f430b9eeae958cd7",
        "size": 4822940,
        "tool": "mdbook",
        "upstream_version": "0.5.4",
        "url": "https://github.com/rust-lang/mdBook/releases/download/v0.5.4/mdbook-v0.5.4-x86_64-unknown-linux-gnu.tar.gz",
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
                    "size": 10853008,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "mdbook",
        "executable_sha256": "49f163c7a5a9d0a4fc9cacf34bd0b96b11801458effddb1b7e951377af909d8e",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.5.4/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "03e8a6d8b13a2971e0b3280affd03b388373c1485e26f73407c3a76b0b1838df",
        "size": 4420902,
        "tool": "mdbook",
        "upstream_version": "0.5.4",
        "url": "https://github.com/rust-lang/mdBook/releases/download/v0.5.4/mdbook-v0.5.4-aarch64-apple-darwin.tar.gz",
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
                    "size": 11880960,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "mdbook.exe",
        "executable_sha256": "ffa917c9b6b608aa8b6189dcb38a548ff2615832113ad6c6e26f6ef9c8d77a44",
        "interpreter": None,
        "licenses": [
            {
                "name": "MPL-2.0",
                "source": "https://github.com/rust-lang/mdBook/blob/v0.5.4/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "8a6b2421aa522de06d746871d2b8fff9c8d71773467e310a4516d2566e7f2de4",
        "size": 4407719,
        "tool": "mdbook",
        "upstream_version": "0.5.4",
        "url": "https://github.com/rust-lang/mdBook/releases/download/v0.5.4/mdbook-v0.5.4-x86_64-pc-windows-msvc.zip",
    },
}
