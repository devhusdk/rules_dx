#!/usr/bin/env python3
"""Regenerate checked-in artifact metadata.

Host tools: `file` (and `zstd` for zstd assets) on PATH. ELF and PE headers are parsed in Python, so no
readelf or objdump is needed (macOS maintainers can run this).
"""

import gzip
import hashlib
import json
import lzma
import os
import pathlib
import re
import shutil
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request
import zipfile
from typing import Any

FETCH_RETRIES = 3
FETCH_TIMEOUT = 300
FETCH_BACKOFF_SECONDS = 2

TLNET = "https://ftp.math.utah.edu/pub/tex/historic/systems/texlive/2025/tlnet-final"
TLPDB_URL = TLNET + "/tlpkg/texlive.tlpdb.xz"


PINS_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "pins.json")
LOCK_NAME = "lock.json"


def expand(value, version):
    """Replaces every {version} in a pins.json value."""
    if isinstance(value, str):
        return value.replace("{version}", version)
    if isinstance(value, list):
        return [expand(item, version) for item in value]
    if isinstance(value, dict):
        return {key: expand(item, version) for key, item in value.items()}
    return value


def load_tools(path=PINS_PATH):
    """Returns the per-tool specs of pins.json with {version} expanded."""
    with open(path, encoding="utf-8") as handle:
        pins = json.load(handle)["tools"]
    tools = {}
    for name, entry in pins.items():
        version = entry["version"]
        spec = {"upstream_version": version}
        for key, value in entry.items():
            if key == "version":
                continue
            spec["platforms" if key == "cells" else key] = expand(value, version)
        tools[name] = spec
    return tools


TOOLS: dict[str, dict[str, Any]] = load_tools()



def _require_tool(name):
    found = shutil.which(name)
    if found is not None:
        return found
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
                written = 0
                for chunk in iter(lambda: response.read(65536), b""):
                    out.write(chunk)
                    written += len(chunk)
                expected = response.headers.get("Content-Length")
            if expected is not None and written != int(expected):
                raise OSError("short body: got %d of %s bytes" % (written, expected))
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


def _check_member_name(asset, name):
    """Stops on any archive member name that could escape the work directory."""
    reason = None
    if not name:
        reason = "empty name"
    elif name.startswith("/") or re.match(r"^[A-Za-z]:", name):
        reason = "absolute path"
    elif "\\" in name:
        reason = "backslash in name"
    elif ".." in pathlib.PurePosixPath(name).parts:
        reason = "parent directory component"
    if reason:
        sys.exit("update: %s member %r: %s" % (asset, name, reason))


def _check_duplicate(asset, name, seen):
    key = os.path.normpath(name)
    if key in seen:
        sys.exit("update: %s member %r: duplicate name" % (asset, name))
    seen.add(key)


def _stream_executable(asset, workdir, executable, source):
    """Copies the executable's bytes to workdir and returns (path, sha256)."""
    exe_path = os.path.join(workdir, executable)
    os.makedirs(os.path.dirname(exe_path), exist_ok=True)
    digest = hashlib.sha256()
    with open(exe_path, "wb") as out:
        for chunk in iter(lambda: source.read(65536), b""):
            digest.update(chunk)
            out.write(chunk)
    return exe_path, digest.hexdigest()


TAR_COMPRESSIONS = {"tar.gz": "gz", "tar.xz": "xz"}


def _tar_members(asset, asset_path, workdir, executable, kind="tar.gz", links="error"):
    members = []
    seen = set()
    exe_member = None
    with tarfile.open(asset_path, "r:" + TAR_COMPRESSIONS[kind]) as archive_file:
        infos = archive_file.getmembers()
        for member in infos:
            _check_member_name(asset, member.name)
            if member.issym() or member.islnk():
                if links != "skip":
                    sys.exit("update: %s member %r: link" % (asset, member.name))
                continue
            if member.isdev() or member.isfifo():
                sys.exit("update: %s member %r: device or fifo" % (asset, member.name))
            if not member.isdir():
                _check_duplicate(asset, member.name, seen)
            members.append(
                {
                    "name": member.name,
                    "size": member.size,
                    "mode": oct(member.mode),
                    "is_executable": bool(member.mode & 0o111) and member.isfile(),
                }
            )
            if os.path.normpath(member.name) == os.path.normpath(executable):
                exe_member = member
        if exe_member is None or not exe_member.isfile():
            sys.exit(
                "update: %s member %r: missing or not a regular file"
                % (asset, executable)
            )
        source = archive_file.extractfile(exe_member)
        exe_path, exe_digest = _stream_executable(asset, workdir, executable, source)
    return members, exe_path, exe_digest


def _zip_members(asset, asset_path, workdir, executable):
    members = []
    seen = set()
    exe_info = None
    with zipfile.ZipFile(asset_path) as archive_file:
        for info in archive_file.infolist():
            _check_member_name(asset, info.filename)
            unix_mode = (info.external_attr >> 16) & 0o177777
            if info.create_system == 3 and stat.S_ISLNK(unix_mode):
                sys.exit("update: %s member %r: symlink" % (asset, info.filename))
            if not info.is_dir():
                _check_duplicate(asset, info.filename, seen)
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
            if os.path.normpath(info.filename) == os.path.normpath(executable):
                exe_info = info
        if exe_info is None or exe_info.is_dir():
            sys.exit(
                "update: %s member %r: missing or not a regular file"
                % (asset, executable)
            )
        with archive_file.open(exe_info) as source:
            exe_path, exe_digest = _stream_executable(
                asset, workdir, executable, source
            )
    return members, exe_path, exe_digest


def _cstr(data, offset):
    return data[offset : data.index(b"\0", offset)].decode("ascii", "replace")


def _read_elf(path):
    """Returns type, interpreter, needed libraries, symbol versions and ABI tag."""
    with open(path, "rb") as handle:
        data = handle.read()
    if data[:4] != b"\x7fELF" or data[4] not in (1, 2) or data[5] not in (1, 2):
        sys.exit("update: %s is not an ELF file" % path)
    wide = data[4] == 2
    end = "<" if data[5] == 1 else ">"
    word = "Q" if wide else "I"
    e_type = struct.unpack_from(end + "H", data, 16)[0]
    phoff = struct.unpack_from(end + word, data, 32 if wide else 28)[0]
    phentsize, phnum = struct.unpack_from(end + "HH", data, 54 if wide else 42)
    segments = []
    for index in range(phnum):
        base = phoff + index * phentsize
        if wide:
            kind, _, offset, vaddr, _, filesz, _, _ = struct.unpack_from(
                end + "IIQQQQQQ", data, base
            )
        else:
            kind, offset, vaddr, _, filesz, _, _, _ = struct.unpack_from(
                end + "IIIIIIII", data, base
            )
        segments.append((kind, offset, vaddr, filesz))

    def file_offset(vaddr):
        for kind, offset, start, filesz in segments:
            if kind == 1 and start <= vaddr < start + filesz:
                return offset + vaddr - start
        sys.exit("update: %s has an address outside its load segments" % path)

    interpreter = None
    dynamic = []
    abi = None
    for kind, offset, _, filesz in segments:
        if kind == 3:
            interpreter = _cstr(data, offset)
        elif kind == 2:
            size = struct.calcsize(end + word)
            for pos in range(offset, offset + filesz, 2 * size):
                tag, value = struct.unpack_from(end + word + word, data, pos)
                if tag == 0:
                    break
                dynamic.append((tag, value))
        elif kind == 4:
            pos, stop = offset, offset + filesz
            while pos + 12 <= stop:
                namesz, descsz, note_type = struct.unpack_from(end + "III", data, pos)
                name = data[pos + 12 : pos + 12 + namesz].rstrip(b"\0")
                desc = pos + 12 + (namesz + 3) // 4 * 4
                if name == b"GNU" and note_type == 1 and descsz >= 16:
                    system, *version = struct.unpack_from(end + "IIII", data, desc)
                    if system == 0:
                        abi = ".".join(str(part) for part in version[:3])
                pos = desc + (descsz + 3) // 4 * 4

    tags: dict[int, int] = {}
    needed_offsets = []
    for tag, value in dynamic:
        if tag == 1:
            needed_offsets.append(value)
        tags.setdefault(tag, value)
    needed = []
    versions = []
    if 5 in tags:
        strtab = file_offset(tags[5])
        needed = [_cstr(data, strtab + value) for value in needed_offsets]
        for table, count, header, aux_at, aux_header in (
            (0x6FFFFFFE, 0x6FFFFFFF, "HHIII", 3, "IHHII"),
            (0x6FFFFFFC, 0x6FFFFFFD, "HHHHIII", 5, "II"),
        ):
            if table not in tags:
                continue
            pos = file_offset(tags[table])
            for _ in range(tags.get(count, 0)):
                fields = struct.unpack_from(end + header, data, pos)
                aux = pos + fields[aux_at]
                for _ in range(fields[1] if header == "HHIII" else fields[3]):
                    values = struct.unpack_from(end + aux_header, data, aux)
                    versions.append(_cstr(data, strtab + values[-2]))
                    if not values[-1]:
                        break
                    aux += values[-1]
                if not fields[-1]:
                    break
                pos += fields[-1]
    return e_type, interpreter, needed, versions, abi


def _elf_linkage(path):
    e_type, interpreter, needed, _, _ = _read_elf(path)
    if interpreter:
        linkage = "dynamic"
    elif e_type == 3:
        linkage = "pie"
    else:
        linkage = "static"
    return linkage, interpreter, sorted(set(needed))


def _abi_floor(path):
    floor = {"kernel": None, "libc": None, "libstdcxx": None}
    linkage, _, _ = _elf_linkage(path)
    if linkage == "static":
        return floor
    _, _, _, versions, abi = _read_elf(path)

    def _highest(prefix):
        best = None
        for version in versions:
            if not version.startswith(prefix):
                continue
            text = version[len(prefix) :]
            if not re.fullmatch(r"[0-9]+(\.[0-9]+)*", text):
                continue
            key = tuple(int(part) for part in text.split("."))
            if best is None or key > best[0]:
                best = (key, text)
        return best[1] if best else None

    floor["libc"] = _highest("GLIBC_")
    floor["libstdcxx"] = _highest("GLIBCXX_")
    floor["kernel"] = abi
    return floor


def _native_bounds(tool, spec):
    if tool in (
        "buf",
        "buildifier",
        "helm",
        "keep_sorted",
        "kubeconform",
        "lychee",
        "shfmt",
        "terraform",
        "typos",
        "xmllint",
    ):
        linkage = "static"
    else:
        linkage = "dynamic"
    return linkage, None, [], {"kernel": None, "libc": None, "libstdcxx": None}


PE_MACHINES = {"x86_64": 0x8664, "arm64": 0xAA64}


def _pe_machine(path):
    """Returns the COFF machine field of a PE file, or exits if it is not a PE."""
    with open(path, "rb") as handle:
        head = handle.read(0x40)
        if len(head) < 0x40 or head[:2] != b"MZ":
            sys.exit("update: %s is not a PE file (no MZ header)" % path)
        offset = int.from_bytes(head[0x3C:0x40], "little")
        handle.seek(offset)
        coff = handle.read(6)
    if len(coff) < 6 or coff[:4] != b"PE\0\0":
        sys.exit("update: %s is not a PE file (no PE signature)" % path)
    return int.from_bytes(coff[4:6], "little")


def _check_pe_machine(asset, spec, exe_path):
    expected = PE_MACHINES.get(spec["cpu"])
    if expected is None:
        sys.exit(
            "update: %s: no PE machine type known for cpu %r" % (asset, spec["cpu"])
        )
    machine = _pe_machine(exe_path)
    if machine != expected:
        sys.exit(
            "update: %s: PE machine 0x%04X, expected 0x%04X for %s"
            % (asset, machine, expected, spec["cpu"])
        )


def _starlark(value, indent=4):
    pad = " " * indent
    if value is None:
        return "None"
    if isinstance(value, bool):
        return "True" if value else "False"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        escaped = value.replace("\\", "\\\\").replace('"', '\\"')
        return '"%s"' % escaped.replace("\n", "\\n")
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
        return os.path.join(workspace, "core", "quality", "artifacts")
    return os.path.dirname(os.path.abspath(__file__))


PYPI_JSON_URL = "https://pypi.org/pypi/%s/%s/json"


def _pypi_digest_error(metadata, spec, digest):
    """Checks a pinned wheel against the PyPI JSON entry for its exact version."""
    wanted = spec["pypi"]["filename"]
    for entry in metadata.get("urls", []):
        if entry.get("filename") != wanted:
            continue
        if entry.get("url") != spec["url"]:
            return "update: %s url %r does not match PyPI %r" % (
                wanted,
                spec["url"],
                entry.get("url"),
            )
        published = entry.get("digests", {}).get("sha256")
        if published != digest:
            return "update: %s digest %s does not match PyPI %s" % (
                wanted,
                digest,
                published,
            )
        return None
    return "update: %s is not published for this version on PyPI" % wanted


_TLPDB_CHECKSUMS: dict[str, str] = {}


def _tlpdb_checksum(package, workdir):
    """Returns the sha512 TeX Live publishes for one package archive."""
    if not _TLPDB_CHECKSUMS:
        path = os.path.join(workdir, "texlive.tlpdb.xz")
        _download(TLPDB_URL, path)
        with lzma.open(path, "rt", encoding="utf-8") as handle:
            text = handle.read()
        for block in text.split("\n\n"):
            name = checksum = None
            for line in block.splitlines():
                if line.startswith("name "):
                    name = line[5:].strip()
                elif line.startswith("containerchecksum "):
                    checksum = line[len("containerchecksum ") :].strip()
            if name and checksum:
                _TLPDB_CHECKSUMS[name] = checksum
    return _TLPDB_CHECKSUMS.get(package)


def _check_tlpdb(asset, package, path, workdir):
    digest = hashlib.sha512()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    expected = _tlpdb_checksum(package, workdir)
    if expected != digest.hexdigest():
        sys.exit(
            "update: %s sha512 %s does not match texlive.tlpdb %s"
            % (asset, digest.hexdigest(), expected)
        )


def _zip_runtime_file(entry, workdir):
    """Returns the runtime file taken from a zip the tool itself ships in."""
    archive_path = os.path.join(workdir, entry["asset"])
    if not os.path.exists(archive_path):
        _download(entry["url"], archive_path)
    with zipfile.ZipFile(archive_path) as archive_file:
        member_digest = hashlib.sha256(archive_file.read(entry["member"])).hexdigest()
    return {
        "path": entry["path"],
        "url": entry["url"],
        "sha256": _sha256(archive_path),
        "format": "zip",
        "member": entry["member"],
        "member_sha256": member_digest,
    }


def _runtime_files(spec, workdir):
    """Returns the extra files one tool needs next to its executable."""
    files = []
    for entry in spec.get("runtime", []):
        if "content" in entry:
            files.append(
                {
                    "path": entry["path"],
                    "content": entry["content"],
                    "sha256": hashlib.sha256(entry["content"].encode()).hexdigest(),
                }
            )
            continue
        if entry["kind"] == "zip":
            files.append(_zip_runtime_file(entry, workdir))
            continue
        archive_path = os.path.join(workdir, "runtime-" + entry["asset"])
        if not os.path.exists(archive_path):
            _download(entry["url"], archive_path)
        _check_tlpdb(entry["asset"], entry["tlpdb"], archive_path, workdir)
        member_digest = None
        with tarfile.open(archive_path, "r:xz") as archive_file:
            for member in archive_file.getmembers():
                _check_member_name(entry["asset"], member.name)
                if member.name != entry["member"]:
                    continue
                if not member.isfile():
                    sys.exit(
                        "update: %s member %r: not a regular file"
                        % (entry["asset"], member.name)
                    )
                source = archive_file.extractfile(member)
                assert source is not None
                member_digest = hashlib.sha256(source.read()).hexdigest()
        if member_digest is None:
            sys.exit(
                "update: %s member %r: missing" % (entry["asset"], entry["member"])
            )
        files.append(
            {
                "path": entry["path"],
                "url": entry["url"],
                "sha256": _sha256(archive_path),
                "format": entry["kind"],
                "member": entry["member"],
                "member_sha256": member_digest,
            }
        )
    return files


RELEASE_BUILD_ORIGIN = "rules_dx"
RELEASE_BUILD_MANIFEST = "/dx-tool-sha256sums.txt"


def _release_build_error(spec):
    """Returns why a rules_dx release-built spec is malformed, or None."""
    if spec.get("origin") != RELEASE_BUILD_ORIGIN:
        return None
    url = spec.get("checksums_url", "")
    if not url.endswith(RELEASE_BUILD_MANIFEST):
        return "update: %s is release built and needs a checksums_url ending in %s" % (
            spec.get("asset"),
            RELEASE_BUILD_MANIFEST,
        )
    return None


def _collect(tool, platform_key, spec, workdir):
    error = _release_build_error(spec)
    if error:
        sys.exit(error)
    asset_path = os.path.join(workdir, spec["asset"])
    _download(spec["url"], asset_path)
    size = os.path.getsize(asset_path)
    digest = _sha256(asset_path)
    if "tlpdb" in spec:
        _check_tlpdb(spec["asset"], spec["tlpdb"], asset_path, workdir)
    checksum_source = (
        "maintainer-established byte identity (upstream publishes no asset digests)"
    )
    if "tlpdb" in spec:
        checksum_source = "texlive.tlpdb containerchecksum (sha512)"
    if "checksums_url" in spec:
        checksums_path = os.path.join(workdir, "checksums.txt")
        _download(spec["checksums_url"], checksums_path)
        published = {}
        with open(checksums_path, encoding="utf-8") as handle:
            for line in handle.read().splitlines():
                parts = line.split()
                if len(parts) == 4 and parts[1].startswith("(") and parts[2] == "=":
                    # BSD style: SHA256 (name) = digest
                    published[parts[1].strip("()")] = parts[3]
                elif len(parts) == 2:
                    published[os.path.basename(parts[1].lstrip("*"))] = parts[0]
        expected = published.get(spec["asset"])
        if expected != digest:
            sys.exit(
                "update: %s digest %s does not match published %s"
                % (spec["asset"], digest, expected)
            )
        checksum_source = "upstream published checksums file"
        if spec.get("origin") == RELEASE_BUILD_ORIGIN:
            checksum_source = "rules_dx release build"
    if "pypi" in spec:
        json_path = os.path.join(workdir, "pypi.json")
        _download(
            PYPI_JSON_URL % (spec["pypi"]["project"], TOOLS[tool]["upstream_version"]),
            json_path,
        )
        with open(json_path, encoding="utf-8") as handle:
            error = _pypi_digest_error(json.load(handle), spec, digest)
        if error:
            sys.exit(error)
        checksum_source = "PyPI JSON API sha256 digest"

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
    elif kind == "zstd":
        inner_path = os.path.join(workdir, "decompressed")
        _run([_require_tool("zstd"), "-d", "-q", "-f", "-o", inner_path, asset_path])
        with open(inner_path, "rb") as handle:
            inner = handle.read()
        member_path = os.path.join(workdir, spec["executable"])
        with open(member_path, "wb") as handle:
            handle.write(inner)
        exe_path, exe_digest = member_path, hashlib.sha256(inner).hexdigest()
        archive = {
            "format": "zstd",
            "member": spec["executable"],
            "member_sha256": exe_digest,
            "member_size": len(inner),
        }
    elif kind in TAR_COMPRESSIONS:
        members, exe_path, exe_digest = _tar_members(
            spec["asset"],
            asset_path,
            workdir,
            spec["executable"],
            kind,
            spec.get("links", "error"),
        )
        archive = {"format": kind, "members": members}
    elif kind == "zip":
        members, exe_path, exe_digest = _zip_members(
            spec["asset"], asset_path, workdir, spec["executable"]
        )
        archive = {"format": "zip", "members": members}
    else:
        sys.exit("update: unknown asset kind %r" % kind)

    if spec["os"] == "windows":
        _check_pe_machine(spec["asset"], spec, exe_path)

    if spec["os"] == "linux":
        linkage, interpreter, needed = _elf_linkage(exe_path)
        if linkage == "pie":
            linkage = "static-pie"
        abi_floor = _abi_floor(exe_path)
    else:
        linkage, interpreter, needed, abi_floor = _native_bounds(tool, spec)
    return {
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
        "runtime_files": _runtime_files(spec, workdir),
        "licenses": TOOLS[tool]["licenses"],
    }


HADOLINT_CABAL_URL = (
    "https://raw.githubusercontent.com/hadolint/hadolint/v%s/hadolint.cabal"
)


def _version_tuple(text):
    return tuple(int(part) for part in text.split("."))


def _shellcheck_range(cabal_text):
    """Returns the (lower, upper) ShellCheck bounds hadolint.cabal declares."""
    match = re.search(r"\bShellCheck\s*>=\s*([0-9.]+)\s*&&\s*<\s*([0-9.]+)", cabal_text)
    if not match:
        return None
    return match.group(1), match.group(2)


def _hadolint_shellcheck_error(cabal_text, hadolint_version, shellcheck_version):
    """Checks the declared cabal range, not the ShellCheck hadolint actually linked."""
    bounds = _shellcheck_range(cabal_text)
    if bounds is None:
        return (
            "update: hadolint %s: no `ShellCheck >=X && <Y` range in hadolint.cabal"
            % hadolint_version
        )
    lower, upper = bounds
    pinned = _version_tuple(shellcheck_version)
    if _version_tuple(lower) <= pinned < _version_tuple(upper):
        return None
    return (
        "update: hadolint %s declares ShellCheck >=%s && <%s in hadolint.cabal, "
        "but shellcheck is pinned at %s; move the two pins together (this checks "
        "the cabal range, not the ShellCheck version linked into the binary)"
        % (hadolint_version, lower, upper, shellcheck_version)
    )


def _check_hadolint_shellcheck(workdir):
    version = TOOLS["hadolint"]["upstream_version"]
    path = os.path.join(workdir, "hadolint.cabal")
    _download(HADOLINT_CABAL_URL % version, path)
    with open(path, encoding="utf-8") as handle:
        error = _hadolint_shellcheck_error(
            handle.read(), version, TOOLS["shellcheck"]["upstream_version"]
        )
    if error:
        sys.exit(error)


def _lock_content(lock):
    return json.dumps(lock, indent=1, sort_keys=True, ensure_ascii=False) + "\n"


def _parse_args(argv):
    """Returns (tool or None, verify_only) for `[--verify-only] [--tool NAME]`."""
    usage = "usage: update [--verify-only] [--tool NAME]"
    tool = None
    verify_only = False
    args = list(argv)
    while args:
        arg = args.pop(0)
        if arg == "--verify-only":
            verify_only = True
        elif arg == "--tool" and args:
            tool = args.pop(0)
        else:
            sys.exit(usage)
    if tool is not None and tool not in TOOLS:
        sys.exit("update: unknown tool %r" % tool)
    return tool, verify_only


def main(argv):
    only_tool, verify_only = _parse_args(argv)
    if not os.path.isdir(_source_dir()):
        sys.exit("update: source directory %r not found" % _source_dir())
    if only_tool in (None, "hadolint", "shellcheck"):
        with tempfile.TemporaryDirectory(prefix="dx-artifacts-") as workdir:
            _check_hadolint_shellcheck(workdir)
    path = os.path.join(_source_dir(), LOCK_NAME)
    lock = {}
    if only_tool and os.path.exists(path):
        with open(path, encoding="utf-8") as handle:
            lock = json.load(handle)
    for tool, config in TOOLS.items():
        if only_tool and tool != only_tool:
            continue
        artifacts = {}
        for platform_key, spec in config["platforms"].items():
            with tempfile.TemporaryDirectory(prefix="dx-artifacts-") as workdir:
                artifacts[platform_key] = _collect(tool, platform_key, spec, workdir)
        lock[tool] = artifacts
    content = _lock_content(lock)
    if verify_only:
        if not os.path.exists(path):
            sys.exit("update: %s is missing" % path)
        with open(path, encoding="utf-8") as handle:
            if handle.read() != content:
                sys.exit("update: %s is stale or upstream bytes changed" % path)
        print("update: %s verified" % LOCK_NAME)
    else:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
        print("update: wrote %s (%d tools)" % (path, len(lock)))


if __name__ == "__main__":
    main(sys.argv[1:])
