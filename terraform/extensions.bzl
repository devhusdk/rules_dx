"""dx_terraform: Terraform providers vendored with sha256 integrity.

Each declared provider/target pair becomes a repository that unpacks the
official provider zip into Terraform's unpacked filesystem-mirror layout. The
sha256 is the hash of the zip, the same value as the `zh:` lock file entry.
Nothing resolves over the network except the pinned download.
"""

TARGETS = {
    "darwin_arm64": ("@platforms//os:macos", "@platforms//cpu:arm64"),
    "linux_amd64": ("@platforms//os:linux", "@platforms//cpu:x86_64"),
    "linux_arm64": ("@platforms//os:linux", "@platforms//cpu:arm64"),
    "windows_amd64": ("@platforms//os:windows", "@platforms//cpu:x86_64"),
    "windows_arm64": ("@platforms//os:windows", "@platforms//cpu:arm64"),
}

_HASHICORP_HOST = "registry.terraform.io/hashicorp/"

def _is_hex64(value):
    if len(value) != 64:
        return False
    for c in value.elems():
        if c not in "0123456789abcdef":
            return False
    return True

def provider_errors(source, version, sha256, urls):
    """Validates one provider declaration.

    Args:
      source: Provider source address HOST/NAMESPACE/TYPE.
      version: Provider version.
      sha256: dict from target to hex digest.
      urls: dict from target to download URL.

    Returns:
      A list of error strings.
    """
    errors = []
    if len(source.split("/")) != 3:
        errors.append("source %r must be HOST/NAMESPACE/TYPE" % source)
    if version == "":
        errors.append("%s: version is required" % source)
    if len(sha256) == 0:
        errors.append("%s: sha256 must name at least one target" % source)
    for target, digest in sha256.items():
        if target not in TARGETS:
            errors.append("%s: unknown target %r, expected one of %s" % (source, target, sorted(TARGETS)))
        if not _is_hex64(digest):
            errors.append("%s: sha256 for %s must be 64 lowercase hex characters" % (source, target))
        if target not in urls and not source.startswith(_HASHICORP_HOST):
            errors.append("%s: urls is required for target %s" % (source, target))
    for target in urls:
        if target not in sha256:
            errors.append("%s: url for %s has no sha256" % (source, target))
    return errors

def default_url(source, version, target):
    kind = source.split("/")[-1]
    return "https://releases.hashicorp.com/terraform-provider-{k}/{v}/terraform-provider-{k}_{v}_{t}.zip".format(k = kind, v = version, t = target)

def _repo_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.url,
        sha256 = ctx.attr.sha256,
        output = "%s/%s/%s" % (ctx.attr.source, ctx.attr.version, ctx.attr.target),
    )
    ctx.file("BUILD.bazel", """load("@rules_dx//terraform/rules:providers.bzl", "terraform_provider")

terraform_provider(
    name = "provider",
    srcs = glob(["**/*"], exclude = ["BUILD.bazel", "WORKSPACE", "REPO.bazel", "MODULE.bazel"]),
    source = {source},
    target = {target},
    version = {version},
    visibility = ["//visibility:public"],
)
""".format(source = repr(ctx.attr.source), target = repr(ctx.attr.target), version = repr(ctx.attr.version)))
    return ctx.repo_metadata(reproducible = True)

_provider_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {
        "sha256": attr.string(mandatory = True),
        "source": attr.string(mandatory = True),
        "target": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
        "version": attr.string(mandatory = True),
    },
)

def _hub_impl(ctx):
    providers = json.decode(ctx.attr.providers)
    lines = ['load("@rules_dx//terraform/rules:providers.bzl", "terraform_provider")', "", 'package(default_visibility = ["//visibility:public"])', ""]
    for target, (os, cpu) in sorted(TARGETS.items()):
        lines.append('config_setting(name = "%s", constraint_values = ["%s", "%s"])' % (target, os, cpu))
    lines.append("")
    for name, entry in sorted(providers.items()):
        branches = ["        \":%s\": \"@%s//:provider\"," % (t, repo) for t, repo in sorted(entry["repos"].items())]
        lines.append("alias(\n    name = \"%s\",\n    actual = select({\n%s\n    }, no_match_error = \"provider %s has no artifact for this execution platform\"),\n)\n" % (name, "\n".join(branches), entry["source"]))
    ctx.file("BUILD.bazel", "\n".join(lines))
    return ctx.repo_metadata(reproducible = True)

_hub_repo = repository_rule(
    implementation = _hub_impl,
    attrs = {"providers": attr.string(mandatory = True)},
)

def _extension_impl(ctx):
    decls = {}
    for module in ctx.modules:
        for tag in module.tags.provider:
            errors = provider_errors(tag.source, tag.version, tag.sha256, tag.urls)
            if errors:
                fail("dx_terraform: " + "; ".join(errors))
            decl = (tag.source, tag.version, tag.sha256, tag.urls)
            if tag.name in decls and decls[tag.name] != decl:
                fail("dx_terraform: conflicting declarations of provider %r" % tag.name)
            decls[tag.name] = decl
    hub = {}
    for name, (source, version, sha256, urls) in sorted(decls.items()):
        repos = {}
        for target, digest in sorted(sha256.items()):
            repo = "dx_terraform_%s_%s" % (name, target)
            _provider_repo(
                name = repo,
                sha256 = digest,
                source = source,
                target = target,
                url = urls.get(target, default_url(source, version, target)),
                version = version,
            )
            repos[target] = repo
        hub[name] = {"repos": repos, "source": source}
    _hub_repo(name = "dx_terraform_providers", providers = json.encode(hub))

_provider = tag_class(attrs = {
    "name": attr.string(mandatory = True, doc = "Target name in @dx_terraform_providers."),
    "sha256": attr.string_dict(mandatory = True, doc = "Target platform to hex sha256 of the official zip."),
    "source": attr.string(mandatory = True, doc = "Provider source address HOST/NAMESPACE/TYPE."),
    "urls": attr.string_dict(doc = "Target platform to zip URL; optional for registry.terraform.io/hashicorp."),
    "version": attr.string(mandatory = True, doc = "Provider version."),
})

dx_terraform = module_extension(
    implementation = _extension_impl,
    tag_classes = {"provider": _provider},
)
