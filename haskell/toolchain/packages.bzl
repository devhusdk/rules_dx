"""Hackage packages from the resolved sidecar: one repository each and a hub of aliases."""

load("//libs/starlark:repo.bzl", "hub_repo")

_DEFS = str(Label("//haskell/rules:defs.bzl"))

_TOOLCHAINS = "@dx_haskell_toolchains//"

def hackage_url(package):
    """Returns the sdist URL of a sidecar package."""
    name = "{}-{}".format(package["name"], package["version"])
    return "https://hackage.haskell.org/package/{}/{}.tar.gz".format(name, name)

def cabal_url(package):
    """Returns the URL of the pinned .cabal revision of a sidecar package."""
    return "https://hackage.haskell.org/package/{}-{}/revision/{}.cabal".format(
        package["name"],
        package["version"],
        package["cabal_revision"],
    )

def repo_name(hub, package):
    """Returns the repository name of one package."""
    return "{}_{}".format(hub, package)

def fetched_packages(sidecar):
    """Returns the packages of a parsed sidecar that are fetched as sdists (Hackage or a tarball URL) by name."""
    return {
        package["name"]: package
        for package in sidecar["packages"]
        if package["source"] in ("hackage", "url")
    }

def sdist_url(package):
    """Returns the sdist URL of a fetched sidecar package."""
    return package["url"] if package["source"] == "url" else hackage_url(package)

def has_revision(package):
    """Returns whether a fetched package carries a pinned cabal revision above 0."""
    return package["source"] == "hackage" and package["cabal_revision"] > 0

def executables(package):
    """Returns the executable names of a package."""
    return [component[len("exe:"):] for component in package["components"] if component.startswith("exe:")]

def dependency_label(hub, sidecar, name):
    """Returns the label a package dependency points at, or None for local packages."""
    for package in sidecar["packages"]:
        if package["name"] != name:
            continue
        if package["source"] == "ghc":
            return "{}:{}".format(_TOOLCHAINS, name)
        if package["source"] in ("hackage", "url"):
            return "@{}//:{}".format(repo_name(hub, name), name)
    return None

def _target(kind, name, attrs, deps):
    attrs = attrs + [
        'srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel"])',
        "deps = " + repr(deps),
        "target_compatible_with = HASKELL_COMPATIBLE_WITH",
        "verbose = False",
    ]
    return "{}(\n    name = \"{}\",\n{}\n)\n".format(kind, name, "".join(["    {},\n".format(attr) for attr in attrs]).rstrip("\n"))

def package_build(hub, sidecar, package):
    """Returns the BUILD file of one package repository."""
    name = package["name"]
    deps = [dependency_label(hub, sidecar, dep) for dep in package["dependencies"]]
    deps = sorted([dep for dep in deps if dep])
    has_lib = "lib" in package["components"]
    lines = [
        'load("@rules_haskell//haskell:cabal.bzl", "haskell_cabal_binary", "haskell_cabal_library")',
        'load("{}", "HASKELL_COMPATIBLE_WITH")'.format(_DEFS),
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
    ]
    if has_lib:
        attrs = ['package_name = "{}"'.format(name), 'version = "{}"'.format(package["version"]), "haddock = False"]
        lines.append(_target("haskell_cabal_library", name, attrs, deps))
    for exe in executables(package):
        exe_deps = deps + [":" + name] if has_lib else deps
        lines.append(_target("haskell_cabal_binary", "{}-exe-{}".format(name, exe), ['exe_name = "{}"'.format(exe)], exe_deps))
    return "\n".join(lines)

def hub_build(hub, sidecar):
    """Returns the hub BUILD file with one alias per package and per executable."""
    lines = ['package(default_visibility = ["//visibility:public"])', ""]
    for name, package in sorted(fetched_packages(sidecar).items()):
        repo = repo_name(hub, name)
        if "lib" in package["components"]:
            lines.append('alias(name = "{name}", actual = "@{repo}//:{name}")\n'.format(name = name, repo = repo))
        for exe in executables(package):
            target = "{}-exe-{}".format(name, exe)
            lines.append('alias(name = "{target}", actual = "@{repo}//:{target}")\n'.format(target = target, repo = repo))
    return "\n".join(lines)

def _hackage_package_impl(rctx):
    rctx.download_and_extract(
        url = [rctx.attr.url],
        sha256 = rctx.attr.sha256,
        stripPrefix = "{}-{}".format(rctx.attr.package, rctx.attr.version),
    )
    if rctx.attr.cabal_url:
        rctx.download(
            url = [rctx.attr.cabal_url],
            output = "{}.cabal".format(rctx.attr.package),
            sha256 = rctx.attr.cabal_sha256,
        )
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", rctx.attr.build)
    return rctx.repo_metadata(reproducible = True)

hackage_package = repository_rule(
    implementation = _hackage_package_impl,
    doc = "Fetches one Hackage sdist, replaces its .cabal file with the pinned revision and writes its BUILD file.",
    attrs = {
        "build": attr.string(mandatory = True, doc = "Content of the BUILD file."),
        "cabal_sha256": attr.string(doc = "Digest of the pinned .cabal revision."),
        "cabal_url": attr.string(doc = "URL of the pinned .cabal revision. Empty for revision 0."),
        "package": attr.string(mandatory = True, doc = "Package name."),
        "sha256": attr.string(mandatory = True, doc = "Digest of the sdist."),
        "url": attr.string(mandatory = True, doc = "Sdist URL."),
        "version": attr.string(mandatory = True, doc = "Package version."),
    },
)

def declare_packages(mctx, tag):
    """Declares the repositories of one haskell.lock tag.

    Args:
        mctx: The module extension context.
        tag: The packages tag with name and deps.

    Returns:
        The names of the repositories created.
    """
    sidecar = json.decode(mctx.read(tag.lock))
    if sidecar.get("version") != 1:
        fail("{} has sidecar version {}, expected 1".format(tag.lock, sidecar.get("version")))
    created = [tag.name]
    for name, package in sorted(fetched_packages(sidecar).items()):
        revised = has_revision(package)
        hackage_package(
            name = repo_name(tag.name, name),
            build = package_build(tag.name, sidecar, package),
            cabal_sha256 = package["cabal_sha256"] if revised else "",
            cabal_url = cabal_url(package) if revised else "",
            package = name,
            sha256 = package["sdist_sha256"],
            url = sdist_url(package),
            version = package["version"],
        )
        created.append(repo_name(tag.name, name))
    hub_repo(name = tag.name, files = {"BUILD.bazel": hub_build(tag.name, sidecar)})
    return created
