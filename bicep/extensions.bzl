"""Module extension that builds one hub of Bicep registry modules per lock tag."""

load("//bicep/private:registry.bzl", "bicep_hub")
load("//core:pins.bzl", "lock_problems", "lock_tag")

def _impl(mctx):
    direct = []
    dev = []
    seen = {}
    for module in mctx.modules:
        for tag in module.tags.lock:
            if tag.name in seen:
                fail("bicep hub %r is declared by both %s and %s" % (tag.name, seen[tag.name], module.name))
            seen[tag.name] = module.name
            text = mctx.read(tag.lock)
            problems = lock_problems(json.decode(text))
            if problems:
                fail("%s %s" % (tag.lock, "; ".join(problems)))
            bicep_hub(name = tag.name, lock = text)
            if module.is_root:
                if mctx.is_dev_dependency(tag):
                    dev.append(tag.name)
                else:
                    direct.append(tag.name)
    return mctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = direct,
        root_module_direct_dev_deps = dev,
    )

bicep = module_extension(
    doc = "Builds one hub of Bicep registry modules per lock tag.",
    implementation = _impl,
    tag_classes = {"lock": lock_tag},
)
