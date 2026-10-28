"""gersemi standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

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
    "executable": "gersemi-linux-arm",
    "executable_sha256": "6aafd13b0363359fb56f8c8a96b34c4f72aef21ff2a3859cb3156294e86decf1",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
    "licenses": [
        {
            "name": "MPL-2.0",
            "source": "https://github.com/BlankSpruce/gersemi/blob/0.29.2/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "ld-linux-aarch64.so.1",
        "libc.so.6",
        "libdl.so.2",
        "libpthread.so.0",
        "libz.so.1",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "6aafd13b0363359fb56f8c8a96b34c4f72aef21ff2a3859cb3156294e86decf1",
    "size": 24411368,
    "tool": "gersemi",
    "upstream_version": "0.29.2",
    "url": "https://github.com/BlankSpruce/gersemi/releases/download/0.29.2/gersemi-linux-arm",
}
