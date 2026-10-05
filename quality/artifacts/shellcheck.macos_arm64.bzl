"""shellcheck standalone artifact metadata (macos_arm64) -- GENERATED, do not edit.

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
                "name": "shellcheck-v0.11.0/LICENSE.txt",
                "size": 35149,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "shellcheck-v0.11.0/README.txt",
                "size": 2374,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "shellcheck-v0.11.0/shellcheck",
                "size": 61551911,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "shellcheck-v0.11.0/shellcheck",
    "executable_sha256": "61c17246d69f012cd458ae82f244c46023dac75d1b69733ca1cc7d28fb270fd7",
    "interpreter": None,
    "licenses": [
        {
            "name": "GPL-3.0",
            "source": "https://github.com/koalaman/shellcheck/blob/v0.11.0/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "339b930feb1ea764467013cc1f72d09cd6b869ebf1013296ba9055ab2ffbd26f",
    "size": 11370575,
    "tool": "shellcheck",
    "upstream_version": "0.11.0",
    "url": "https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.darwin.aarch64.tar.gz",
}
