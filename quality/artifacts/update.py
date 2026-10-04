#!/usr/bin/env python3
"""Regenerate checked-in artifact metadata."""

import gzip
import hashlib
import os
import re
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request
import zipfile
from typing import Any

SCHEMA_VERSION = 1

FETCH_RETRIES = 3
FETCH_TIMEOUT = 300
FETCH_BACKOFF_SECONDS = 2

TOOLS: dict[str, dict[str, Any]] = {
    "buildifier": {
        "upstream_version": "8.5.1",
        "release_page": "https://github.com/bazel-contrib/buildtools/releases/tag/v8.5.1",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/bazel-contrib/buildtools/blob/v8.5.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "buildifier-linux-amd64",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-linux-amd64",
                "kind": "raw",
                "executable": "buildifier-linux-amd64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "buildifier-linux-arm64",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-linux-arm64",
                "kind": "raw",
                "executable": "buildifier-linux-arm64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "buildifier-darwin-arm64",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-darwin-arm64",
                "kind": "raw",
                "executable": "buildifier-darwin-arm64",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "buildifier-windows-amd64.exe",
                "url": "https://github.com/bazel-contrib/buildtools/releases/download/v8.5.1/buildifier-windows-amd64.exe",
                "kind": "raw",
                "executable": "buildifier-windows-amd64.exe",
            },
        },
    },
    "biome": {
        "upstream_version": "2.5.12",
        "release_page": "https://github.com/biomejs/biome/releases/tag/@biomejs/biome@2.5.12",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/biomejs/biome/blob/main/LICENSE-MIT",
            },
            {
                "name": "Apache-2.0",
                "source": "https://github.com/biomejs/biome/blob/main/LICENSE-APACHE",
            },
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "biome-linux-x64",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-linux-x64",
                "kind": "raw",
                "executable": "biome-linux-x64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "biome-linux-arm64",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-linux-arm64",
                "kind": "raw",
                "executable": "biome-linux-arm64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "biome-darwin-arm64",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-darwin-arm64",
                "kind": "raw",
                "executable": "biome-darwin-arm64",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "biome-win32-x64.exe",
                "url": "https://github.com/biomejs/biome/releases/download/@biomejs/biome@2.5.12/biome-win32-x64.exe",
                "kind": "raw",
                "executable": "biome-win32-x64.exe",
            },
        },
    },
    "gofumpt": {
        "upstream_version": "v0.12.0",
        "release_page": "https://github.com/mvdan/gofumpt/releases/tag/v0.12.0",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/mvdan/gofumpt/blob/v0.12.0/LICENSE",
            },
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "gofumpt_v0.12.0_linux_amd64",
                "url": "https://github.com/mvdan/gofumpt/releases/download/v0.12.0/gofumpt_v0.12.0_linux_amd64",
                "kind": "raw",
                "executable": "gofumpt_v0.12.0_linux_amd64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "gofumpt_v0.12.0_linux_arm64",
                "url": "https://github.com/mvdan/gofumpt/releases/download/v0.12.0/gofumpt_v0.12.0_linux_arm64",
                "kind": "raw",
                "executable": "gofumpt_v0.12.0_linux_arm64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "gofumpt_v0.12.0_darwin_arm64",
                "url": "https://github.com/mvdan/gofumpt/releases/download/v0.12.0/gofumpt_v0.12.0_darwin_arm64",
                "kind": "raw",
                "executable": "gofumpt_v0.12.0_darwin_arm64",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "gofumpt_v0.12.0_windows_amd64.exe",
                "url": "https://github.com/mvdan/gofumpt/releases/download/v0.12.0/gofumpt_v0.12.0_windows_amd64.exe",
                "kind": "raw",
                "executable": "gofumpt_v0.12.0_windows_amd64.exe",
            },
        },
    },
    "staticcheck": {
        "upstream_version": "2026.2.1",
        "release_page": "https://github.com/dominikh/go-tools/releases/tag/2026.2.1",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/dominikh/go-tools/blob/2026.2.1/LICENSE",
            },
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "staticcheck_linux_amd64.tar.gz",
                "url": "https://github.com/dominikh/go-tools/releases/download/2026.2.1/staticcheck_linux_amd64.tar.gz",
                "kind": "tar.gz",
                "executable": "staticcheck/staticcheck",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "staticcheck_linux_arm64.tar.gz",
                "url": "https://github.com/dominikh/go-tools/releases/download/2026.2.1/staticcheck_linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "staticcheck/staticcheck",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "staticcheck_darwin_arm64.tar.gz",
                "url": "https://github.com/dominikh/go-tools/releases/download/2026.2.1/staticcheck_darwin_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "staticcheck/staticcheck",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "staticcheck_windows_amd64.tar.gz",
                "url": "https://github.com/dominikh/go-tools/releases/download/2026.2.1/staticcheck_windows_amd64.tar.gz",
                "kind": "tar.gz",
                "executable": "staticcheck/staticcheck.exe",
            },
        },
    },
    "taplo": {
        "upstream_version": "0.10.0",
        "release_page": "https://github.com/tamasfe/taplo/releases/tag/0.10.0",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/tamasfe/taplo/blob/0.10.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "taplo-linux-x86_64.gz",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-linux-x86_64.gz",
                "kind": "gzip",
                "executable": "taplo-x86_64",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "taplo-linux-aarch64.gz",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-linux-aarch64.gz",
                "kind": "gzip",
                "executable": "taplo-aarch64",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "taplo-darwin-aarch64.gz",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-darwin-aarch64.gz",
                "kind": "gzip",
                "executable": "taplo",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "taplo-windows-x86_64.zip",
                "url": "https://github.com/tamasfe/taplo/releases/download/0.10.0/taplo-windows-x86_64.zip",
                "kind": "zip",
                "executable": "taplo.exe",
            },
        },
    },
    "vale": {
        "upstream_version": "3.20.0",
        "release_page": "https://github.com/vale-cli/vale/releases/tag/v3.20.0",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/vale-cli/vale/blob/v3.20.0/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "vale_3.20.0_Linux_64-bit.tar.gz",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Linux_64-bit.tar.gz",
                "kind": "tar.gz",
                "executable": "vale",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "vale_3.20.0_Linux_arm64.tar.gz",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "vale",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "vale_3.20.0_macOS_arm64.tar.gz",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_macOS_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "vale",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "vale_3.20.0_Windows_64-bit.zip",
                "url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_Windows_64-bit.zip",
                "kind": "zip",
                "executable": "vale.exe",
                "checksums_url": "https://github.com/vale-cli/vale/releases/download/v3.20.0/vale_3.20.0_checksums.txt",
            },
        },
    },
    "ruff": {
        "upstream_version": "0.16.7",
        "release_page": "https://github.com/astral-sh/ruff/releases/tag/0.16.7",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/astral-sh/ruff/blob/0.16.7/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "ruff-x86_64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "ruff-x86_64-unknown-linux-gnu/ruff",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-unknown-linux-gnu.tar.gz.sha256",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "ruff-aarch64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "ruff-aarch64-unknown-linux-gnu/ruff",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-unknown-linux-gnu.tar.gz.sha256",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "ruff-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "ruff-aarch64-apple-darwin/ruff",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-aarch64-apple-darwin.tar.gz.sha256",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "ruff-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "ruff.exe",
                "checksums_url": "https://github.com/astral-sh/ruff/releases/download/0.16.7/ruff-x86_64-pc-windows-msvc.zip.sha256",
            },
        },
    },
    "gitleaks": {
        "upstream_version": "8.30.1",
        "release_page": "https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/gitleaks/gitleaks/blob/v8.30.1/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "gitleaks_8.30.1_linux_x64.tar.gz",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz",
                "kind": "tar.gz",
                "executable": "gitleaks",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "gitleaks_8.30.1_linux_arm64.tar.gz",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "gitleaks",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "gitleaks_8.30.1_darwin_arm64.tar.gz",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_darwin_arm64.tar.gz",
                "kind": "tar.gz",
                "executable": "gitleaks",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "gitleaks_8.30.1_windows_x64.zip",
                "url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_windows_x64.zip",
                "kind": "zip",
                "executable": "gitleaks.exe",
                "checksums_url": "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_checksums.txt",
            },
        },
    },
    "ty": {
        "upstream_version": "0.0.80",
        "release_page": "https://github.com/astral-sh/ty/releases/tag/0.0.80",
        "licenses": [
            {
                "name": "MIT",
                "source": "https://github.com/astral-sh/ty/blob/0.0.80/LICENSE",
            }
        ],
        "platforms": {
            "linux_x86_64": {
                "os": "linux",
                "cpu": "x86_64",
                "asset": "ty-x86_64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-x86_64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "ty-x86_64-unknown-linux-gnu/ty",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
            "linux_arm64": {
                "os": "linux",
                "cpu": "arm64",
                "asset": "ty-aarch64-unknown-linux-gnu.tar.gz",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-aarch64-unknown-linux-gnu.tar.gz",
                "kind": "tar.gz",
                "executable": "ty-aarch64-unknown-linux-gnu/ty",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
            "macos_arm64": {
                "os": "macos",
                "cpu": "arm64",
                "asset": "ty-aarch64-apple-darwin.tar.gz",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-aarch64-apple-darwin.tar.gz",
                "kind": "tar.gz",
                "executable": "ty-aarch64-apple-darwin/ty",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
            "windows_x86_64": {
                "os": "windows",
                "cpu": "x86_64",
                "asset": "ty-x86_64-pc-windows-msvc.zip",
                "url": "https://github.com/astral-sh/ty/releases/download/0.0.80/ty-x86_64-pc-windows-msvc.zip",
                "kind": "zip",
                "executable": "ty.exe",
                "checksums_url": "https://github.com/astral-sh/ty/releases/download/0.0.80/sha256.sum",
            },
        },
    },
}


def _require_tool(name):
    for directory in os.environ.get("PATH", "").split(os.pathsep):
        candidate = os.path.join(directory, name)
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
    sys.exit("update: required maintainer tool %r not found on PATH" % name)


def _run(argv):
    process = subprocess.run(argv, capture_output=True, text=True)
    if process.returncode != 0:
        sys.exit("update: %s failed:\n%s" % (" ".join(argv), process.stderr))
    return process.stdout


def _download(url, path):
    scheme = urllib.parse.urlparse(url).scheme
    if scheme != "https":
        sys.exit("update: refusing non-https fetch for %r" % url)
    last_error = None
    for attempt in range(1, FETCH_RETRIES + 1):
        try:
            request = urllib.request.Request(
                url, headers={"User-Agent": "rules_dx-artifact-update"}
            )
            with (
                urllib.request.urlopen(request, timeout=FETCH_TIMEOUT) as response,
                open(path, "wb") as out,
            ):
                for chunk in iter(lambda: response.read(65536), b""):
                    out.write(chunk)
            return
        except Exception as error:  # noqa: BLE001
            last_error = error
            if attempt < FETCH_RETRIES:
                time.sleep(FETCH_BACKOFF_SECONDS * attempt)
    sys.exit(
        "update: download failed after %d attempts for %r: %s"
        % (FETCH_RETRIES, url, last_error)
    )


def _sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _elf_linkage(path):
    readelf = _require_tool("readelf")
    headers = _run([readelf, "-h", "-l", "-d", path])
    linkage = "static"
    if re.search(r"Type:.*DYN", headers):
        linkage = "pie" if "INTERP" not in headers else "dynamic"
    elif "INTERP" in headers:
        linkage = "dynamic"
    interpreter = None
    match = re.search(r"\[Requesting program interpreter: ([^\]]+)\]", headers)
    if match:
        interpreter = match.group(1)
    needed = sorted(set(re.findall(r"\(NEEDED\)[^[]*\[([^\]]+)\]", headers)))
    return linkage, interpreter, needed


def _abi_floor(path):
    floor = {"kernel": None, "libc": None, "libstdcxx": None}
    linkage, _, _ = _elf_linkage(path)
    if linkage == "static":
        return floor
    objdump = _require_tool("objdump")
    dynamic = _run([objdump, "-T", path])
    glibc = re.findall(r"GLIBC_([0-9.]+)", dynamic)
    cxx = re.findall(r"GLIBCXX_([0-9.]+)", dynamic)

    def _highest(versions):
        best = None
        for version in versions:
            key = tuple(int(part) for part in version.split("."))
            if best is None or key > best[0]:
                best = (key, version)
        return best[1] if best else None

    floor["libc"] = _highest(glibc)
    floor["libstdcxx"] = _highest(cxx)
    readelf = _require_tool("readelf")
    note = _run([readelf, "-n", path])
    match = re.search(r"OS:\s+Linux,\s+ABI:\s+([0-9.]+)", note)
    if match:
        floor["kernel"] = match.group(1)
    return floor


def _native_bounds(tool, spec):
    if tool == "buildifier":
        linkage = "static"
    else:
        linkage = "dynamic"
    return linkage, None, [], {"kernel": None, "libc": None, "libstdcxx": None}


def _starlark(value, indent=4):
    pad = " " * indent
    if value is None:
        return "None"
    if isinstance(value, bool):
        return "True" if value else "False"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return '"%s"' % value.replace("\\", "\\\\").replace('"', '\\"')
    if isinstance(value, list):
        if not value:
            return "[]"
        items = [pad + _starlark(item, indent + 4) for item in value]
        return "[\n%s,\n%s]" % (",\n".join(items), " " * (indent - 4))
    if isinstance(value, dict):
        if not value:
            return "{}"
        items = [
            "%s%s: %s" % (pad, _starlark(key), _starlark(item, indent + 4))
            for key, item in sorted(value.items())
        ]
        return "{\n%s,\n%s}" % (",\n".join(items), " " * (indent - 4))
    raise TypeError("unsupported metadata value: %r" % (value,))


def _source_dir():
    workspace = os.environ.get("BUILD_WORKSPACE_DIRECTORY")
    if workspace:
        return os.path.join(workspace, "quality", "artifacts")
    return os.path.dirname(os.path.abspath(__file__))


def _collect(tool, platform_key, spec, workdir):
    asset_path = os.path.join(workdir, spec["asset"])
    _download(spec["url"], asset_path)
    size = os.path.getsize(asset_path)
    digest = _sha256(asset_path)
    checksum_source = (
        "maintainer-established byte identity (upstream publishes no asset digests)"
    )
    if "checksums_url" in spec:
        checksums_path = os.path.join(workdir, "checksums.txt")
        _download(spec["checksums_url"], checksums_path)
        published = {}
        with open(checksums_path, encoding="utf-8") as handle:
            for line in handle.read().splitlines():
                parts = line.split()
                if len(parts) == 2:
                    published[parts[1].lstrip("*")] = parts[0]
        expected = published.get(spec["asset"])
        if expected != digest:
            sys.exit(
                "update: %s digest %s does not match published %s"
                % (spec["asset"], digest, expected)
            )
        checksum_source = "upstream published checksums file"

    kind = spec["kind"]
    if kind == "raw":
        archive = {"format": "none"}
        exe_path, exe_digest = asset_path, digest
    elif kind == "gzip":
        with gzip.open(asset_path, "rb") as handle:
            inner = handle.read()
        member_path = os.path.join(workdir, spec["executable"])
        with open(member_path, "wb") as handle:
            handle.write(inner)
        exe_path, exe_digest = member_path, hashlib.sha256(inner).hexdigest()
        archive = {
            "format": "gzip",
            "member": spec["executable"],
            "member_sha256": exe_digest,
            "member_size": len(inner),
        }
    elif kind == "tar.gz":
        members = []
        with tarfile.open(asset_path, "r:gz") as archive_file:
            archive_file.extractall(workdir)
            for member in archive_file.getmembers():
                members.append(
                    {
                        "name": member.name,
                        "size": member.size,
                        "mode": oct(member.mode),
                        "is_executable": bool(member.mode & 0o111) and member.isfile(),
                    }
                )
        exe_path = os.path.join(workdir, spec["executable"])
        exe_digest = _sha256(exe_path)
        archive = {"format": "tar.gz", "members": members}
    elif kind == "zip":
        members = []
        with zipfile.ZipFile(asset_path) as archive_file:
            archive_file.extractall(workdir)
            for info in archive_file.infolist():
                mode = (info.external_attr >> 16) & 0o777
                is_dir = info.is_dir()
                is_exe = (not is_dir) and (
                    info.filename.endswith(".exe") or bool(mode & 0o111)
                )
                members.append(
                    {
                        "name": info.filename,
                        "size": info.file_size,
                        "mode": oct(mode),
                        "is_executable": is_exe,
                    }
                )
        exe_path = os.path.join(workdir, spec["executable"])
        exe_digest = _sha256(exe_path)
        archive = {"format": "zip", "members": members}
    else:
        sys.exit("update: unknown asset kind %r" % kind)

    if spec["os"] == "linux":
        linkage, interpreter, needed = _elf_linkage(exe_path)
        if linkage == "pie":
            linkage = "static-pie"
        abi_floor = _abi_floor(exe_path)
    else:
        linkage, interpreter, needed, abi_floor = _native_bounds(tool, spec)
    return {
        "schema_version": SCHEMA_VERSION,
        "tool": tool,
        "upstream_version": TOOLS[tool]["upstream_version"],
        "delivery_class": "standalone",
        "os": spec["os"],
        "cpu": spec["cpu"],
        "url": spec["url"],
        "sha256": digest,
        "size": size,
        "checksum_source": checksum_source,
        "archive": archive,
        "executable": spec["executable"],
        "executable_sha256": exe_digest,
        "linkage": linkage,
        "interpreter": interpreter,
        "needed_shared_libraries": needed,
        "abi_floor": abi_floor,
        "runtime_files": [],
        "licenses": TOOLS[tool]["licenses"],
    }


def _emit(artifact, tool, platform_key):
    filename = "%s.%s.bzl" % (tool, platform_key)
    path = os.path.join(_source_dir(), filename)
    content = (
        '"""%s standalone artifact metadata (%s) -- GENERATED, do not edit.\n'
        "\n"
        "Regenerate with: bazel run //quality/artifacts:update\n"
        '"""\n'
        "\n"
        "# buildifier: disable=attr-licenses\n"
        "ARTIFACT = %s\n" % (tool, platform_key, _starlark(artifact))
    )
    return path, content


def main(argv):
    verify_only = argv == ["--verify-only"]
    if argv and not verify_only:
        sys.exit("usage: update [--verify-only]")
    _require_tool("file")
    if not os.path.isdir(_source_dir()):
        sys.exit("update: source directory %r not found" % _source_dir())
    failed = False
    for tool, config in TOOLS.items():
        for platform_key, spec in config["platforms"].items():
            with tempfile.TemporaryDirectory(prefix="dx-artifacts-") as workdir:
                artifact = _collect(tool, platform_key, spec, workdir)
            path, content = _emit(artifact, tool, platform_key)
            if verify_only:
                with open(path, encoding="utf-8") as handle:
                    if handle.read() != content:
                        print("update: %s is stale or upstream bytes changed" % path)
                        failed = True
                    else:
                        print("update: %s verified" % os.path.basename(path))
            else:
                with open(path, "w", encoding="utf-8") as handle:
                    handle.write(content)
                print(
                    "update: wrote %s (%d bytes, sha256 %s...)"
                    % (path, artifact["size"], artifact["sha256"][:16])
                )
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main(sys.argv[1:])
