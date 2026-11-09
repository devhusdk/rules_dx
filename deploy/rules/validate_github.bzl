"""Validators for the github deploy rule."""

load(":validate.bzl", "NAME_CHARS", "only_chars")

_VALID_TAG_CHARS = NAME_CHARS

def github_notes_error(notes, notes_file = None):
    """Validates the notes value: commits or none, with an optional notes_file."""
    if notes not in ["commits", "none"]:
        return "github_deploy: invalid notes '" + str(notes) + "': want 'commits' or 'none'; use notes_file for a file"
    if notes_file and notes != "none":
        return "github_deploy: notes_file needs notes = 'none'"
    return ""

def github_tag_error(tag):
    """Validates one release tag value."""
    if type(tag) != "string" or tag == "":
        return ("github_deploy: invalid release_tag '" + str(tag) +
                "': want a non-empty tag (for example 'v0.0.0-dryrun')")
    if tag == "current":
        return "github_deploy: invalid release_tag 'current': use tag_from_head = True"
    if not only_chars(tag, _VALID_TAG_CHARS):
        return ("github_deploy: invalid release_tag '" + tag +
                "': want only [A-Za-z0-9._-]")
    return ""

def github_asset_collision_error(assets):
    """Reports the first pair of artifacts that share one asset name."""
    seen = {}
    for label, base in assets:
        if base in seen:
            return ("github_deploy: artifacts " + seen[base] + " and " + label +
                    " share the asset name '" + base + "'")
        seen[base] = label
    return ""
