"""cue standalone artifact metadata (linux_arm64) -- GENERATED, do not edit.

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
                "name": "LICENSE",
                "size": 11358,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.md",
                "size": 4234,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "doc/ref/spec.md",
                "size": 101116,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "doc/tutorial/basics/README.md",
                "size": 176,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "doc/tutorial/kubernetes/README.md",
                "size": 138,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "cue",
                "size": 22544546,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "cue",
    "executable_sha256": "60aa52c46fc02226b9790698900ab578764227a1e7801da5630324b4b72e2df3",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/cue-lang/cue/blob/v0.17.1/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "0d729be30d52c952ca38fc9dcb692caa09d8463fa0b64df5781312779183fbcd",
    "size": 8769454,
    "tool": "cue",
    "upstream_version": "v0.17.1",
    "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_linux_arm64.tar.gz",
}
