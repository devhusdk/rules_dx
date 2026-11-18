"""Warnings for module extensions that read tags from the root module only."""

visibility(["//core/...", "//python/..."])

def dropped_tag_warnings(extension, modules, tag_classes):
    """Returns one warning per non-root module and tag class whose tags the extension ignores."""
    warnings = []
    for mod in modules:
        if mod.is_root:
            continue
        for tag_class in tag_classes:
            if getattr(mod.tags, tag_class):
                warnings.append("{ext}: module '{mod}' sets {ext}.{tag}(...), which is ignored because only the root module's {tag} tags count; move the tag to the root MODULE.bazel to apply it".format(
                    ext = extension,
                    mod = mod.name,
                    tag = tag_class,
                ))
    return warnings

def warn_dropped_tags(extension, modules, tag_classes):
    """Prints each warning from dropped_tag_warnings."""
    for warning in dropped_tag_warnings(extension, modules, tag_classes):
        print("WARNING: " + warning)  # buildifier: disable=print
