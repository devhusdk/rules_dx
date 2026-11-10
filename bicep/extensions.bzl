"""Module extension that restores Bicep registry modules pinned by manifest digest."""

load("//bicep/private:registry.bzl", "bicep_registry_repo", "registry_errors")
load("//quality/artifacts:bicep.bzl", "ARTIFACTS")

def _clis():
    return {key: Label("@dx_bicep_%s//:%s" % (key, art["executable"])) for key, art in ARTIFACTS.items()}

def _impl(ctx):
    seen = {}
    for module in ctx.modules:
        for tag in module.tags.registry_module:
            errors = registry_errors(tag.ref, tag.digest)
            if errors:
                fail("bicep.registry_module %r: %s" % (tag.name, "; ".join(errors)))
            if tag.name in seen and seen[tag.name] != (tag.ref, tag.digest):
                fail("bicep.registry_module: conflicting declarations of %r" % tag.name)
            seen[tag.name] = (tag.ref, tag.digest)
            bicep_registry_repo(name = tag.name, ref = tag.ref, digest = tag.digest, clis = _clis())
    return ctx.extension_metadata(reproducible = True)

_registry_module = tag_class(
    doc = "Restores one br: module into a repository.",
    attrs = {
        "digest": attr.string(mandatory = True, doc = "sha256: and 64 hex characters of the module manifest. Print it with `bazel run //bicep/private/tools:pin -- <ref>`."),
        "name": attr.string(mandatory = True, doc = "Repository name."),
        "ref": attr.string(mandatory = True, doc = "Module reference, br:HOST/PATH:TAG."),
    },
)

bicep = module_extension(
    doc = "Restores Bicep registry modules.",
    implementation = _impl,
    tag_classes = {"registry_module": _registry_module},
)
