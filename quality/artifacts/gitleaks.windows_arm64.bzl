"""gitleaks standalone artifact metadata (windows_arm64) -- GENERATED, do not edit.

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
                "size": 1069,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "README.md",
                "size": 29998,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "gitleaks.exe",
                "size": 21111296,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "gitleaks.exe",
    "executable_sha256": "200df852fdecbedb19a33960657333cba5e231740bc8968972b507b50f93b194",
    "interpreter": None,
    "licenses": [
        {
            "name": "MIT",
            "source": "https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "b95f5e4f5c425cedca7ee203d9afd29597e692c4924a12ed42f970537c72cc0f",
    "size": 7701820,
    "tool": "gitleaks",
    "upstream_version": "8.30.1",
    "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_windows_arm64.zip",
}
