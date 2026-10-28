"""gersemi standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.2.0",
        "libc": "2.14",
        "libstdcxx": None,
    },
    "archive": {
        "format": "none",
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "gersemi-linux",
    "executable_sha256": "34ae3c8e32f65ca64361149c5e97dbb523f446b837588784b5e98e29a1c93af9",
    "interpreter": "/lib64/ld-linux-x86-64.so.2",
    "licenses": [
        {
            "name": "MPL-2.0",
            "source": "https://github.com/BlankSpruce/gersemi/blob/0.29.2/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "libc.so.6",
        "libdl.so.2",
        "libpthread.so.0",
        "libz.so.1",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "34ae3c8e32f65ca64361149c5e97dbb523f446b837588784b5e98e29a1c93af9",
    "size": 25449344,
    "tool": "gersemi",
    "upstream_version": "0.29.2",
    "url": "https://github.com/BlankSpruce/gersemi/releases/download/0.29.2/gersemi-linux",
}
