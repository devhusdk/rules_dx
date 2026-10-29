"""Pinned 7-Zip extractors shared by the repository rules that unpack .7z archives.

Bazel's native .7z support cannot decode BCJ2, so archives that use it need a real 7-Zip.
"""

_RELEASE = "https://github.com/ip7z/7zip/releases/download/26.03/"

_WINDOWS = struct(
    archive = None,
    binary = "7zr.exe",
    sha256 = "ad4c82fadcbdf93c03b4fc440f300509c7d60c5c2f4d183e35d9d70d6957037d",
    url = _RELEASE + "7zr.exe",
)

SEVEN_ZIP_EXTRACTORS = {
    "linux_arm64": struct(
        archive = "7z2603-linux-arm64.tar.xz",
        binary = "7zzs",
        sha256 = "2389ba20e4d8295e8709c20b6263b69bd1ec4972fe38a04ad7a1badbf595b996",
        url = _RELEASE + "7z2603-linux-arm64.tar.xz",
    ),
    "linux_x86_64": struct(
        archive = "7z2603-linux-x64.tar.xz",
        binary = "7zzs",
        sha256 = "dc99eff5008f1ab79bd7084c68513701547a808a89502bf4133683535ab3c695",
        url = _RELEASE + "7z2603-linux-x64.tar.xz",
    ),
    "macos_arm64": struct(
        archive = "7z2603-mac.tar.xz",
        binary = "7zz",
        sha256 = "5ca87677072c59f5602e5c49baa27d4694bacd2259b4e507f0094249d4281480",
        url = _RELEASE + "7z2603-mac.tar.xz",
    ),
    # 7zr.exe is x86, so it also runs on Windows arm64 under emulation.
    "windows_arm64": _WINDOWS,
    "windows_x86_64": _WINDOWS,
}

_HOST_OSES = {"linux": "linux", "mac os x": "macos"}

_HOST_ARCHES = {"aarch64": "arm64", "amd64": "x86_64", "arm64": "arm64", "x86_64": "x86_64"}

def seven_zip_host(os_name, arch):
    """Returns the SEVEN_ZIP_EXTRACTORS key of one host from Bazel's os name and arch."""
    name = os_name.lower()
    name = "windows" if name.startswith("windows") else _HOST_OSES.get(name, name)
    return name + "_" + _HOST_ARCHES.get(arch, arch)

def seven_zip_fetch(rctx, output):
    """Downloads the pinned extractor of the repository host into output and returns its path."""
    host = seven_zip_host(rctx.os.name, rctx.os.arch)
    if host not in SEVEN_ZIP_EXTRACTORS:
        fail("the pinned 7-Zip extractor runs on " + ", ".join(sorted(SEVEN_ZIP_EXTRACTORS.keys())) + " hosts only, not " + rctx.os.name + " " + rctx.os.arch)
    extractor = SEVEN_ZIP_EXTRACTORS[host]
    canonical_id = "dx-7zip:" + extractor.url
    if extractor.archive == None:
        rctx.download(
            url = extractor.url,
            output = output + "/" + extractor.binary,
            sha256 = extractor.sha256,
            executable = True,
            canonical_id = canonical_id,
        )
    else:
        rctx.download_and_extract(
            url = extractor.url,
            output = output,
            sha256 = extractor.sha256,
            canonical_id = canonical_id,
        )
    return str(rctx.path(output + "/" + extractor.binary))

def seven_zip_extract(rctx, binary, archive, output, args = []):
    """Extracts one .7z archive into output with the fetched extractor, failing with its output."""
    result = rctx.execute([binary, "x", "-y", "-bd", "-o" + output] + args + [str(archive)])
    if result.return_code != 0:
        fail("cannot extract '" + str(archive) + "': " + result.stdout + result.stderr)
