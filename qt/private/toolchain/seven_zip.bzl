"""Pinned 7-Zip extractors that unpack the Qt SDK .7z archives."""

_OSES = {"linux": "linux", "mac os x": "macos"}

_ARCHES = {"aarch64": "arm64", "amd64": "x86_64", "arm64": "arm64", "x86_64": "x86_64"}

def seven_zip_extractors(pins):
    """Returns the pinned 7-Zip extractor of every host cell in a decoded qt/pins.json."""
    return {
        host: struct(archive = entry["archive"], binary = entry["binary"], sha256 = entry["sha256"], url = entry["urls"][0])
        for host, entry in pins["seven_zip"].items()
    }

def host_cell_of(os_name, arch):
    """Returns `linux|macos|windows` + `_` + `x86_64|arm64` from Bazel's os name and arch."""
    name = os_name.lower()
    name = "windows" if name.startswith("windows") else _OSES.get(name, name)
    return name + "_" + _ARCHES.get(arch, arch)

def seven_zip_fetch(rctx, output):
    """Downloads the pinned extractor of the repository host into output and returns its path."""
    extractors = seven_zip_extractors(json.decode(rctx.read(Label("//qt:pins.json"))))
    host = host_cell_of(rctx.os.name, rctx.os.arch)
    if host not in extractors:
        fail("the pinned 7-Zip extractor runs on " + ", ".join(sorted(extractors.keys())) + " hosts only, not " + rctx.os.name + " " + rctx.os.arch)
    extractor = extractors[host]
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
