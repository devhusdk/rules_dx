"""ffmt standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "4.10.17",
        "libc": "2.17",
        "libstdcxx": None,
    },
    "archive": {
        "format": "zip",
        "members": [
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "ffmt-0.4.5.data/scripts/ffmt",
                "size": 5138624,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/METADATA",
                "size": 3958,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/WHEEL",
                "size": 143,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/licenses/LICENSE",
                "size": 1079,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/sboms/ffmt.cyclonedx.json",
                "size": 56449,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "ffmt-0.4.5.dist-info/RECORD",
                "size": 484,
            },
        ],
    },
    "checksum_source": "PyPI JSON API sha256 digest",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "ffmt-0.4.5.data/scripts/ffmt",
    "executable_sha256": "67b530337952cf397ade195479313387a50394c64f15ec9743d2fcf54d06a8be",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/sbryngelson/ffmt/blob/v0.4.5/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "libc.so.6",
        "libdl.so.2",
        "libgcc_s.so.1",
        "libpthread.so.0",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "9c8e8770043c5418cf8aa4f34d10df691f444ed9ee94fac26df873d65ba9c341",
    "size": 1801772,
    "tool": "ffmt",
    "upstream_version": "0.4.5",
    "url": "https://files.pythonhosted.org/packages/10/51/9a1dd26978fa168e54b2f51379f869a2eeb7d3e0b6927abc260243924c27/ffmt-0.4.5-py3-none-manylinux_2_17_aarch64.manylinux2014_aarch64.whl",
}
