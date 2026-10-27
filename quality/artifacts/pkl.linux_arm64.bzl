"""pkl standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.7.0",
        "libc": "2.17",
        "libstdcxx": None,
    },
    "archive": {
        "format": "none",
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "pkl-linux-aarch64",
    "executable_sha256": "a76d2dd47da435a8f911b0347373f47c7e59ea54fb75ff846d20b8df10dba058",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
    "licenses": [
        {
            "name": "Apache-2.0",
            "source": "https://github.com/apple/pkl/blob/0.32.1/LICENSE.txt",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "ld-linux-aarch64.so.1",
        "libc.so.6",
        "libdl.so.2",
        "libpthread.so.0",
        "librt.so.1",
        "libz.so.1",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "a76d2dd47da435a8f911b0347373f47c7e59ea54fb75ff846d20b8df10dba058",
    "size": 98635256,
    "tool": "pkl",
    "upstream_version": "0.32.1",
    "url": "https://github.com/apple/pkl/releases/download/0.32.1/pkl-linux-aarch64",
}
