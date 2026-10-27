"""Local-first Maven Central publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")
load(":validate.bzl", "EMBED_UNSAFE", "NAME_CHARS", "VERSION_CHARS", "has_any", "only_chars")

_VALID_GROUP_CHARS = NAME_CHARS

_VALID_ARTIFACT_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

MAVEN_DEFAULT_REPOSITORY_URL = "https://central.sonatype.com/api/v1/publisher/"

_RETIRED_HOSTS = ["oss.sonatype.org", "s01.oss.sonatype.org"]

_SUPPORTED_HOSTS = ["central.sonatype.com"]

_PUBLISHER_PATH = "/api/v1/publisher/"

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
    raw_host = repository_url[len("https://"):].split("/")[0]
    path = repository_url[len("https://") + len(raw_host):]
    if host not in _SUPPORTED_HOSTS or path != _PUBLISHER_PATH:
        return ("maven_deploy: invalid repository_url '" + repository_url +
                "': want the Central Portal Publisher API '" +
                MAVEN_DEFAULT_REPOSITORY_URL + "'")
    return ""

def _maven_jar_kind_error(kind, filename):
    """Validates one jar-like filename value for a given kind."""
    if type(filename) != "string" or filename == "":
        return ("maven_deploy: invalid " + kind + " '" + str(filename) +
                "': want a non-empty .jar filename")
    if not filename.endswith(".jar"):
        return ("maven_deploy: invalid " + kind + " '" + filename +
                "': want a filename ending in .jar")
    if has_any(filename, EMBED_UNSAFE):
        return ("maven_deploy: invalid " + kind + " '" + filename +
                "': want no quotes, backslashes, spaces, or newlines " +
                "so the filename embeds safely in the deploy launcher")
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

    sources_rloc = ""
    javadoc_rloc = ""
    seen = {jar_file.path: "jar"}
    for kind, attr_value, check in [
        ("sources", ctx.attr.sources, maven_sources_error),
        ("javadoc", ctx.attr.javadoc, maven_javadoc_error),
    ]:
        if attr_value == None:
            continue
        files = attr_value[DefaultInfo].files.to_list()
        if len(files) != 1:
            fail("maven_deploy " + str(ctx.label) + ": " + kind + " " +
                 str(attr_value.label) + " provides " + str(len(files)) +
                 " files, want exactly one .jar")
        extra = files[0]
        extra_error = check(extra.basename)
        if extra_error != "":
            fail(extra_error + " (in " + str(ctx.label) + ")")
        if extra.path in seen:
            fail("maven_deploy " + str(ctx.label) + ": " + kind +
                 " is the same file as " + seen[extra.path])
        seen[extra.path] = kind
        if kind == "sources":
            sources_rloc = rlocation_path(ctx, extra)
        else:
            javadoc_rloc = rlocation_path(ctx, extra)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const JAR_RLOC: &str = \"" + jar_rloc + "\";\n" +
                  "const POM_RLOC: &str = \"" + pom_rloc + "\";\n" +
                  "const SOURCES_RLOC: &str = \"" + sources_rloc + "\";\n" +
                  "const JAVADOC_RLOC: &str = \"" + javadoc_rloc + "\";\n" +
                  "const GROUP: &str = \"" + ctx.attr.group + "\";\n" +
                  "const ARTIFACT: &str = \"" + ctx.attr.artifact + "\";\n" +
                  "const VERSION: &str = \"" + ctx.attr.version + "\";\n" +
                  "const REPOSITORY_URL: &str = \"" + ctx.attr.repository_url + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::maven_main(&dx_deploy_tools::MavenLaunch { jar_rloc: JAR_RLOC, pom_rloc: POM_RLOC, sources_rloc: SOURCES_RLOC, javadoc_rloc: JAVADOC_RLOC, group: GROUP, artifact: ARTIFACT, version: VERSION, repository_url: REPOSITORY_URL }, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_maven_launcher = rule(
    implementation = _maven_launcher_impl,
    doc = "Writes the launcher Rust source for one Maven deployment.",
    attrs = {
        "artifact": attr.string(
            doc = "Maven artifact id the jar and pom are published under.",
            mandatory = True,
        ),
        "group": attr.string(
            doc = "Maven group id the jar and pom are published under.",
            mandatory = True,
        ),
        "jar": attr.label(
            doc = "Single jar file to publish.",
            allow_single_file = True,
            mandatory = True,
        ),
        "javadoc": attr.label(
            doc = "Optional javadoc jar published as -javadoc.jar.",
            allow_single_file = True,
        ),
        "pom": attr.label(
            doc = "Single pom file published alongside the jar.",
            allow_single_file = True,
            mandatory = True,
        ),
        "repository_url": attr.string(
            doc = "Maven repository endpoint that receives the publish.",
            mandatory = True,
        ),
        "sources": attr.label(
            doc = "Optional sources jar published as -sources.jar.",
            allow_single_file = True,
        ),
        "version": attr.string(
            doc = "Maven version of the published artifact.",
            mandatory = True,
        ),
    },
)

def maven_deploy(name, jar, pom, group, artifact, version = "0.0.0", repository_url = MAVEN_DEFAULT_REPOSITORY_URL, profile = "release", sources = None, javadoc = None):
    """Publishes one jar plus its pom, and optional sources and javadoc jars, as a local-first Maven deployment.

    Live uploads require both sources and javadoc (Central rule).
    """
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
        javadoc = javadoc,
        pom = pom,
        sources = sources,
        repository_url = repository_url,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [jar, pom] + ([sources] if sources else []) + ([javadoc] if javadoc else []),
        deps = [Label("//deploy/rules:dx_deploy_tools")],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
