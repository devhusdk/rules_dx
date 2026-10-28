"""helm standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit."""

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
                "is_executable": False,
                "mode": "0o755",
                "name": "windows-amd64/",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "windows-amd64/helm.exe",
                "size": 67873792,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "windows-amd64/LICENSE",
                "size": 11373,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "windows-amd64/README.md",
                "size": 4410,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "windows-amd64/helm.exe",
    "executable_sha256": "912d7210c880c48b2ac91102b1297d6ac637edd9aaaf05beb12da0b496b53ba9",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/helm/helm/blob/v4.3.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "fcfd7b796dcf0f225abc40b5636a3b01f55f3de2fb1dfb18475df94e62c14112",
    "size": 20865982,
    "tool": "helm",
    "upstream_version": "4.3.1",
    "url": "https://get.helm.sh/helm-v4.3.1-windows-amd64.zip",
}
