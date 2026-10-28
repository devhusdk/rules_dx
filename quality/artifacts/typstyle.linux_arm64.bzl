"""typstyle standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.7.0",
        "libc": "2.34",
        "libstdcxx": None,
    },
    "archive": {
        "format": "none",
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "typstyle-aarch64-unknown-linux-gnu",
    "executable_sha256": "4021f09180d04b804abfd6f5def3b09a0c63ecbb4d40e0d9b4ddd98e7b388f12",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/typstyle-rs/typstyle/blob/v0.15.2/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "libc.so.6",
        "libgcc_s.so.1",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "4021f09180d04b804abfd6f5def3b09a0c63ecbb4d40e0d9b4ddd98e7b388f12",
    "size": 2495336,
    "tool": "typstyle",
    "upstream_version": "0.15.2",
    "url": "https://github.com/typstyle-rs/typstyle/releases/download/v0.15.2/typstyle-aarch64-unknown-linux-gnu",
}
