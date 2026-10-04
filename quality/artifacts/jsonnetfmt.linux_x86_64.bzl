"""jsonnetfmt standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit.

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
                "size": 7699,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet",
                "size": 6123704,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnetfmt",
                "size": 4030648,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-lint",
                "size": 4178104,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonnet-deps",
                "size": 3780792,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "jsonnetfmt",
    "executable_sha256": "a4e08e923ae45ae7f6314f7e64b6bee24dd31135e5c83e80c488cc87ba9d8aef",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/google/go-jsonnet/blob/v0.22.0/LICENSE",
        },
    ],
    "linkage": "static",
    "needed_shared_libraries": [],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "e87a93ea44e34da92c15636205c7f2240bf3ac92d00ccb855dbb4e7e03ea6941",
    "size": 6708178,
    "tool": "jsonnetfmt",
    "upstream_version": "v0.22.0",
    "url": "https://github.com/google/go-jsonnet/releases/download/v0.22.0/go-jsonnet_0.22.0_linux_amd64.tar.gz",
}
