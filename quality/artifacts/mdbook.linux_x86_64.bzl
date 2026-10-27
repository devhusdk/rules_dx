"""mdbook standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
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
    "schema_version": 1,
    "sha256": "c0b903f01dd8f4edc644372ad2b80b1fdddd12552d37b6a098657cbd8eddd768",
    "size": 4658310,
    "tool": "mdbook",
    "upstream_version": "0.4.52",
    "url": "https://github.com/rust-lang/mdBook/releases/download/v0.4.52/mdbook-v0.4.52-x86_64-unknown-linux-gnu.tar.gz",
}
