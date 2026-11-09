"""Local-first Helm chart publisher for dx deploy."""

load("//helm/rules:providers.bzl", "HelmChartInfo")
load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deployment_budget", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":oci.bzl", "oci_registry_error", "oci_repository_error")
load(":validate.bzl", "VERSION_CHARS", "only_chars")

def helm_version_error(version):
    """Validates one chart release version value."""
    if type(version) != "string" or version == "":
        return ("helm_chart_deploy: invalid version '" + str(version) +
                "': want the Chart.yaml version (for example '1.2.3')")
    if not only_chars(version, VERSION_CHARS):
        return ("helm_chart_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._+-]")
    return ""

def _helm_config_impl(ctx):
    """Writes the deploy config for one Helm chart deployment."""
    info = ctx.attr.chart[HelmChartInfo]
    label = ctx.attr.chart.label
    prefix = (label.repo_name or ctx.workspace_name) + "/" + (label.package + "/" if label.package else "")
    files = []
    for f in ctx.attr.chart[DefaultInfo].files.to_list():
        rloc = rlocation_path(ctx, f)
        if not rloc.startswith(prefix):
            fail("helm_chart_deploy " + str(ctx.label) + ": chart file " + rloc + " is outside " + prefix)
        files.append({"path": rloc[len(prefix):], "rloc": rloc})
    if "Chart.yaml" not in [f["path"] for f in files]:
        fail("helm_chart_deploy " + str(ctx.label) + ": chart " + str(label) + " has no Chart.yaml")
    helm = ctx.file.helm
    return deploy_config(
        ctx,
        "helm_chart",
        {
            "deploy_name": ctx.attr.deploy_name,
            "files": files,
            "helm_rloc": rlocation_path(ctx, helm),
            "registry": ctx.attr.registry,
            "repository": ctx.attr.repository,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = [helm] + ctx.attr.chart[DefaultInfo].files.to_list() + info.subcharts),
    )

_helm_config = rule(
    implementation = _helm_config_impl,
    doc = "Writes the deploy config for one Helm chart deployment.",
    attrs = {
        "chart": attr.label(
            doc = "The helm_chart to package and push.",
            mandatory = True,
            providers = [HelmChartInfo],
        ),
        "deploy_name": attr.string(doc = "Deploy target name.", mandatory = True),
        "helm": attr.label(
            doc = "Pinned helm executable.",
            allow_single_file = True,
            cfg = "exec",
            mandatory = True,
        ),
        "registry": attr.string(doc = "OCI registry host plus optional path.", mandatory = True),
        "repository": attr.string(doc = "Path under the registry, empty for none.", mandatory = True),
        "version": attr.string(doc = "Release version Chart.yaml must declare, empty to publish Chart.yaml's version."),
    },
)

def helm_chart_deploy(name, chart, registry, version = "", repository = "", helm = None, build_mode = "release", sbom = None, size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Packages a helm_chart and pushes it to an OCI registry as a local-first deployment."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    where = " (in " + native.package_name() + ":" + name + ")"
    if version != "":
        version_error = helm_version_error(version)
        if version_error != "":
            fail(version_error + where)
    registry_error = oci_registry_error(registry)
    if registry_error != "":
        fail(registry_error.replace("oci_deploy", "helm_chart_deploy") + where)
    if repository != "":
        repository_error = oci_repository_error(repository)
        if repository_error != "":
            fail(repository_error.replace("oci_deploy", "helm_chart_deploy") + where)

    config_target = name + "_config"
    _helm_config(
        name = config_target,
        chart = chart,
        deploy_name = name,
        helm = helm or str(Label("//config:tool_helm")),
        registry = registry,
        repository = repository,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [chart]) | attrs)
    )
