"""Warnings for module extensions that read tags from the root module only."""

visibility(["//core/...", "//python/..."])

def dropped_tag_warnings(extension, modules, tag_classes):
    """Returns one warning per non-root module and tag class whose tags the extension ignores.

    A root-only extension used to skip these tags with no trace, so a dependency that set
    one never learned it had no effect. The warning names the module and the tag so the
    owner of the root module can copy the tag over if they want it.

    Args:
      extension: the extension name as users write it, e.g. "dx_licenses".
      modules: module_ctx.modules, or structs with name, is_root and tags.
      tag_classes: the tag class names the extension reads from the root module only.

    Returns:
      A list of warning strings, empty when no non-root module sets one of the tags.
    """
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
