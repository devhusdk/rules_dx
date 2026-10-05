"""actionlint standalone artifact metadata (linux_arm64) -- GENERATED, do not edit.

Regenerate with: bazel run //quality/artifacts:update
"""

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
                "mode": "0o644",
                "name": "LICENSE.txt",
                "size": 1067,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.md",
                "size": 8243,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "docs/README.md",
                "size": 768,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "docs/api.md",
                "size": 4290,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "docs/checks.md",
                "size": 129765,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "docs/config.md",
                "size": 3525,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "docs/install.md",
                "size": 6950,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "docs/reference.md",
                "size": 1634,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "docs/usage.md",
                "size": 19841,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "man/actionlint.1",
                "size": 7278,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "actionlint",
                "size": 5832866,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "actionlint",
    "executable_sha256": "ac0323433c2853ec3fb978c611430c5b3dc5d43c58d1a1ec031b00ab572beb60",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/rhysd/actionlint/blob/v1.7.12/LICENSE.txt",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "325e971b6ba9bfa504672e29be93c24981eeb1c07576d730e9f7c8805afff0c6",
    "size": 2111482,
    "tool": "actionlint",
    "upstream_version": "1.7.12",
    "url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_arm64.tar.gz",
}
