"""Shared path and platform helpers for package rules."""

def data_path(file, label):
    """Returns the install path of a data file relative to its target's package."""
    owner = file.owner
    if owner == None or owner.repo_name != label.repo_name or owner.package != label.package:
        return file.basename
    prefix = ("../" + label.repo_name + "/" if label.repo_name else "") + (label.package + "/" if label.package else "")
    return file.short_path[len(prefix):]

def platform_arch(has_constraint, os, arches):
    """Returns the first matching arch name for the target OS, or ""."""
    if not has_constraint(os):
        return ""
    for name, constraint in arches:
        if has_constraint(constraint):
            return name
    return ""

def claim_dest(seen, dest, value, fold_case = False):
    """Records value for dest and returns the earlier value, or None."""
    key = dest.lower() if fold_case else dest
    if key in seen:
        return seen[key]
    seen[key] = value
    return None

def only_chars(value, allowed):
    """Returns True when every char of value is in allowed."""
    for c in value.elems():
        if c not in allowed:
            return False
    return True

def has_any(value, chars):
    """Returns True when value contains any of chars."""
    for c in value.elems():
        if c in chars:
            return True
    return False
