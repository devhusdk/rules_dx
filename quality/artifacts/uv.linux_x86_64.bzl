"""uv standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "2.6.32",
        "libc": "2.17",
        "libstdcxx": None,
    },
    "archive": {
        "format": "tar.gz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "uv-x86_64-unknown-linux-gnu",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "uv-x86_64-unknown-linux-gnu/uv",
                "size": 47993144,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "uv-x86_64-unknown-linux-gnu/uvx",
                "size": 331368,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "uv-x86_64-unknown-linux-gnu/uv",
    "executable_sha256": "abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f",
    "interpreter": "/lib64/ld-linux-x86-64.so.2",
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/astral-sh/uv/blob/0.12.23/LICENSE-MIT",
        },
        {
            "name": "Apache-2.0",
            "source": "https://github.com/astral-sh/uv/blob/0.12.23/LICENSE-APACHE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "libc.so.6",
        "libdl.so.2",
        "libgcc_s.so.1",
        "libm.so.6",
        "libpthread.so.0",
        "librt.so.1",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "9167d72b3319674b6303c4cbe071854bba13ebdf3d76b1a7cbdc175471fb66d6",
    "size": 19906150,
    "tool": "uv",
    "upstream_version": "0.12.23",
    "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-x86_64-unknown-linux-gnu.tar.gz",
}
