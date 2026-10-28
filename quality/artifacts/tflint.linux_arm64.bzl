"""tflint standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

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
                "mode": "0o755",
                "name": "tflint",
                "size": 51839138,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "tflint",
    "executable_sha256": "d6cf983371d8a04e791ffb3a3c34e24c9b1c030dd35370b4c13370468af966c0",
    "interpreter": None,
    "licenses": [
        {
            "name": "MPL-2.0",
            "source": "https://github.com/terraform-linters/tflint/blob/v0.65.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "843017c47a98be1de4d88fa038daa293770fe25cd01656c4f08b2e30d1d3b3a0",
    "size": 15856442,
    "tool": "tflint",
    "upstream_version": "0.65.0",
    "url": "https://github.com/terraform-linters/tflint/releases/download/v0.65.0/tflint_linux_arm64.zip",
}
