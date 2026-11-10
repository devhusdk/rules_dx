"""canonical helpers."""

def is_canonical(text):
    """Returns whether text starts with the canonical repository marker."""
    return text.startswith("@@")  # buildifier: disable=canonical-repository

def strip_canonical(text):
    """Strips one leading canonical-repository marker for readable observations."""
    if is_canonical(text):
        return text[2:]
    return text
