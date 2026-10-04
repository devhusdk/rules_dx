"""cue standalone artifact metadata (macos_arm64) -- GENERATED, do not edit.

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
                "size": 23320034,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "cue",
    "executable_sha256": "96fe60cbbe4dac69bae9199a2c4c69837559251dae3120609c0ecc666ec744c2",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/cue-lang/cue/blob/v0.17.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "64921403f012a97f89494c03605db2fbf7d9daa77dc2631819ac4406cb2e8074",
    "size": 9128908,
    "tool": "cue",
    "upstream_version": "v0.17.1",
    "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_darwin_arm64.tar.gz",
}
