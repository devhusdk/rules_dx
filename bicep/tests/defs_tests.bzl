"""bicep_library ownership and provider proofs."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//bicep/private:providers.bzl", "BicepInfo")
load("//bicep/private:registry.bzl", "blob_url", "cache_directory", "fetch_failure", "is_digest", "manifest_url", "module_layer")
load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core:owns_class_test.bzl", "owns_class_test")

owns_bicep_class_test = owns_class_test("bicep", ["app.bicep"])

def _transitive_context_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[BicepInfo]
    asserts.equals(env, ["app.bicep", "lib.bicep"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, ["data.json"], [f.basename for f in info.transitive_data.to_list()])
    return analysistest.end(env)

transitive_context_test = analysistest.make(_transitive_context_impl)

def _registry_deps_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[BicepInfo]
    asserts.equals(env, ["main.json", "manifest"], sorted([f.basename for f in info.transitive_registry.to_list()]))
    asserts.equals(env, ["registry_app.bicep"], [f.basename for f in info.transitive_srcs.to_list()])
    return analysistest.end(env)

registry_deps_test = analysistest.make(_registry_deps_impl)

_REF = "br:mcr.microsoft.com/bicep/avm/res/storage/storage-account:0.14.3"
_DIGEST = "sha256:" + "a" * 64
_LAYER = "sha256:" + "b" * 64

def _layer(digest):
    return {"mediaType": "application/vnd.ms.bicep.module.layer.v1+json", "digest": digest}

def _manifest(layers):
    return json.encode({"schemaVersion": 2, "layers": layers})

def registry_unit_tests(name):
    """Declares unit tests of the registry cache layout, URLs and manifest checks."""
    starlark_test(name = name, mode = "unit", checks = [
        expect_equal("cache directory", cache_directory(_REF), "br/mcr.microsoft.com/bicep$avm$res$storage$storage-account/0.14.3$"),
        expect_equal("cache directory lowers the host and escapes its port", cache_directory("br:Example.azurecr.io:443/m:1"), "br/example.azurecr.io$443/m/1$"),
        expect_equal("cache directory marks uppercase tag characters", cache_directory("br:mcr.microsoft.com/a/b:V1.Ab"), "br/mcr.microsoft.com/a$b/V1.Ab$9"),
        expect_equal("cache directory marks uppercase past 64 characters", cache_directory("br:mcr.microsoft.com/m:A" + "a" * 63 + "B"), "br/mcr.microsoft.com/m/A" + "a" * 63 + "B$10000000000000001"),
        expect_equal("manifest url", manifest_url(_REF, _DIGEST), "https://mcr.microsoft.com/v2/bicep/avm/res/storage/storage-account/manifests/" + _DIGEST),
        expect_equal("manifest url keeps the port", manifest_url("br:localhost:5000/m:1", _DIGEST), "https://localhost:5000/v2/m/manifests/" + _DIGEST),
        expect_equal("blob url", blob_url(_REF, _LAYER), "https://mcr.microsoft.com/v2/bicep/avm/res/storage/storage-account/blobs/" + _LAYER),
        expect_equal("module layer", module_layer(_REF, _manifest([_layer(_LAYER)])), (_LAYER, "")),
        expect_equal("other layers are skipped", module_layer(_REF, _manifest([{"mediaType": "application/x.other", "digest": _DIGEST}, _layer(_LAYER)])), (_LAYER, "")),
        expect_equal("no module layer", module_layer(_REF, _manifest([])), ("", "bicep registry module %s: manifest has 0 layers of type application/vnd.ms.bicep.module.layer.v1+json, expected 1" % _REF)),
        expect_equal("two module layers", module_layer(_REF, _manifest([_layer(_LAYER), _layer(_LAYER)]))[1], "bicep registry module %s: manifest has 2 layers of type application/vnd.ms.bicep.module.layer.v1+json, expected 1" % _REF),
        expect_equal("not json", module_layer(_REF, "<html>"), ("", "bicep registry module %s: manifest is not an OCI image manifest with layers" % _REF)),
        expect_equal("no layers key", module_layer(_REF, "{}")[1], "bicep registry module %s: manifest is not an OCI image manifest with layers" % _REF),
        expect_equal("empty layer digest names the layer", module_layer(_REF, _manifest([_layer("")])), ("", "bicep registry module %s: layer application/vnd.ms.bicep.module.layer.v1+json has digest \"\", expected sha256: plus 64 lowercase hex characters" % _REF)),
        expect_equal("short layer digest names the layer", module_layer(_REF, _manifest([_layer("sha256:abc")]))[1], "bicep registry module %s: layer application/vnd.ms.bicep.module.layer.v1+json has digest \"sha256:abc\", expected sha256: plus 64 lowercase hex characters" % _REF),
        expect_equal("sha512 layer digest is rejected", module_layer(_REF, _manifest([_layer("sha512:" + "b" * 64)]))[0], ""),
        expect_equal(
            "wrong layer digest names the layer",
            fetch_failure(_REF, "layer", _LAYER, blob_url(_REF, _LAYER)),
            "bicep registry module %s: layer %s was not fetched from https://mcr.microsoft.com/v2/bicep/avm/res/storage/storage-account/blobs/%s with that sha256" % (_REF, _LAYER, _LAYER),
        ),
        expect_equal(
            "wrong manifest digest names the manifest",
            fetch_failure(_REF, "manifest", _DIGEST, manifest_url(_REF, _DIGEST)),
            "bicep registry module %s: manifest %s was not fetched from https://mcr.microsoft.com/v2/bicep/avm/res/storage/storage-account/manifests/%s with that sha256" % (_REF, _DIGEST, _DIGEST),
        ),
        expect_equal("digest", [is_digest(_DIGEST), is_digest("sha256:" + "g" * 64), is_digest("sha256:" + "a" * 65)], [True, False, False]),
    ])
