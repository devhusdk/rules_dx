"""Local-first Maven Central publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")
load(":validate.bzl", "EMBED_UNSAFE", "NAME_CHARS", "VERSION_CHARS", "has_any", "only_chars")

_VALID_GROUP_CHARS = NAME_CHARS

_VALID_ARTIFACT_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

MAVEN_DEFAULT_REPOSITORY_URL = "https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/"

_RETIRED_HOSTS = ["oss.sonatype.org", "s01.oss.sonatype.org"]

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
                "': want only [A-Za-z0-9._-] so the group embeds " +
                "safely in the deploy launcher")
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
                "': want only [A-Za-z0-9._-] so the artifact embeds " +
                "safely in the deploy launcher")
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
                "': want only [A-Za-z0-9._-+] so the version embeds " +
                "safely in the deploy launcher")
    if _all_dots(version):
        return ("maven_deploy: invalid version '" + version +
                "': want a version that is not made only of dots")
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
    host = repository_url[len("https://"):].split("/")[0].lower()
    if host in _RETIRED_HOSTS:
        return ("maven_deploy: invalid repository_url '" + repository_url +
                "': " + host + " is retired OSSRH infrastructure (use '" +
                MAVEN_DEFAULT_REPOSITORY_URL + "')")
    if has_any(repository_url, EMBED_UNSAFE):
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
    if has_any(filename, EMBED_UNSAFE):
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
    if has_any(filename, EMBED_UNSAFE):
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
    doc = "Writes the launcher Rust source for one Maven deployment.",
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
        deps = [Label("//deploy/rules:dx_deploy_tools")],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
