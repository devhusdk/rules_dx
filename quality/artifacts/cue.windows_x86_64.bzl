"""cue standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit.

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
        "format": "zip",
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
                "name": "cue.exe",
                "size": 25128448,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "cue.exe",
    "executable_sha256": "a2452301a1abeaec8b46d4ce13b5ad5875851d447bcaeda43c556418b9b57e08",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/cue-lang/cue/blob/v0.17.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "9f15378dc52b9a1bb6fa1755adc0410c9f17f330a621b784a5b31fdbc91c6d7a",
    "size": 9911896,
    "tool": "cue",
    "upstream_version": "v0.17.1",
    "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_windows_amd64.zip",
}
