"""Hackage packages from the resolved sidecar: one repository each and a hub of aliases."""

load("//core:repo.bzl", "hub_repo")

_DEFS = str(Label("//haskell/private:defs.bzl"))

_TOOLCHAINS = "@dx_haskell_toolchains//lib"

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

def is_sha256(value):
    """Returns whether a value is 64 lowercase hex characters."""
    return len(value) == 64 and not value.lstrip("0123456789abcdef")

def pin_problems(sidecar):
    """Returns one message per fetched package whose sdist or cabal revision digest is not a sha256."""
    problems = []
    for name, package in sorted(fetched_packages(sidecar).items()):
        fields = ["sdist_sha256"] + (["cabal_sha256"] if has_revision(package) else [])
        for field in fields:
            value = package.get(field, "")
            if type(value) != "string" or not is_sha256(value):
                problems.append("package {} has {} {}, expected 64 lowercase hex characters".format(name, field, repr(value)))
    return problems

def executables(package):
    """Returns the executable names of a package."""
    return [component[len("exe:"):] for component in package["components"] if component.startswith("exe:")]

def sublibrary_target(package, sublibrary):
    """Returns the target name of one public sublibrary of a package."""
    return "{}-lib-{}".format(package, sublibrary)

def sublibraries(package):
    """Returns the sublibrary names of a package."""
    return [component[len("lib:"):] for component in package["components"] if component.startswith("lib:")]

def dependency_label(hub, sidecar, name):
    """Returns the label a package dependency points at, or None for local packages."""
    name, _, sublibrary = name.partition(":")
    for package in sidecar["packages"]:
        if package["name"] != name:
            continue
        if package["source"] == "ghc":
            return "{}:{}".format(_TOOLCHAINS, name)
        if package["source"] in ("hackage", "url"):
            target = sublibrary_target(name, sublibrary) if sublibrary else name
            return "@{}//:{}".format(repo_name(hub, name), target)
    return None

def tool_labels(hub, package):
    """Returns the executable labels of the build-tool-depends of a package."""
    labels = []
    for tool in package.get("tools", []):
        name, _, exe = tool.partition(":")
        labels.append("@{}//:{}-exe-{}".format(repo_name(hub, name), name, exe))
    return sorted(labels)

def _identifier_char(char):
    return char.isalnum() or char in "-_"

def _words(text):
    words = []
    word = ""
    for char in text.elems():
        if _identifier_char(char):
            word += char
        else:
            if word:
                words.append(word)
            word = ""
    if word:
        words.append(word)
    return words

def _indent(line):
    return len(line) - len(line.lstrip(" \t"))

def _field_words(lines, fields):
    texts = []
    field = None
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("--"):
            continue
        if field != None and _indent(line) > field:
            texts.append(stripped)
            continue
        field = None
        key, colon, value = stripped.partition(":")
        if colon and key.strip().lower() in fields:
            field = _indent(line)
            texts.append(value)
    return _words(" ".join(texts))

def cabal_libraries(text):
    """Returns the library stanzas of a .cabal file by name, "" for the main library.

    Args:
        text: The .cabal file content.

    Returns:
        A dict from library name to a dict with "deps", the sibling libraries it builds on,
        and "empty", whether it and its imported common stanzas list no modules.
    """
    stanzas = {}
    commons = {}
    current = None
    for line in text.splitlines():
        if line.strip() and _indent(line) == 0 and not line.startswith("--"):
            words = [word for word in line.replace("\t", " ").split(" ") if word]
            kind = words[0].lower()
            current = None
            if kind == "library":
                current = []
                stanzas[words[1] if len(words) > 1 else ""] = current
            elif kind == "common" and len(words) > 1:
                current = []
                commons[words[1]] = current
        elif current != None:
            current.append(line)
    named = [name for name in stanzas if name]
    libraries = {}
    for name, lines in stanzas.items():
        imported = [line for common in _field_words(lines, ["import"]) for line in commons.get(common, [])]
        depends = _field_words(lines, ["build-depends"])
        libraries[name] = {
            "deps": sorted([sibling for sibling in named if sibling != name and sibling in depends]),
            "empty": not _field_words(lines + imported, ["exposed-modules", "other-modules"]),
        }
    return libraries

def _target(kind, name, attrs, deps, tools):
    attrs = attrs + ([
        "tools = " + repr(tools),
    ] if tools else []) + [
        'srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel"])',
        "deps = " + repr(deps),
        "target_compatible_with = HASKELL_COMPATIBLE_WITH",
        "verbose = False",
    ]
    return "{}(\n    name = \"{}\",\n{}\n)\n".format(kind, name, "".join(["    {},\n".format(attr) for attr in attrs]).rstrip("\n"))

def _cabal_args(library):
    return ['cabal_args = ":empty_library"'] if library.get("empty") else []

def package_deps(hub, sidecar, package):
    """Returns the dependency labels of one package."""
    return sorted([dep for dep in [dependency_label(hub, sidecar, dep) for dep in package["dependencies"]] if dep])

def package_build(name, version, components, deps, tools, libraries):
    """Returns the BUILD file of one package repository.

    Args:
        name: The package name.
        version: The package version.
        components: The components of the package in the lock.
        deps: The dependency labels of the package.
        tools: The build tool labels of the package.
        libraries: The library stanzas of the .cabal file, as returned by cabal_libraries.

    Returns:
        The BUILD file content.
    """
    has_lib = "lib" in components
    sublibs = sorted([sub for sub in libraries if sub])
    lines = [
        'load("@rules_haskell//haskell:cabal.bzl", "haskell_cabal_args", "haskell_cabal_binary", "haskell_cabal_library")',
        'load("{}", "HASKELL_COMPATIBLE_WITH")'.format(_DEFS),
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
    ]
    if [library for library in libraries.values() if library["empty"]]:
        lines.append('haskell_cabal_args(\n    name = "empty_library",\n    is_empty = True,\n)\n')
    common = ['package_name = "{}"'.format(name), 'version = "{}"'.format(version), "haddock = False"]
    for sub in sublibs:
        sub_deps = deps + [":" + sublibrary_target(name, sibling) for sibling in libraries[sub]["deps"]]
        lines.append(_target("haskell_cabal_library", sublibrary_target(name, sub), common + ['sublibrary_name = "{}"'.format(sub)] + _cabal_args(libraries[sub]), sub_deps, tools))
    if has_lib:
        lib_deps = deps + [":" + sublibrary_target(name, sibling) for sibling in libraries.get("", {}).get("deps", [])]
        lines.append(_target("haskell_cabal_library", name, common + _cabal_args(libraries.get("", {})), lib_deps, tools))
    for exe in executables({"components": components}):
        exe_deps = deps + ([":" + name] if has_lib else []) + [":" + sublibrary_target(name, sub) for sub in sublibs]
        lines.append(_target("haskell_cabal_binary", "{}-exe-{}".format(name, exe), ['exe_name = "{}"'.format(exe)], exe_deps, tools))
    return "\n".join(lines)

def hub_build(hub, sidecar):
    """Returns the hub BUILD file with one alias per package and per executable."""
    lines = ['package(default_visibility = ["//visibility:public"])', ""]
    for name, package in sorted(fetched_packages(sidecar).items()):
        repo = repo_name(hub, name)
        if "lib" in package["components"]:
            lines.append('alias(name = "{name}", actual = "@{repo}//:{name}")\n'.format(name = name, repo = repo))
        for sub in sublibraries(package):
            target = sublibrary_target(name, sub)
            lines.append('alias(name = "{target}", actual = "@{repo}//:{target}")\n'.format(target = target, repo = repo))
        for exe in executables(package):
            target = "{}-exe-{}".format(name, exe)
            lines.append('alias(name = "{target}", actual = "@{repo}//:{target}")\n'.format(target = target, repo = repo))
    return "\n".join(lines)

def _hackage_package_impl(rctx):
    pins = {"sha256": rctx.attr.sha256}
    if rctx.attr.cabal_url:
        pins["cabal_sha256"] = rctx.attr.cabal_sha256
    for field, value in pins.items():
        if not is_sha256(value):
            fail("package {} has {} {}, expected 64 lowercase hex characters".format(rctx.attr.package, field, repr(value)))
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
    libraries = cabal_libraries(rctx.read("{}.cabal".format(rctx.attr.package)))
    rctx.file("BUILD.bazel", package_build(rctx.attr.package, rctx.attr.version, rctx.attr.components, rctx.attr.deps, rctx.attr.tools, libraries))
    return rctx.repo_metadata(reproducible = True)

hackage_package = repository_rule(
    implementation = _hackage_package_impl,
    doc = "Fetches one Hackage sdist, replaces its .cabal file with the pinned revision and writes its BUILD file.",
    attrs = {
        "cabal_sha256": attr.string(doc = "Digest of the pinned .cabal revision."),
        "cabal_url": attr.string(doc = "URL of the pinned .cabal revision. Empty for revision 0."),
        "components": attr.string_list(mandatory = True, doc = "Components of the package in the lock."),
        "deps": attr.string_list(doc = "Dependency labels of the package."),
        "package": attr.string(mandatory = True, doc = "Package name."),
        "sha256": attr.string(mandatory = True, doc = "Digest of the sdist."),
        "tools": attr.string_list(doc = "Build tool labels of the package."),
        "url": attr.string(mandatory = True, doc = "Sdist URL."),
        "version": attr.string(mandatory = True, doc = "Package version."),
    },
)

def declare_packages(mctx, tag):
    """Declares the repositories of one cabal.lock tag.

    Args:
        mctx: The module extension context.
        tag: The packages tag with name and deps.

    Returns:
        The names of the repositories created.
    """
    sidecar = json.decode(mctx.read(tag.lock))
    if sidecar.get("version") != 1:
        fail("{} has sidecar version {}, expected 1".format(tag.lock, sidecar.get("version")))
    problems = pin_problems(sidecar)
    if problems:
        fail("{}: {}".format(tag.lock, "; ".join(problems)))
    created = [tag.name]
    for name, package in sorted(fetched_packages(sidecar).items()):
        revised = has_revision(package)
        hackage_package(
            name = repo_name(tag.name, name),
            components = package["components"],
            deps = package_deps(tag.name, sidecar, package),
            cabal_sha256 = package["cabal_sha256"] if revised else "",
            cabal_url = cabal_url(package) if revised else "",
            package = name,
            sha256 = package["sdist_sha256"],
            tools = tool_labels(tag.name, package),
            url = sdist_url(package),
            version = package["version"],
        )
        created.append(repo_name(tag.name, name))
    hub_repo(name = tag.name, files = {"BUILD.bazel": hub_build(tag.name, sidecar)})
    return created
