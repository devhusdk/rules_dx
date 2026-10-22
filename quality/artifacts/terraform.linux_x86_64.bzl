"""terraform standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

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
                "mode": "0o644",
                "name": "LICENSE.txt",
                "size": 4950,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "terraform",
                "size": 120320162,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "terraform",
    "executable_sha256": "0b5a4e400548d9538af88a4c5a2726b97b38751b8f577aa6687ad051ba2070f2",
    "interpreter": None,
    "licenses": [
        {
            "name": "BUSL-1.1",
            "source": "https://github.com/hashicorp/terraform/blob/v1.16.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "745d33b4b02b7980c62a38ec1beea24ee084ea8caf3f503c200554bd9a0cbe49",
    "size": 36304618,
    "tool": "terraform",
    "upstream_version": "1.16.1",
    "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_linux_amd64.zip",
}
