"""Local-first Maven Central publisher for dx deploy."""

load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")
load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_maven.bzl", "maven_artifact_error", "maven_group_error", "maven_jar_error", "maven_javadoc_error", "maven_pom_error", "maven_sources_error", "maven_version_error")

def _maven_config_impl(ctx):
    """Writes the deploy config for one Maven deployment."""
    jar_files = ctx.attr.jar[DefaultInfo].files.to_list()
    if len(jar_files) != 1:
        fail("maven_deploy " + str(ctx.label) + ": jar " +
             str(ctx.attr.jar.label) + " provides " +
             str(len(jar_files)) + " files, want exactly one .jar")
    jar_file = jar_files[0]
    jar_error = maven_jar_error(jar_file.basename)
    if jar_error != "":
        fail(jar_error + " (in " + str(ctx.label) + ")")

    pom_files = ctx.attr.pom[DefaultInfo].files.to_list()
    if len(pom_files) != 1:
        fail("maven_deploy " + str(ctx.label) + ": pom " +
             str(ctx.attr.pom.label) + " provides " +
             str(len(pom_files)) + " files, want exactly one .pom")
    pom_file = pom_files[0]
    pom_error = maven_pom_error(pom_file.basename)
    if pom_error != "":
        fail(pom_error + " (in " + str(ctx.label) + ")")

    java = ctx.toolchains["@bazel_tools//tools/jdk:runtime_toolchain_type"].java_runtime
    java_exes = [f for f in java.files.to_list() if f.path.endswith("/bin/java") or f.path.endswith("/bin/java.exe")]
    japicmp = ctx.file._japicmp
    api_checked = ctx.attr.api_compat != "off"
    if api_checked and len(java_exes) != 1:
        fail("maven_deploy " + str(ctx.label) + ": the Java runtime has " + str(len(java_exes)) + " bin/java files, want exactly one")

    values = {
        "api_baseline": ctx.attr.api_baseline,
        "api_compat": ctx.attr.api_compat,
        "artifact": ctx.attr.artifact,
        "japicmp_rloc": rlocation_path(ctx, japicmp) if api_checked else "",
        "java_rloc": rlocation_path(ctx, java_exes[0]) if api_checked else "",
        "group": ctx.attr.group,
        "jar_rloc": rlocation_path(ctx, jar_file),
        "pom_rloc": rlocation_path(ctx, pom_file),
        "version": ctx.attr.version,
    }
    pinned = [jar_file, pom_file]
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
        values[kind + "_rloc"] = rlocation_path(ctx, extra)
        pinned.append(extra)

    runfiles = ctx.runfiles(files = pinned)
    if api_checked:
        runfiles = runfiles.merge(ctx.runfiles(files = [japicmp], transitive_files = java.files))
    return deploy_config(ctx, "maven", values, runfiles)

_maven_config = rule(
    implementation = _maven_config_impl,
    doc = "Writes the deploy config for one Maven deployment.",
    toolchains = ["@bazel_tools//tools/jdk:runtime_toolchain_type"],
    attrs = API_COMPAT_ATTRS | {
        "_japicmp": attr.label(
            default = Label("//deploy/private/pins:japicmp"),
            allow_single_file = True,
        ),
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
        "sources": attr.label(
            doc = "Optional sources jar published as -sources.jar.",
            allow_single_file = True,
        ),
        "version": attr.string(
            doc = "Version the pom must declare, empty to publish the pom's version.",
        ),
    },
)

def maven_deploy(name, jar, pom, group, artifact = "", version = "", build_mode = "release", sources = None, javadoc = None, sbom = None, api_compat = "fail", api_baseline = "registry", size_budget = None, tags = None, visibility = None, testonly = False, target_compatible_with = None):
    """Publishes one jar plus its pom, and optional sources and javadoc jars, as a local-first Maven deployment. api_compat and api_baseline work as described under "Public API compatibility" in the README; the tool is japicmp on the jar."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    group_error = maven_group_error(group)
    if group_error != "":
        fail(group_error + " (in " + native.package_name() + ":" + name + ")")
    artifact, version = deploy_identity(name, artifact, version, maven_artifact_error, maven_version_error)

    api_error = api_compat_error("maven_deploy", api_compat, api_baseline)
    if api_error != "":
        fail(api_error + " (in " + native.package_name() + ":" + name + ")")
    config_target = name + "_config"
    _maven_config(
        name = config_target,
        api_baseline = api_baseline,
        api_compat = api_compat,
        artifact = artifact,
        group = group,
        jar = jar,
        javadoc = javadoc,
        pom = pom,
        sources = sources,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [jar, pom] + ([sources] if sources else []) + ([javadoc] if javadoc else [])) | attrs)
    )
