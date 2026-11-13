"""The bicep hub: one repository whose packages are the locked Bicep registry modules."""

load("//core:pins.bzl", "lock_problems")

_HEX = "0123456789abcdef"

_MANIFEST = "application/vnd.oci.image.manifest.v1+json"

_LAYER = "application/vnd.ms.bicep.module.layer.v1+json"

def is_digest(digest):
    """Returns whether a value is sha256: plus 64 lowercase hex characters."""
    hex = digest[len("sha256:"):]
    return digest.startswith("sha256:") and len(hex) == 64 and not hex.lstrip(_HEX)

def _tag_directory(tag):
    low = 0
    high = 0
    for index, char in enumerate(tag.elems()):
        if char.isupper():
            if index < 64:
                low |= 1 << index
            else:
                high |= 1 << (index - 64)
    if high:
        return "%s$%x%s" % (tag, high, ("0" * 16 + "%x" % low)[-16:])
    return "%s$%s" % (tag, "%x" % low if low else "")

def _split(ref):
    name, _, tag = ref[len("br:"):].rpartition(":")
    registry, _, repository = name.partition("/")
    return registry, repository, tag

def cache_directory(ref):
    """Returns the directory under the cache root where the Bicep CLI reads a module."""
    registry, repository, tag = _split(ref)
    return "br/%s/%s/%s" % (registry.lower().replace(":", "$"), repository.replace("/", "$"), _tag_directory(tag))

def manifest_url(ref, digest):
    """Returns the registry URL of a module manifest."""
    registry, repository, _ = _split(ref)
    return "https://%s/v2/%s/manifests/%s" % (registry, repository, digest)

def blob_url(ref, digest):
    """Returns the registry URL of a module blob."""
    registry, repository, _ = _split(ref)
    return "https://%s/v2/%s/blobs/%s" % (registry, repository, digest)

def module_layer(ref, manifest):
    """Returns the digest of the compiled template layer of a manifest and an error, one of them empty."""
    decoded = json.decode(manifest, default = None)
    if type(decoded) != "dict" or type(decoded.get("layers")) != "list":
        return "", "bicep registry module %s: manifest is not an OCI image manifest with layers" % ref
    layers = [layer for layer in decoded["layers"] if type(layer) == "dict" and layer.get("mediaType") == _LAYER]
    if len(layers) != 1:
        return "", "bicep registry module %s: manifest has %d layers of type %s, expected 1" % (ref, len(layers), _LAYER)
    digest = layers[0].get("digest", "")
    if type(digest) != "string" or not is_digest(digest):
        return "", "bicep registry module %s: layer %s has digest %r, expected sha256: plus 64 lowercase hex characters" % (ref, _LAYER, digest)
    return digest, ""

def fetch_failure(ref, what, digest, url):
    """Returns the error for a manifest or layer that was not fetched with its pinned digest."""
    return "bicep registry module %s: %s %s was not fetched from %s with that sha256" % (ref, what, digest, url)

def _fail(message):
    fail("bicep hub: " + message)

def _package_files(rctx, package):
    name = package["name"]
    ref = "br:%s:%s" % (name, package["version"])
    manifests = [package.get("extra", {}).get("bicep_manifest")]
    layers = package.get("archives", [])
    if type(manifests[0]) != "dict" or len(layers) != 1:
        _fail("%s: the lock needs one layer archive and an extra.bicep_manifest" % name)
    manifest_problems = lock_problems({"schema": 1, "packages": [{"name": name, "version": package["version"], "archives": manifests}]})
    if manifest_problems:
        _fail("; ".join(manifest_problems))
    manifest = "sha256:" + manifests[0]["sha256"]
    layer = "sha256:" + layers[0]["sha256"]
    directory = "%s/home/.bicep/%s" % (name, cache_directory(ref))
    for archive, what, output, headers in [
        (manifests[0], "manifest", directory + "/manifest", {"Accept": _MANIFEST}),
        (layers[0], "layer", directory + "/main.json", {}),
    ]:
        result = rctx.download(url = archive["urls"], output = output, sha256 = archive["sha256"], headers = headers, allow_fail = True)
        if not result.success:
            _fail(fetch_failure(ref, what, "sha256:" + archive["sha256"], archive["urls"][0]))
    found, error = module_layer(ref, rctx.read(directory + "/manifest"))
    if error:
        _fail(error)
    if found != layer:
        _fail("%s: manifest names the layer %s, the lock %s" % (name, found, layer))
    rctx.file(directory + "/metadata", json.encode_indent({"manifestDigest": manifest}, indent = "  "))
    rctx.file(name + "/BUILD.bazel", _PACKAGE_BUILD.format(
        target = repr(name.rpartition("/")[2]),
        cache_root = repr(name + "/home/.bicep"),
        digest = repr(manifest),
        ref = repr(ref),
    ))

_PACKAGE_BUILD = """load("@rules_dx//bicep:defs.bzl", "bicep_registry_module")

bicep_registry_module(
    name = {target},
    srcs = glob(["home/.bicep/br/**"]),
    cache_root = {cache_root},
    digest = {digest},
    ref = {ref},
    visibility = ["//visibility:public"],
)
"""

def _hub_impl(rctx):
    for package in json.decode(rctx.attr.lock)["packages"]:
        _package_files(rctx, package)
    rctx.file("BUILD.bazel", "")
    return rctx.repo_metadata(reproducible = True)

bicep_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Lays out every locked Bicep registry module in the cache layout of the Bicep CLI, one package per module.",
    attrs = {
        "lock": attr.string(mandatory = True, doc = "The decoded dx lock as JSON text."),
    },
)
