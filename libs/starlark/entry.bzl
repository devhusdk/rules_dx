"""Shared main inference for executable wrappers."""

def dx_main_candidates(name, ext, dirs = []):
    """Returns the paths tried when main is unset: the name's stem, then each dir."""
    stem = name.split("/")[-1] + ext
    return [stem] + [d + "/" + stem for d in dirs]

def dx_infer_main(kind, name, main, srcs, candidates):
    """Returns main, or the one candidate in srcs or on disk, or the only src."""
    if main != None:
        return main
    found = [c for c in candidates if c in (srcs or [])] or native.glob(candidates, allow_empty = True)
    if len(found) == 1:
        return found[0]
    if not found and srcs != None and len(srcs) == 1 and type(srcs[0]) == "string":
        return srcs[0]
    fail("{} //{}:{} has no main; add {} or set main.".format(kind, native.package_name(), name, " or ".join(candidates)))

def _external(label):
    return type(label) != "string" or label.startswith("@") or label.startswith("//")

def dx_entry_quality_srcs(main, srcs, extra = None):
    """Returns the sources an executable owns: main, then srcs, then extra, once each."""
    owned = []
    for src in ([main] if main != None and not _external(main) else []) + list(srcs or []) + list(extra or []):
        if src not in owned:
            owned.append(src)
    return owned
