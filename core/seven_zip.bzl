"""Pinned 7-Zip extractors shared by the repository rules that unpack .7z archives."""

load(":host.bzl", "host_cell")

_RELEASE = "https://github.com/ip7z/7zip/releases/download/26.06/"

_WINDOWS = struct(
    archive = None,
    binary = "7zr.exe",
    sha256 = "e80db2dad715a6c5adabe292d31ed54e11f4e9b99ca349827b50573b175891bc",
    url = _RELEASE + "7zr.exe",
)

SEVEN_ZIP_EXTRACTORS = {
    "linux_arm64": struct(
        archive = "7z2606-linux-arm64.tar.xz",
        binary = "7zzs",
        sha256 = "edcc408aacce77da5223728f2eceb5e9f6dc713b8a9df934ad21e2a7f7560627",
        url = _RELEASE + "7z2606-linux-arm64.tar.xz",
    ),
    "linux_x86_64": struct(
        archive = "7z2606-linux-x64.tar.xz",
        binary = "7zzs",
        sha256 = "2b539cf067b2ad48e1baa5093dbaa8818b73542d1222a0ffde9dc5f2922ea62e",
        url = _RELEASE + "7z2606-linux-x64.tar.xz",
    ),
    "macos_arm64": struct(
        archive = "7z2606-mac.tar.xz",
        binary = "7zz",
        sha256 = "b6f17920b4240940461dac8ef4009891881ac44d2a5ead625ede2ec8472185dd",
        url = _RELEASE + "7z2606-mac.tar.xz",
    ),
    "windows_arm64": _WINDOWS,
    "windows_x86_64": _WINDOWS,
}

def seven_zip_fetch(rctx, output):
    """Downloads the pinned extractor of the repository host into output and returns its path."""
    host = host_cell(rctx)
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
