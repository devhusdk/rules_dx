"""Validators for the resources deploy rule."""

def resource_path_error(path):
    """Validates one logical resource path, returning an error or empty."""
    if type(path) != "string" or path == "":
        return "must not be empty"
    if path.startswith("/"):
        return "must be relative"
    if "\\" in path:
        return "must use '/' separators"
    for segment in path.split("/"):
        if segment == "":
            return "must not contain empty segments"
        if segment == "." or segment == "..":
            return "must not contain '.' or '..'"
    return ""

def resource_mapping_error(paths):
    """Validates staged logical paths, returning a duplicate or collision error or empty."""
    seen = {}
    for path in paths:
        if path in seen:
            return "duplicate path '" + path + "'"
        seen[path] = True
    segmented = [(path, path.split("/")) for path in paths]
    for i in range(len(segmented)):
        for j in range(i + 1, len(segmented)):
            (a, parts_a) = segmented[i]
            (b, parts_b) = segmented[j]
            if len(parts_a) <= len(parts_b):
                (first, short, second, long) = (a, parts_a, b, parts_b)
            else:
                (first, short, second, long) = (b, parts_b, a, parts_a)
            if long[:len(short)] == short:
                return "path '" + first + "' collides with '" + second + "'"
    return ""
