"""terraform standalone artifact metadata (macos_arm64) -- GENERATED, do not edit."""

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
                "name": "terraform",
                "size": 114340064,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "terraform",
    "executable_sha256": "39d08f802c61d133c04b9b22ffcdf2434f1aeec0c4cd5b144698e63441ea57c0",
    "interpreter": None,
    "licenses": [
        {
            "name": "BUSL-1.1",
            "source": "https://github.com/hashicorp/terraform/blob/v1.16.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "e22cba761ddbd4d218939b28715ab3af37aaf8a42efa41f7d75b2c3d73636060",
    "size": 33804928,
    "tool": "terraform",
    "upstream_version": "1.16.1",
    "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_darwin_arm64.zip",
}
