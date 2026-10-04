"""staticcheck standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit.

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
                "mode": "0o755",
                "name": "staticcheck",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "staticcheck/staticcheck.exe",
                "size": 17298432,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "staticcheck/LICENSE",
                "size": 1058,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "staticcheck/LICENSE-THIRD-PARTY",
                "size": 6442,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "staticcheck/staticcheck.exe",
    "executable_sha256": "f06891e46b30b96d8bbf19dad1c8364f821e30c26e0bddee08638b32b0be3e5a",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/dominikh/go-tools/blob/2026.2.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "c111b68123d4fb738d09184d8dae37211488431259150c658a4b80d7ef3c1c2c",
    "size": 9504076,
    "tool": "staticcheck",
    "upstream_version": "2026.2.1",
    "url": "https://github.com/dominikh/go-tools/releases/download/2026.2.1/staticcheck_windows_amd64.tar.gz",
}
