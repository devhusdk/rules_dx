"""terraform: Terraform providers vendored with sha256 integrity."""

load("//core:pins.bzl", "lock_problems", "lock_tag")
load("//core:repo.bzl", "hub_repo")

TARGETS = {
    "darwin_amd64": ("@platforms//os:macos", "@platforms//cpu:x86_64"),
    "darwin_arm64": ("@platforms//os:macos", "@platforms//cpu:arm64"),
    "linux_amd64": ("@platforms//os:linux", "@platforms//cpu:x86_64"),
    "linux_arm64": ("@platforms//os:linux", "@platforms//cpu:arm64"),
    "windows_amd64": ("@platforms//os:windows", "@platforms//cpu:x86_64"),
    "windows_arm64": ("@platforms//os:windows", "@platforms//cpu:arm64"),
}

PLATFORM_TARGETS = {
    "linux_x86_64": "linux_amd64",
    "linux_arm64": "linux_arm64",
    "macos_x86_64": "darwin_amd64",
    "macos_arm64": "darwin_arm64",
    "windows_x86_64": "windows_amd64",
    "windows_arm64": "windows_arm64",
}

def _repo_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.urls,
        sha256 = ctx.attr.sha256,
        output = "%s/%s/%s" % (ctx.attr.source, ctx.attr.version, ctx.attr.target),
    )
    ctx.file("BUILD.bazel", """load("@rules_dx//terraform:defs.bzl", "terraform_provider")

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
        "urls": attr.string_list(mandatory = True),
        "version": attr.string(mandatory = True),
    },
)

def hub_build(providers):
    """Returns the hub BUILD file aliasing each provider to its platform repository."""
    lines = ['load("@rules_dx//terraform:defs.bzl", "terraform_provider")', "", 'package(default_visibility = ["//visibility:public"])', ""]
    for target, (os, cpu) in sorted(TARGETS.items()):
        lines.append('config_setting(name = "%s", constraint_values = ["%s", "%s"])' % (target, os, cpu))
    lines.append("")
    for name, entry in sorted(providers.items()):
        branches = ["        \":%s\": \"@%s//:provider\"," % (t, repo) for t, repo in sorted(entry["repos"].items())]
        lines.append("alias(\n    name = \"%s\",\n    actual = select({\n%s\n    }, no_match_error = \"provider %s has no artifact for this execution platform\"),\n)\n" % (name, "\n".join(branches), entry["source"]))
    return "\n".join(lines)

def provider_repos(hub, lock):
    """Returns the platform repository attributes of every package of a decoded lock, by repository name."""
    repos = {}
    for package in lock["packages"]:
        for archive in package["archives"]:
            platform = archive.get("platform")
            if platform not in PLATFORM_TARGETS:
                fail("terraform %s %s: archive without a platform" % (package["name"], package["version"]))
            target = PLATFORM_TARGETS[platform]
            repos["%s_%s_%s" % (hub, package["name"].rpartition("/")[2], target)] = {
                "sha256": archive["sha256"],
                "source": package["name"],
                "target": target,
                "urls": archive["urls"],
                "version": package["version"],
            }
    return repos

def hub_providers(hub, lock):
    """Returns the hub entries of a decoded lock: provider type to its source and repository per target."""
    providers = {}
    for package in lock["packages"]:
        kind = package["name"].rpartition("/")[2]
        if kind in providers:
            fail("terraform hub %r: %s and %s are both named %s" % (hub, providers[kind]["source"], package["name"], kind))
        repos = {}
        for archive in package["archives"]:
            target = PLATFORM_TARGETS[archive["platform"]]
            repos[target] = "%s_%s_%s" % (hub, kind, target)
        providers[kind] = {"repos": repos, "source": package["name"]}
    return providers

def _extension_impl(mctx):
    direct = []
    dev = []
    seen = {}
    for module in mctx.modules:
        for tag in module.tags.lock:
            if tag.name in seen:
                fail("terraform hub %r is declared by both %s and %s" % (tag.name, seen[tag.name], module.name))
            seen[tag.name] = module.name
            lock = json.decode(mctx.read(tag.lock))
            problems = lock_problems(lock)
            if problems:
                fail("%s %s" % (tag.lock, "; ".join(problems)))
            for name, attrs in sorted(provider_repos(tag.name, lock).items()):
                _provider_repo(name = name, **attrs)
            hub_repo(name = tag.name, files = {"BUILD.bazel": hub_build(hub_providers(tag.name, lock))})
            if module.is_root:
                if mctx.is_dev_dependency(tag):
                    dev.append(tag.name)
                else:
                    direct.append(tag.name)
    return mctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = direct,
        root_module_direct_dev_deps = dev,
    )

terraform = module_extension(
    doc = "Builds one hub of Terraform providers per lock tag.",
    implementation = _extension_impl,
    tag_classes = {"lock": lock_tag},
)
