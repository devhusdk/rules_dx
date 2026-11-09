load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "only_chars")

_VALID_GROUP_CHARS = NAME_CHARS

_VALID_ARTIFACT_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

def _all_dots(value):
    """Reports whether a value is only dots."""
    return value.replace(".", "") == ""

def maven_group_error(group):
    """Validates one Maven groupId value."""
    if type(group) != "string" or group == "":
        return ("maven_deploy: invalid group '" + str(group) +
                "': want a non-empty groupId (for example 'com.example')")
    if not only_chars(group, _VALID_GROUP_CHARS):
        return ("maven_deploy: invalid group '" + group +
                "': want only [A-Za-z0-9._-]")
    for segment in group.split("."):
        if segment == "":
            return ("maven_deploy: invalid group '" + group +
                    "': want non-empty dot-separated segments (no leading, " +
                    "trailing, or repeated dots)")
    return ""

def maven_artifact_error(artifact):
    """Validates one Maven artifactId value."""
    if type(artifact) != "string" or artifact == "":
        return ("maven_deploy: invalid artifact '" + str(artifact) +
                "': want a non-empty artifactId (for example 'maven_demo')")
    if not only_chars(artifact, _VALID_ARTIFACT_CHARS):
        return ("maven_deploy: invalid artifact '" + artifact +
                "': want only [A-Za-z0-9._-]")
    if _all_dots(artifact):
        return ("maven_deploy: invalid artifact '" + artifact +
                "': want a name that is not made only of dots")
    return ""

def maven_version_error(version):
    """Validates one Maven version value."""
    if type(version) != "string" or version == "":
        return ("maven_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("maven_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    if _all_dots(version):
        return ("maven_deploy: invalid version '" + version +
                "': want a version that is not made only of dots")
    return ""

def _maven_jar_kind_error(kind, filename):
    """Validates one jar-like filename value for a given kind."""
    if type(filename) != "string" or filename == "":
        return ("maven_deploy: invalid " + kind + " '" + str(filename) +
                "': want a non-empty .jar filename")
    if not filename.endswith(".jar"):
        return ("maven_deploy: invalid " + kind + " '" + filename +
                "': want a filename ending in .jar")
    return ""

def maven_jar_error(filename):
    """Validates one jar filename value."""
    return _maven_jar_kind_error("jar", filename)

def maven_sources_error(filename):
    """Validates one sources jar filename value."""
    return _maven_jar_kind_error("sources jar", filename)

def maven_javadoc_error(filename):
    """Validates one javadoc jar filename value."""
    return _maven_jar_kind_error("javadoc jar", filename)

def maven_pom_error(filename):
    """Validates one pom filename value."""
    if type(filename) != "string" or filename == "":
        return ("maven_deploy: invalid pom '" + str(filename) +
                "': want a non-empty .pom filename")
    if not filename.endswith(".pom"):
        return ("maven_deploy: invalid pom '" + filename +
                "': want a filename ending in .pom")
    return ""
