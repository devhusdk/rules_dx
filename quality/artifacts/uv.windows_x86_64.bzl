"""uv standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit."""

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
                "mode": "0o0",
                "name": "uv.exe",
                "size": 40175408,
            },
            {
                "is_executable": True,
                "mode": "0o0",
                "name": "uvw.exe",
                "size": 346416,
            },
            {
                "is_executable": True,
                "mode": "0o0",
                "name": "uvx.exe",
                "size": 346928,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "uv.exe",
    "executable_sha256": "70742ae9bd9f64f984fd69126662cfda5c03558bcb40075b2263833e4075f744",
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
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "75d05de6762778c31ee183398de7dd15093fad0ed90b1f236d8205ea5ec00c90",
    "size": 18043715,
    "tool": "uv",
    "upstream_version": "0.12.23",
    "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-x86_64-pc-windows-msvc.zip",
}
