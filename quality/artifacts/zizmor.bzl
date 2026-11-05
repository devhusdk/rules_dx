"""zizmor standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_arm64": {
        "abi_floor": {
            "kernel": "3.7.0",
            "libc": "2.34",
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.gz",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "zizmor",
                    "size": 25703120,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "zizmor",
        "executable_sha256": "5056aebea3b0ec5238921c5fd11dae104fdb65589e3ed0a8bb2e06ef85d6e809",
        "interpreter": "/lib/ld-linux-aarch64.so.1",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/zizmorcore/zizmor/blob/v1.32.0/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "ld-linux-aarch64.so.1",
            "libc.so.6",
            "libgcc_s.so.1",
            "libm.so.6",
        ],
        "os": "linux",
        "runtime_files": [],
        "sha256": "cb152b207f815116d7fa1c1ee4614f61578803351395052908095e79adfd9fd0",
        "size": 9004349,
        "tool": "zizmor",
        "upstream_version": "1.32.0",
        "url": "https://github.com/zizmorcore/zizmor/releases/download/v1.32.0/zizmor-aarch64-unknown-linux-gnu.tar.gz",
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
                    "name": "zizmor",
                    "size": 27485344,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "zizmor",
        "executable_sha256": "04d55fb88e582a8e9a9ce6b4753df723fc44c35e0d1a8e8771db9e017066c6df",
        "interpreter": "/lib64/ld-linux-x86-64.so.2",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/zizmorcore/zizmor/blob/v1.32.0/LICENSE",
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
        "sha256": "bb110cc765eca9cd7efb2e284a956be5974c60027b0970f16397d86208a9957b",
        "size": 9418466,
        "tool": "zizmor",
        "upstream_version": "1.32.0",
        "url": "https://github.com/zizmorcore/zizmor/releases/download/v1.32.0/zizmor-x86_64-unknown-linux-gnu.tar.gz",
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
                    "name": "zizmor",
                    "size": 23639408,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "zizmor",
        "executable_sha256": "fd1e0503f98714dac56fb6859ccb8ddbac2dc1eb00562dc9d174c6858994c0f1",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/zizmorcore/zizmor/blob/v1.32.0/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "6efed18c4539a663fd3c851a2be9533fd8456c4e646d9416206da059154a0d3c",
        "size": 8597835,
        "tool": "zizmor",
        "upstream_version": "1.32.0",
        "url": "https://github.com/zizmorcore/zizmor/releases/download/v1.32.0/zizmor-aarch64-apple-darwin.tar.gz",
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
                    "name": "zizmor.exe",
                    "size": 25298944,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "zizmor.exe",
        "executable_sha256": "7e15e89277a8d051589476a6c3333067541dc469ddc59535545bd458d0fc6d82",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/zizmorcore/zizmor/blob/v1.32.0/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "d284a16e09d9df4ce2c000357e7b59bd531be093bea5718a1ce225b449b59d49",
        "size": 8532182,
        "tool": "zizmor",
        "upstream_version": "1.32.0",
        "url": "https://github.com/zizmorcore/zizmor/releases/download/v1.32.0/zizmor-x86_64-pc-windows-msvc.zip",
    },
}
