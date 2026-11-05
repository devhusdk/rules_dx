"""periphery standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
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
                    "name": "periphery",
                    "size": 23552480,
                },
                {
                    "is_executable": True,
                    "mode": "0o755",
                    "name": "libIndexStore.dylib",
                    "size": 2010768,
                },
                {
                    "is_executable": False,
                    "mode": "0o644",
                    "name": "LICENSE.md",
                    "size": 1050,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "periphery",
        "executable_sha256": "043b2c2ff7589b87f2b30c6c9b91e8d9b8e5c6c3cd03d2e3395960e00d53e9b5",
        "interpreter": None,
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/peripheryapp/periphery/blob/3.8.0/LICENSE.md",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "07d4e286e31dd79164df39097e0b59f533c94badbe18158464a455ea88a166d7",
        "size": 8773347,
        "tool": "periphery",
        "upstream_version": "3.8.0",
        "url": "https://github.com/peripheryapp/periphery/releases/download/3.8.0/periphery-3.8.0.zip",
    },
}
