"""Validators for the octopus deploy rule."""

load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "charset_error", "https_url_error", "only_chars", "suffix_error")

_VALID_PROJECT_CHARS = NAME_CHARS

_VALID_CHANNEL_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

_VALID_ENVIRONMENT_CHARS = NAME_CHARS

OCTOPUS_DEFAULT_REGISTRY_URL = "https://octopus.example.invalid"

def octopus_project_error(project):
    """Validates one Octopus project name value."""
    return charset_error(
        "octopus_deploy",
        "project",
        project,
        _VALID_PROJECT_CHARS,
        "a non-empty project (for example 'octopus_demo')",
        "only [A-Za-z0-9._-]",
    )

def octopus_channel_error(channel):
    """Validates one Octopus channel name value."""
    return charset_error(
        "octopus_deploy",
        "channel",
        channel,
        _VALID_CHANNEL_CHARS,
        "a non-empty channel (for example 'Default')",
        "only [A-Za-z0-9._-]",
    )

def octopus_version_error(version):
    """Validates one Octopus release version value."""
    return charset_error(
        "octopus_deploy",
        "version",
        version,
        _VALID_VERSION_CHARS,
        "a non-empty version (for example '0.0.0')",
        "only [A-Za-z0-9._-+]",
    )

def octopus_environment_error(environment):
    """Validates one Octopus environment (--deployTo) value."""
    return charset_error(
        "octopus_deploy",
        "environment",
        environment,
        _VALID_ENVIRONMENT_CHARS,
        "a non-empty environment (for example 'Production')",
        "only [A-Za-z0-9._-]",
    )

def octopus_space_error(space):
    """Validates one Octopus space name value (empty means the default space)."""
    if space == "":
        return ""
    return charset_error(
        "octopus_deploy",
        "space",
        space,
        _VALID_ENVIRONMENT_CHARS,
        "a space name (for example 'Default') or empty for the default space",
        "only [A-Za-z0-9._-]",
    )

def octopus_registry_url_error(registry_url):
    """Validates one Octopus server URL value."""
    return https_url_error(
        "octopus_deploy",
        "registry_url",
        registry_url,
        "a non-empty https URL (for example '" + OCTOPUS_DEFAULT_REGISTRY_URL + "')",
        "an https URL so pushes never go over plaintext",
    )

def octopus_package_id_error(package):
    """Validates one Octopus package id value."""
    return charset_error(
        "octopus_deploy",
        "package",
        package,
        _VALID_PROJECT_CHARS,
        "a non-empty package id (for example 'octopus_demo')",
        "only [A-Za-z0-9._-]",
    )

def octopus_package_error(filename):
    """Validates one Octopus package filename value."""
    return suffix_error(
        "octopus_deploy",
        "package",
        filename,
        ".tar.gz",
        "a non-empty .tar.gz filename from archive_deploy",
        "a filename ending in .tar.gz (archive_deploy output)",
    )
