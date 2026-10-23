"""uv standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.7.0",
        "libc": "2.28",
        "libstdcxx": None,
    },
    "archive": {
        "format": "tar.gz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "uv-aarch64-unknown-linux-gnu",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "uv-aarch64-unknown-linux-gnu/uvx",
                "size": 299128,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "uv-aarch64-unknown-linux-gnu/uv",
                "size": 41366376,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "uv-aarch64-unknown-linux-gnu/uv",
    "executable_sha256": "ed797a095bf9aea58135fa7081c290e51a14cfd3a7b1f24e55cb1d1a0c1007b0",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
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
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "6524bd338177ed50d035d39354e12545e993bbeba2ecbddf0480c5b3a81d313f",
    "size": 18965616,
    "tool": "uv",
    "upstream_version": "0.12.23",
    "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-aarch64-unknown-linux-gnu.tar.gz",
}
