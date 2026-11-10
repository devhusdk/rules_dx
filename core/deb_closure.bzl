"""Repository-rule helper that unpacks pinned .deb shared libraries into a closure directory."""

def deb_closure(rctx, mirror, packages, closure):
    """Extracts each pinned .deb's libraries and copyright into `closure`/{lib,licenses}."""
    for package in packages:
        scratch = "_debs/" + package.package
        rctx.download_and_extract(
            url = mirror + "/" + package.deb,
            sha256 = package.sha256,
            output = scratch,
            type = "deb",
        )
        data = [entry for entry in rctx.path(scratch).readdir() if entry.basename.startswith("data.tar")]
        if len(data) != 1:
            fail("{} has no single data archive".format(package.deb))
        rctx.extract(data[0], output = scratch + "/data")
        for soname, path in package.libraries.items():
            source = rctx.path(scratch + "/data/" + path)
            if not source.exists:
                fail("{} does not ship {}".format(package.deb, path))
            rctx.rename(source.realpath, closure + "/lib/" + soname)
        if package.copyright != None:
            copyright = rctx.path(scratch + "/data/" + package.copyright)
            if not copyright.exists:
                fail("{} does not ship {}".format(package.deb, package.copyright))
            rctx.file(closure + "/licenses/" + package.package, rctx.read(copyright), executable = False)
    rctx.delete("_debs")
