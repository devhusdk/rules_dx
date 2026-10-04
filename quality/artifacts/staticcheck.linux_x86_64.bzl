"""staticcheck standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit.

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
                "size": 17049931,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "staticcheck/staticcheck",
    "executable_sha256": "cafc8e0e9bd4b8310c2a05654b711b6c97b0b494cee30be84894bd4d79069b1e",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/dominikh/go-tools/blob/2026.2.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "91186205a78db3f2d40efb3c102749aef66f85c2204793de7488d163b655aa7c",
    "size": 9418804,
    "tool": "staticcheck",
    "upstream_version": "2026.2.1",
    "url": "https://github.com/dominikh/go-tools/releases/download/2026.2.1/staticcheck_linux_amd64.tar.gz",
}
