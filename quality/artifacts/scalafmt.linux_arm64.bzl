"""scalafmt standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.7.0",
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
                "size": 54401512,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "scalafmt",
    "executable_sha256": "799e9d7412971b254a2ff216c7313673c8e4ecd162a228208dd9c93ba66cf70b",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
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
    "sha256": "07fd593f3efb887a71f8400db48a9424279d37bf712630ebe1ea337ccc1945e7",
    "size": 14356691,
    "tool": "scalafmt",
    "upstream_version": "3.11.4",
    "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-aarch64-pc-linux.zip",
}
