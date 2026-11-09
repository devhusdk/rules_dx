def typescript_inner_kwargs(tags, kwargs):
    """Returns the tags and target_compatible_with kwargs for inner targets."""
    out = {}
    inner_tags = tags if tags != None else kwargs.get("tags", None)
    if inner_tags != None:
        out["tags"] = list(inner_tags)
    if kwargs.get("target_compatible_with", None) != None:
        out["target_compatible_with"] = kwargs["target_compatible_with"]
    return out

def typescript_forward_kwargs(scoped):
    """Returns the kwargs the public typescript_project forwarder keeps."""
    out = {}
    for key in ("aspect_hints", "hdrs", "tags", "testonly", "target_compatible_with"):
        if scoped.get(key, None) != None:
            out[key] = scoped[key]
    return out
