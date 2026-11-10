"""Validators for the cloudflare deploy rule."""

load(":validate.bzl", "NAME_CHARS", "only_chars")

_HEX = "0123456789abcdef"

_PROJECT_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789-"

def _branch_error(attr, value):
    if type(value) != "string" or value == "" or len(value) > 100:
        return ("cloudflare_pages_deploy: invalid " + attr + " '" + str(value) +
                "': want 1-100 characters of [A-Za-z0-9._/-]")
    if not only_chars(value, NAME_CHARS + "/"):
        return ("cloudflare_pages_deploy: invalid " + attr + " '" + value +
                "': want 1-100 characters of [A-Za-z0-9._/-]")
    if value.startswith("-") or value.startswith("/") or value.endswith("/") or ".." in value or "//" in value:
        return ("cloudflare_pages_deploy: invalid " + attr + " '" + value +
                "': no leading '-', no '..', no empty path parts")
    return ""

def cloudflare_pages_error(account_id, project, branch, production_branch):
    """Validates the account, project and branch values."""
    if type(account_id) != "string" or len(account_id) != 32:
        return ("cloudflare_pages_deploy: invalid account_id '" + str(account_id) +
                "': want 32 lowercase hex characters")
    if not only_chars(account_id, _HEX):
        return ("cloudflare_pages_deploy: invalid account_id '" + account_id +
                "': want 32 lowercase hex characters")
    bad_project = type(project) != "string" or project == "" or len(project) > 58 or project.startswith("-") or project.endswith("-")
    if not bad_project:
        bad_project = not only_chars(project, _PROJECT_CHARS)
    if bad_project:
        return ("cloudflare_pages_deploy: invalid project '" + str(project) +
                "': want 1-58 characters of [a-z0-9-] that start and end with a letter or digit")
    branch_error = _branch_error("branch", branch)
    if branch_error != "":
        return branch_error
    return _branch_error("production_branch", production_branch)
