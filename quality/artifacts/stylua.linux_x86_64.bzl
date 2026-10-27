"""stylua standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
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
    "schema_version": 1,
    "sha256": "48ee7db28346f96f4bd545e190ab080a88a7b5a664e97649cf5d5c60426f402d",
    "size": 3480092,
    "tool": "stylua",
    "upstream_version": "2.6.0",
    "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-linux-x86_64-musl.zip",
}
