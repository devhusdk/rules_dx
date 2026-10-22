"""terraform standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit."""

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
                "mode": "0o664",
                "name": "LICENSE.txt",
                "size": 4950,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "terraform.exe",
                "size": 123013768,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "terraform.exe",
    "executable_sha256": "31825d5276c00019b22f24796862b1d1989e22ec918bb048d2438e6aba47818a",
    "interpreter": None,
    "licenses": [
        {
            "name": "BUSL-1.1",
            "source": "https://github.com/hashicorp/terraform/blob/v1.16.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "5c6c6d8fedf56ce29c55f0c1fc91de3c259f42c2d220a28e827b5b60fd47bfa1",
    "size": 36699985,
    "tool": "terraform",
    "upstream_version": "1.16.1",
    "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_windows_amd64.zip",
}
