"""stylua standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

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
    "schema_version": 1,
    "sha256": "278495597e3827417f3af7b9ad38011ad4a5ec725503553160ab69b7301059bb",
    "size": 3350659,
    "tool": "stylua",
    "upstream_version": "2.6.0",
    "url": "https://github.com/JohnnyMorganz/StyLua/releases/download/v2.6.0/stylua-linux-aarch64-musl.zip",
}
