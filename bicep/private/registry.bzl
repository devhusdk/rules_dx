"""Repository rule that downloads one Bicep registry module pinned by digest."""

_HEX = "0123456789abcdef"

_MANIFEST = "application/vnd.oci.image.manifest.v1+json"

_LAYER = "application/vnd.ms.bicep.module.layer.v1+json"

def registry_errors(ref, digest):
    """Returns the errors in a registry module declaration, ref and digest."""
    errors = []
    if not ref.startswith("br:"):
        errors.append("ref %r must start with br:" % ref)
    elif ":" not in ref.rpartition("/")[2] or ref.endswith(":"):
        errors.append("ref %r must end in :<tag>" % ref)
    if not is_digest(digest):
        errors.append("digest %r must be sha256: plus 64 lowercase hex characters" % digest)
    return errors

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

def _fetch(rctx, what, digest, url, output, headers):
    result = rctx.download(url = [url], output = output, sha256 = digest[len("sha256:"):], headers = headers, allow_fail = True)
    if not result.success:
        fail(fetch_failure(rctx.attr.ref, what, digest, url))

def _impl(rctx):
    ref = rctx.attr.ref
    errors = registry_errors(ref, rctx.attr.digest)
    if errors:
        fail("bicep registry module: " + "; ".join(errors))
    directory = "home/.bicep/" + cache_directory(ref)
    _fetch(rctx, "manifest", rctx.attr.digest, manifest_url(ref, rctx.attr.digest), directory + "/manifest", {"Accept": _MANIFEST})
    layer, error = module_layer(ref, rctx.read(directory + "/manifest"))
    if error:
        fail(error)
    _fetch(rctx, "layer", layer, blob_url(ref, layer), directory + "/main.json", {})
    rctx.file(directory + "/metadata", json.encode_indent({"manifestDigest": rctx.attr.digest}, indent = "  "))
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
    doc = "Downloads one Bicep registry module by its manifest digest into the cache layout of the Bicep CLI.",
    attrs = {
        "digest": attr.string(mandatory = True, doc = "sha256: and 64 hex characters of the module manifest."),
        "ref": attr.string(mandatory = True, doc = "Module reference, br:HOST/PATH:TAG."),
    },
)
