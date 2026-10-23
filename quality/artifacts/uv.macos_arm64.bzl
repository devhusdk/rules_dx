"""uv standalone artifact metadata (macos_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": None,
        "libc": None,
        "libstdcxx": None,
    },
    "archive": {
        "format": "tar.gz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "uv-aarch64-apple-darwin",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "uv-aarch64-apple-darwin/uvx",
                "size": 335536,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "uv-aarch64-apple-darwin/uv",
                "size": 35517728,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "uv-aarch64-apple-darwin/uv",
    "executable_sha256": "2f7be1879e3337eef20875c0f80ef033337305d0405efc5d72642d6292f66b3b",
    "interpreter": None,
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
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "50487ae565ccd96e499056b4674d438f4c53170202617b4c759defe0c6a1b544",
    "size": 17044217,
    "tool": "uv",
    "upstream_version": "0.12.23",
    "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-aarch64-apple-darwin.tar.gz",
}
