"""terraform standalone artifact metadata (windows_arm64) -- GENERATED, do not edit."""

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
                "size": 111778440,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "terraform.exe",
    "executable_sha256": "cf8e1383f9020901db233218193be427b2b1eb5b206f3ff10a3340712028df72",
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
    "sha256": "a1777d9e6bce7764843c23a88afaaaa68c43745dcad76be7bef0cd184c4475d9",
    "size": 32349772,
    "tool": "terraform",
    "upstream_version": "1.16.1",
    "url": "https://releases.hashicorp.com/terraform/1.16.1/terraform_1.16.1_windows_arm64.zip",
}
