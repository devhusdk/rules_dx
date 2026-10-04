"""cue standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
                "size": 22881792,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "cue.exe",
    "executable_sha256": "3c8f78a928c6b6039194ea5c11b44bd587b804c8489526ee3c7150b330d09959",
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
    "sha256": "db88b2c302be1829e653e20ea87e3f1adc10ae2052d58fe87d21d0ed97102e51",
    "size": 8850476,
    "tool": "cue",
    "upstream_version": "v0.17.1",
    "url": "https://github.com/cue-lang/cue/releases/download/v0.17.1/cue_v0.17.1_windows_arm64.zip",
}
