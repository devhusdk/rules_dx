"""shellcheck standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit."""

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
                "name": "LICENSE.txt",
                "size": 35149,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.txt",
                "size": 2374,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "shellcheck.exe",
                "size": 34778624,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "shellcheck.exe",
    "executable_sha256": "c9e82ada36ef4b8d4caf1f97fa89289048c8f4a33c2c76ffffc88bfe09ff00c5",
    "interpreter": None,
    "licenses": [
        {
            "name": "GPL-3.0",
            "source": "https://github.com/koalaman/shellcheck/blob/v0.11.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "8a4e35ab0b331c85d73567b12f2a444df187f483e5079ceffa6bda1faa2e740e",
    "size": 8068167,
    "tool": "shellcheck",
    "upstream_version": "0.11.0",
    "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.zip",
}
