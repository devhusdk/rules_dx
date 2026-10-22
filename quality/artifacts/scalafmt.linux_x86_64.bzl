"""scalafmt standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.2.0",
        "libc": "2.34",
        "libstdcxx": None,
    },
    "archive": {
        "format": "zip",
        "members": [
            {
                "is_executable": True,
                "mode": "0o744",
                "name": "scalafmt",
                "size": 57205024,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "scalafmt",
    "executable_sha256": "88526f9f4d64c2fb023d54578812419f49e2ec09e30e4fb77443a05f1a59cac0",
    "interpreter": "/lib64/ld-linux-x86-64.so.2",
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/scalameta/scalafmt/blob/v3.11.4/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "libc.so.6",
        "libm.so.6",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "e7d43a5621074a63a46d5b287d0b0bb0650033deeb836af2b27515b2127476f2",
    "size": 16466224,
    "tool": "scalafmt",
    "upstream_version": "3.11.4",
    "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-x86_64-pc-linux.zip",
}
