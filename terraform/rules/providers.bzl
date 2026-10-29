"""Terraform provider mirror providers and rule."""

TerraformProviderInfo = provider(
    doc = "One Terraform provider in unpacked filesystem-mirror layout.",
    fields = {
        "files": "depset of files of the provider tree.",
        "mirror_root": "Path of the mirror root (the -plugin-dir value), relative to the execroot.",
        "source": "Provider source address, for example registry.terraform.io/hashicorp/null.",
        "target": "Terraform target platform, for example linux_amd64.",
        "version": "Provider version.",
    },
)

TerraformModuleInfo = provider(
    doc = "Providers a Terraform module needs, including those of its deps.",
    fields = {
        "transitive_providers": "depset of TerraformProviderInfo.",
    },
)

def provider_mirror_root(path, source, version, target):
    """Strips the mirror layout suffix from the path of a provider executable.

    Args:
      path: Path of the provider executable.
      source: Provider source address (HOST/NS/TYPE).
      version: Provider version.
      target: Terraform target platform.

    Returns:
      The mirror root, or None when the path does not follow the layout.
    """
    marker = "/%s/%s/%s/" % (source, version, target)
    index = path.find(marker)
    if index < 0:
        return None
    return path[:index]

def _terraform_provider_impl(ctx):
    kind = ctx.attr.source.split("/")[-1]
    executables = [
        f
        for f in ctx.files.srcs
        if f.basename.startswith("terraform-provider-" + kind) and not f.basename.endswith(".sig")
    ]
    if len(executables) != 1:
        fail("%s: expected exactly one terraform-provider-%s executable, found %d" % (ctx.label, kind, len(executables)))
    root = provider_mirror_root(executables[0].path, ctx.attr.source, ctx.attr.version, ctx.attr.target)
    if root == None:
        fail("%s: %s is not under %s/%s/%s" % (ctx.label, executables[0].path, ctx.attr.source, ctx.attr.version, ctx.attr.target))
    files = depset(ctx.files.srcs)
    return [
        DefaultInfo(files = files),
        TerraformProviderInfo(
            source = ctx.attr.source,
            version = ctx.attr.version,
            target = ctx.attr.target,
            files = files,
            mirror_root = root,
        ),
    ]

terraform_provider = rule(
    implementation = _terraform_provider_impl,
    doc = "One Terraform provider unpacked into the filesystem-mirror layout.",
    attrs = {
        "source": attr.string(mandatory = True, doc = "Provider source address HOST/NAMESPACE/TYPE."),
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Files of the unpacked mirror tree."),
        "target": attr.string(mandatory = True, doc = "Terraform target platform."),
        "version": attr.string(mandatory = True, doc = "Provider version."),
    },
)
