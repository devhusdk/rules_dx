"""actionlint standalone artifact metadata (macos_arm64) -- GENERATED, do not edit.

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
                "size": 5829842,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "actionlint",
    "executable_sha256": "8db11704dc296f096216db4db65d86cd7f0ebfdf4c38453a1da276b137b88388",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/rhysd/actionlint/blob/v1.7.12/LICENSE.txt",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "aba9ced2dee8d27fecca3dc7feb1a7f9a52caefa1eb46f3271ea66b6e0e6953f",
    "size": 2164202,
    "tool": "actionlint",
    "upstream_version": "1.7.12",
    "url": "https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_darwin_arm64.tar.gz",
}
