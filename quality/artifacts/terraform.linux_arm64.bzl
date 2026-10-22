"""terraform standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

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
                "size": 110559394,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "terraform",
    "executable_sha256": "6d4234db9e03a2203f844037ee1de02657e5bd52c097a89770292c929bf2b122",
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
    "sha256": "423288a23ab024d42ac05c409972585f7ec0cf1be572b773ad952f9a1c41387d",
    "size": 32486274,
    "tool": "terraform",
    "upstream_version": "1.16.1",
    "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_linux_arm64.zip",
}
