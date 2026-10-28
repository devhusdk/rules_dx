"""Kubernetes manifest ownership and schema staging for dx lint."""

load("//quality:native_config.bzl", "DxNativeConfigInfo")
load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources")

def _version_error(version):
    parts = version.split(".")
    if len(parts) != 3:
        return "kubernetes_version '" + version + "' must look like X.Y.Z"
    for part in parts:
        if part == "" or not part.isdigit():
            return "kubernetes_version '" + version + "' must look like X.Y.Z"
    return ""

def _kubernetes_manifests_impl(ctx):
    direct_sources = {}
    if len(ctx.files.srcs) > 0:
        direct_sources["kubernetes"] = depset(ctx.files.srcs)
    check_direct_sources(direct_sources, str(ctx.label))
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]

kubernetes_manifests = rule(
    implementation = _kubernetes_manifests_impl,
    doc = "Owns the Kubernetes manifests of one package for dx lint (kubeconform).",
    attrs = {
        "srcs": attr.label_list(
            allow_files = [".yaml", ".yml", ".json"],
            doc = "Kubernetes manifest files of this package; every document needs apiVersion and kind.",
        ),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Other manifest libraries this package depends on."),
    },
)

def _crd_name_error(basename):
    stem = basename[:-len(".json")] if basename.endswith(".json") else ""
    if stem == "" or "_" not in stem:
        return "CRD schema '" + basename + "' must be named <kind>_<version>.json"
    kind, version = stem.rsplit("_", 1)
    if kind == "" or version == "":
        return "CRD schema '" + basename + "' must be named <kind>_<version>.json"
    return ""

def _kubernetes_schemas_impl(ctx):
    version_error = _version_error(ctx.attr.kubernetes_version)
    if version_error != "":
        fail("kubernetes_schemas " + str(ctx.label) + ": " + version_error)
    root = ctx.label.name + ".kubeconform"
    links = []
    seen = {}
    for f in ctx.files.schemas:
        if f.basename in seen:
            fail("kubernetes_schemas " + str(ctx.label) + ": duplicate schema basename '" + f.basename + "'")
        seen[f.basename] = True
        link = ctx.actions.declare_file(root + "/core/" + f.basename)
        ctx.actions.symlink(output = link, target_file = f)
        links.append(link)
    seen_crd = {}
    for f, group in ctx.attr.crd_schemas.items():
        file = f.files.to_list()[0]
        error = _crd_name_error(file.basename)
        if error != "":
            fail("kubernetes_schemas " + str(ctx.label) + ": " + error)
        if group == "" or "/" in group:
            fail("kubernetes_schemas " + str(ctx.label) + ": CRD API group '" + group + "' must be a non-empty group name")
        key = group + "/" + file.basename
        if key in seen_crd:
            fail("kubernetes_schemas " + str(ctx.label) + ": duplicate CRD schema '" + key + "'")
        seen_crd[key] = True
        link = ctx.actions.declare_file(root + "/crd/" + group + "/" + file.basename)
        ctx.actions.symlink(output = link, target_file = file)
        links.append(link)
    config = ctx.actions.declare_file(ctx.label.name + ".kubeconform.json")
    ctx.actions.write(config, json.encode({
        "core_dir": root + "/core",
        "crd_dir": root + "/crd",
        "kubernetes_version": ctx.attr.kubernetes_version,
        "schema_version": 1,
        "skip_kinds": sorted(ctx.attr.skip_kinds),
        "strict": ctx.attr.strict,
    }) + "\n")
    closure = depset([config] + links)
    return [
        DefaultInfo(files = closure),
        DxNativeConfigInfo(
            closure = closure,
            config = config,
            tool_id = "kubeconform",
        ),
    ]

kubernetes_schemas = rule(
    implementation = _kubernetes_schemas_impl,
    doc = "Stages offline JSON schemas and the kubeconform settings for dx lint.",
    attrs = {
        "crd_schemas": attr.label_keyed_string_dict(
            allow_files = [".json"],
            doc = "CRD schema files named <kind>_<version>.json mapped to their API group.",
            default = {},
        ),
        "kubernetes_version": attr.string(
            mandatory = True,
            doc = "Kubernetes version the core schemas describe, as X.Y.Z.",
        ),
        "schemas": attr.label_list(
            allow_files = [".json"],
            doc = "Core schema files named like kubernetes-json-schema, such as deployment-apps-v1.json.",
            default = [],
        ),
        "skip_kinds": attr.string_list(doc = "Resource kinds kubeconform skips.", default = []),
        "strict": attr.bool(doc = "Rejects unknown fields and duplicate keys.", default = True),
    },
)
