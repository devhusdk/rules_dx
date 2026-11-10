def qt_uri_error(uri):
    """Returns why a QML module URI is malformed, or an empty string."""
    if uri == "":
        return "uri must not be empty"
    for segment in uri.split("."):
        if segment == "" or not (segment[0].isalpha() or segment[0] == "_"):
            return "uri '" + uri + "' segment '" + segment + "' must start with a letter"
        for char in segment.elems():
            if not (char.isalnum() or char == "_"):
                return "uri '" + uri + "' segment '" + segment + "' holds '" + char + "'"
    return ""
