"""Validators for the octopus deploy rule."""

load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "only_chars")

_VALID_PROJECT_CHARS = NAME_CHARS

_VALID_CHANNEL_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

_VALID_ENVIRONMENT_CHARS = NAME_CHARS

OCTOPUS_DEFAULT_REGISTRY_URL = "https://octopus.example.invalid"

def octopus_project_error(project):
    """Validates one Octopus project name value."""
    if type(project) != "string" or project == "":
        return ("octopus_deploy: invalid project '" + str(project) +
                "': want a non-empty project (for example 'octopus_demo')")
    if not only_chars(project, _VALID_PROJECT_CHARS):
        return ("octopus_deploy: invalid project '" + project +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_channel_error(channel):
    """Validates one Octopus channel name value."""
    if type(channel) != "string" or channel == "":
        return ("octopus_deploy: invalid channel '" + str(channel) +
                "': want a non-empty channel (for example 'Default')")
    if not only_chars(channel, _VALID_CHANNEL_CHARS):
        return ("octopus_deploy: invalid channel '" + channel +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_version_error(version):
    """Validates one Octopus release version value."""
    if type(version) != "string" or version == "":
        return ("octopus_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("octopus_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def octopus_environment_error(environment):
    """Validates one Octopus environment (--deployTo) value."""
    if type(environment) != "string" or environment == "":
        return ("octopus_deploy: invalid environment '" + str(environment) +
                "': want a non-empty environment (for example 'Production')")
    if not only_chars(environment, _VALID_ENVIRONMENT_CHARS):
        return ("octopus_deploy: invalid environment '" + environment +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_space_error(space):
    """Validates one Octopus space name value (empty means the default space)."""
    if space == "":
        return ""
    if type(space) != "string":
        return ("octopus_deploy: invalid space '" + str(space) +
                "': want a space name (for example 'Default') or empty for the default space")
    if not only_chars(space, _VALID_ENVIRONMENT_CHARS):
        return ("octopus_deploy: invalid space '" + space +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_registry_url_error(registry_url):
    """Validates one Octopus server URL value."""
    if type(registry_url) != "string" or registry_url == "":
        return ("octopus_deploy: invalid registry_url '" + str(registry_url) +
                "': want a non-empty https URL (for example '" +
                OCTOPUS_DEFAULT_REGISTRY_URL + "')")
    if not registry_url.startswith("https://"):
        return ("octopus_deploy: invalid registry_url '" + registry_url +
                "': want an https URL so pushes never go over plaintext")
    return ""

def octopus_package_id_error(package):
    """Validates one Octopus package id value."""
    if type(package) != "string" or package == "":
        return ("octopus_deploy: invalid package '" + str(package) +
                "': want a non-empty package id (for example 'octopus_demo')")
    if not only_chars(package, _VALID_PROJECT_CHARS):
        return ("octopus_deploy: invalid package '" + package +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_package_error(filename):
    """Validates one Octopus package filename value."""
    if type(filename) != "string" or filename == "":
        return ("octopus_deploy: invalid package '" + str(filename) +
                "': want a non-empty .tar.gz filename from archive_deploy")
    if not filename.endswith(".tar.gz"):
        return ("octopus_deploy: invalid package '" + filename +
                "': want a filename ending in .tar.gz (archive_deploy output)")
    return ""
