"""Local-first Maven Central publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")

MAVEN_SCHEMA_VERSION = 1

_VALID_GROUP_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"

_VALID_ARTIFACT_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"

_VALID_VERSION_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-+"

MAVEN_DEFAULT_REPOSITORY_URL = "https://oss.sonatype.org/service/local/staging/deploy/maven2/"

def maven_group_charset():
    """Returns the launcher-safe group charset via registry query."""
    return _VALID_GROUP_CHARS

def maven_artifact_charset():
    """Returns the launcher-safe artifact charset via registry query."""
    return _VALID_ARTIFACT_CHARS

def maven_version_charset():
    """Returns the launcher-safe version charset via registry query."""
    return _VALID_VERSION_CHARS

def maven_schema_error():
    """Validates the versioned coordinate charset schema."""
    if MAVEN_SCHEMA_VERSION != 1:
        return "maven coordinates: unsupported schema v" + str(MAVEN_SCHEMA_VERSION) + " (want v1)"
    if type(_VALID_GROUP_CHARS) != "string" or _VALID_GROUP_CHARS == "":
        return "maven group: want a non-empty charset (schema v1)"
    seen = {}
    for c in _VALID_GROUP_CHARS.elems():
        if c in seen:
            return "maven group: duplicate charset char '" + c + "'"
        seen[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/", "+"]:
            return "maven group: unsafe charset char '" + c + "' (must stay launcher-safe)"
    if type(_VALID_ARTIFACT_CHARS) != "string" or _VALID_ARTIFACT_CHARS == "":
        return "maven artifact: want a non-empty charset (schema v1)"
    seen_artifact = {}
    for c in _VALID_ARTIFACT_CHARS.elems():
        if c in seen_artifact:
            return "maven artifact: duplicate charset char '" + c + "'"
        seen_artifact[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/", "+"]:
            return "maven artifact: unsafe charset char '" + c + "' (must stay launcher-safe)"
    if type(_VALID_VERSION_CHARS) != "string" or _VALID_VERSION_CHARS == "":
        return "maven version: want a non-empty charset (schema v1)"
    seen_version = {}
    for c in _VALID_VERSION_CHARS.elems():
        if c in seen_version:
            return "maven version: duplicate charset char '" + c + "'"
        seen_version[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/"]:
            return "maven version: unsafe charset char '" + c + "' (must stay launcher-safe)"
    return ""

def maven_group_error(group):
    """Validates one Maven groupId value."""
    if type(group) != "string" or group == "":
        return ("maven_deploy: invalid group '" + str(group) +
                "': want a non-empty groupId (for example 'com.example')")
    for c in group.elems():
        if c not in _VALID_GROUP_CHARS:
            return ("maven_deploy: invalid group '" + group +
                    "': want only [A-Za-z0-9._-] so the group embeds " +
                    "safely in the deploy launcher")
    return ""

def maven_artifact_error(artifact):
    """Validates one Maven artifactId value."""
    if type(artifact) != "string" or artifact == "":
        return ("maven_deploy: invalid artifact '" + str(artifact) +
                "': want a non-empty artifactId (for example 'maven_demo')")
    for c in artifact.elems():
        if c not in _VALID_ARTIFACT_CHARS:
            return ("maven_deploy: invalid artifact '" + artifact +
                    "': want only [A-Za-z0-9._-] so the artifact embeds " +
                    "safely in the deploy launcher")
    return ""

def maven_version_error(version):
    """Validates one Maven version value."""
    if type(version) != "string" or version == "":
        return ("maven_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    for c in version.elems():
        if c not in _VALID_VERSION_CHARS:
            return ("maven_deploy: invalid version '" + version +
                    "': want only [A-Za-z0-9._-+] so the version embeds " +
                    "safely in the deploy launcher")
    return ""

def maven_repository_error(repository_url):
    """Validates one Maven repository URL value."""
    if type(repository_url) != "string" or repository_url == "":
        return ("maven_deploy: invalid repository_url '" + str(repository_url) +
                "': want a non-empty https URL (for example '" +
                MAVEN_DEFAULT_REPOSITORY_URL + "')")
    if not repository_url.startswith("https://"):
        return ("maven_deploy: invalid repository_url '" + repository_url +
                "': want an https URL so uploads never go over plaintext")
    for c in repository_url.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("maven_deploy: invalid repository_url '" + repository_url +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the URL embeds safely in the deploy launcher")
    return ""

def maven_jar_error(filename):
    """Validates one jar filename value."""
    if type(filename) != "string" or filename == "":
        return ("maven_deploy: invalid jar '" + str(filename) +
                "': want a non-empty .jar filename")
    if not filename.endswith(".jar"):
        return ("maven_deploy: invalid jar '" + filename +
                "': want a filename ending in .jar")
    for c in filename.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("maven_deploy: invalid jar '" + filename +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the filename embeds safely in the deploy launcher")
    return ""

def maven_pom_error(filename):
    """Validates one pom filename value."""
    if type(filename) != "string" or filename == "":
        return ("maven_deploy: invalid pom '" + str(filename) +
                "': want a non-empty .pom filename")
    if not filename.endswith(".pom"):
        return ("maven_deploy: invalid pom '" + filename +
                "': want a filename ending in .pom")
    for c in filename.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("maven_deploy: invalid pom '" + filename +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the filename embeds safely in the deploy launcher")
    return ""

def _maven_launcher_impl(ctx):
    """Expands the rust_binary launcher for one Maven deployment."""
    jar_files = ctx.attr.jar[DefaultInfo].files.to_list()
    if len(jar_files) != 1:
        fail("maven_deploy " + str(ctx.label) + ": jar " +
             str(ctx.attr.jar.label) + " provides " +
             str(len(jar_files)) + " files, want exactly one .jar")
    jar_file = jar_files[0]
    jar_error = maven_jar_error(jar_file.basename)
    if jar_error != "":
        fail(jar_error + " (in " + str(ctx.label) + ")")
    jar_rloc = rlocation_path(ctx, jar_file)

    pom_files = ctx.attr.pom[DefaultInfo].files.to_list()
    if len(pom_files) != 1:
        fail("maven_deploy " + str(ctx.label) + ": pom " +
             str(ctx.attr.pom.label) + " provides " +
             str(len(pom_files)) + " files, want exactly one .pom")
    pom_file = pom_files[0]
    pom_error = maven_pom_error(pom_file.basename)
    if pom_error != "":
        fail(pom_error + " (in " + str(ctx.label) + ")")
    pom_rloc = rlocation_path(ctx, pom_file)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const JAR_RLOC: &str = \"" + jar_rloc + "\";\n" +
                  "const POM_RLOC: &str = \"" + pom_rloc + "\";\n" +
                  "const GROUP: &str = \"" + ctx.attr.group + "\";\n" +
                  "const ARTIFACT: &str = \"" + ctx.attr.artifact + "\";\n" +
                  "const VERSION: &str = \"" + ctx.attr.version + "\";\n" +
                  "const REPOSITORY_URL: &str = \"" + ctx.attr.repository_url + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::maven_main(JAR_RLOC, POM_RLOC, GROUP, ARTIFACT, VERSION, REPOSITORY_URL, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_maven_launcher = rule(
    implementation = _maven_launcher_impl,
    attrs = {
        "artifact": attr.string(
            mandatory = True,
        ),
        "group": attr.string(
            mandatory = True,
        ),
        "jar": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "pom": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "repository_url": attr.string(
            mandatory = True,
        ),
        "version": attr.string(
            mandatory = True,
        ),
    },
)

def maven_deploy(name, jar, pom, group, artifact, version = "0.0.0", repository_url = MAVEN_DEFAULT_REPOSITORY_URL, profile = "release"):
    """Publishes one jar plus its pom as a local-first Maven deployment."""
    group_error = maven_group_error(group)
    if group_error != "":
        fail(group_error + " (in " + native.package_name() + ":" + name + ")")
    artifact_error = maven_artifact_error(artifact)
    if artifact_error != "":
        fail(artifact_error + " (in " + native.package_name() + ":" + name + ")")
    version_error = maven_version_error(version)
    if version_error != "":
        fail(version_error + " (in " + native.package_name() + ":" + name + ")")
    repository_error = maven_repository_error(repository_url)
    if repository_error != "":
        fail(repository_error + " (in " + native.package_name() + ":" + name + ")")

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _maven_launcher(
        name = launcher_target,
        artifact = artifact,
        group = group,
        jar = jar,
        pom = pom,
        repository_url = repository_url,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [jar, pom],
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
