"""uv standalone artifact metadata (windows_arm64) -- GENERATED, do not edit."""

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
                "size": 39656240,
            },
            {
                "is_executable": True,
                "mode": "0o0",
                "name": "uvw.exe",
                "size": 331568,
            },
            {
                "is_executable": True,
                "mode": "0o0",
                "name": "uvx.exe",
                "size": 332080,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "uv.exe",
    "executable_sha256": "d9c5f88091f0f53f9c26ddd64af5154c853ce69c87410e219180ad2fb0abcf73",
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
    "sha256": "13294e232ececbe709c06b74e6ced06f2a225ea5591476685362f22be56a50d5",
    "size": 19113421,
    "tool": "uv",
    "upstream_version": "0.12.23",
    "url": "https://github.com/astral-sh/uv/releases/download/0.12.23/uv-aarch64-pc-windows-msvc.zip",
}
