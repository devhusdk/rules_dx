"""Repository rule that restores one Bicep registry module and pins it by digest."""

load("//core/quality/artifacts:bicep.bzl", "ARTIFACTS")

_HEX = "0123456789abcdef"

def registry_errors(ref, digest):
    """Returns the errors in a registry module declaration, ref and digest."""
    errors = []
    if not ref.startswith("br:"):
        errors.append("ref %r must start with br:" % ref)
    elif ":" not in ref.rpartition("/")[2] or ref.endswith(":"):
        errors.append("ref %r must end in :<tag>" % ref)
    hex = digest[len("sha256:"):]
    if not digest.startswith("sha256:") or len(hex) != 64 or any([c not in _HEX for c in hex.elems()]):
        errors.append("digest %r must be sha256: plus 64 lowercase hex characters" % digest)
    return errors

def _platform(rctx):
    os = rctx.os.name.lower()
    arch = rctx.os.arch
    if arch == "amd64":
        arch = "x86_64"
    if arch == "aarch64":
        arch = "arm64"
    if os.startswith("windows"):
        os = "windows"
    elif os.startswith("mac"):
        os = "macos"
    key = "%s_%s" % (os, arch)
    if key not in ARTIFACTS:
        fail("bicep registry module: no Bicep CLI for host %s" % key)
    return key

def _sha256(rctx, path):
    if rctx.os.name.lower().startswith("windows"):
        result = rctx.execute(["certutil", "-hashfile", str(path), "SHA256"])
        if result.return_code == 0:
            return result.stdout.splitlines()[1].strip().replace(" ", "").lower()
    else:
        for cmd in (["sha256sum", str(path)], ["shasum", "-a", "256", str(path)]):
            result = rctx.execute(cmd)
            if result.return_code == 0:
                return result.stdout.split(" ")[0].strip()
    fail("bicep registry module: cannot hash %s: %s" % (path, result.stderr))

def _find_manifests(root):
    found = []
    pending = [root] if root.exists else []
    for _ in range(1000):
        if not pending:
            break
        for child in pending.pop().readdir():
            if child.is_dir:
                pending.append(child)
            elif child.basename == "manifest":
                found.append(child)
    return found

def _impl(rctx):
    errors = registry_errors(rctx.attr.ref, rctx.attr.digest)
    if errors:
        fail("bicep registry module: " + "; ".join(errors))
    key = _platform(rctx)
    exe = rctx.path(rctx.attr.clis[key])
    rctx.file("restore.bicep", "module m '%s' = {\n  name: 'm'\n}\n" % rctx.attr.ref)
    home = str(rctx.path("home"))
    rctx.file("bicepconfig.json", json.encode({"cacheRootDirectory": home + "/.bicep"}) + "\n")
    result = rctx.execute(
        [str(exe), "restore", "restore.bicep"],
        environment = {
            "DOTNET_BUNDLE_EXTRACT_BASE_DIR": home + "/dotnet",
            "HOME": home,
            "USERPROFILE": home,
        },
    )
    if result.return_code != 0:
        fail("bicep restore %s failed:\n%s%s" % (rctx.attr.ref, result.stdout, result.stderr))
    paths = _find_manifests(rctx.path("home/.bicep/br"))
    if len(paths) != 1:
        fail("bicep restore %s: expected one cached manifest, found %d" % (rctx.attr.ref, len(paths)))
    actual = "sha256:" + _sha256(rctx, paths[0])
    if actual != rctx.attr.digest:
        fail("bicep registry module %s: manifest digest mismatch\n  expected %s\n  actual   %s" % (rctx.attr.ref, rctx.attr.digest, actual))
    rctx.delete("home/dotnet")
    rctx.file("BUILD.bazel", """load("@rules_dx//bicep:defs.bzl", "bicep_registry_module")

bicep_registry_module(
    name = "module",
    srcs = glob(["home/.bicep/br/**"]),
    cache_root = "home/.bicep",
    digest = {digest},
    ref = {ref},
    visibility = ["//visibility:public"],
)
""".format(digest = repr(rctx.attr.digest), ref = repr(rctx.attr.ref)))
    return rctx.repo_metadata(reproducible = True)

bicep_registry_repo = repository_rule(
    implementation = _impl,
    doc = "Restores one Bicep registry module and checks its manifest digest.",
    attrs = {
        "clis": attr.string_keyed_label_dict(mandatory = True, doc = "Host key to the Bicep CLI repository file."),
        "digest": attr.string(mandatory = True, doc = "Expected sha256 of the restored manifest."),
        "ref": attr.string(mandatory = True, doc = "Module reference, br:HOST/PATH:TAG."),
    },
)
