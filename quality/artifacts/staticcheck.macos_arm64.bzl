"""staticcheck standalone artifact metadata (macos_arm64) -- GENERATED, do not edit.

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
                "is_executable": False,
                "mode": "0o644",
                "name": "staticcheck/LICENSE-THIRD-PARTY",
                "size": 6442,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "staticcheck/LICENSE",
                "size": 1058,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "staticcheck/staticcheck",
                "size": 16247842,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "staticcheck/staticcheck",
    "executable_sha256": "32053b44a877e01aadd13343e504f386efff56d7f344ccb2ccf20b4e045dba79",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/dominikh/go-tools/blob/2026.2.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "06db0a00e5d44d0d60bc99dab10e1bd49103531054d560af658775450a25f72c",
    "size": 8947312,
    "tool": "staticcheck",
    "upstream_version": "2026.2.1",
    "url": "https://github.com/dominikh/go-tools/releases/download/2026.2.1/staticcheck_darwin_arm64.tar.gz",
}
