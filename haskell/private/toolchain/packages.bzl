"""Hackage packages from a cabal lock file: one repository each and a hub of aliases."""

load("//core:pins.bzl", "lock_problems")
load("//core:repo.bzl", "hub_repo")

_DEFS = str(Label("//haskell/private:defs.bzl"))

_TOOLCHAINS = "@dx_haskell_toolchains//lib"

def cabal_url(package):
    """Returns the URL of the pinned .cabal revision of a lock package."""
    return "https://hackage.haskell.org/package/{}-{}/revision/{}.cabal".format(
        package["name"],
        package["version"],
        package["extra"]["cabal_revision"]["number"],
    )

def repo_name(hub, package):
    """Returns the repository name of one package."""
    return "{}_{}".format(hub, package)

def has_revision(package):
    """Returns whether a lock package carries a pinned cabal revision above 0."""
    return package.get("extra", {}).get("cabal_revision", {}).get("number", 0) > 0

def is_sha256(value):
    """Returns whether a value is 64 lowercase hex characters."""
    return len(value) == 64 and not value.lstrip("0123456789abcdef")

def pin_problems(lock):
    """Returns one message per lock package without one sdist archive or whose cabal revision digest is not a sha256."""
    problems = lock_problems(lock)
    for package in lock.get("packages", []):
        name = package.get("name", "")
        if len(package.get("archives", [])) != 1:
            problems.append("package {} has {} archives, expected 1".format(name, len(package.get("archives", []))))
        if has_revision(package):
            value = package["extra"]["cabal_revision"].get("sha256", "")
            if type(value) != "string" or not is_sha256(value):
                problems.append("package {} has cabal revision sha256 {}, expected 64 lowercase hex characters".format(name, repr(value)))
    return problems

def by_name(lock):
    """Returns the packages of a lock by name."""
    return {package["name"]: package for package in lock["packages"]}

def components(package):
    """Returns the cabal components of a lock package."""
    return package.get("extra", {}).get("cabal_components", [])

def executables(package):
    """Returns the executable names of a package."""
    return [component[len("exe:"):] for component in components(package) if component.startswith("exe:")]

def sublibrary_target(package, sublibrary):
    """Returns the target name of one public sublibrary of a package."""
    return "{}-lib-{}".format(package, sublibrary)

def sublibraries(package):
    """Returns the sublibrary names of a package."""
    return [component[len("lib:"):] for component in components(package) if component.startswith("lib:")]

def dependency_label(hub, names, name):
    """Returns the label a package dependency points at: a lock package, else a GHC boot package."""
    name, _, sublibrary = name.partition(":")
    if name in names:
        target = sublibrary_target(name, sublibrary) if sublibrary else name
        return "@{}//:{}".format(repo_name(hub, name), target)
    return "{}:{}".format(_TOOLCHAINS, name)

def tool_labels(hub, package):
    """Returns the executable labels of the build-tool-depends of a package."""
    labels = []
    for tool in package.get("extra", {}).get("cabal_tools", []):
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
    """Returns the library stanzas of a .cabal file by name, "" for the main library."""
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

def package_deps(hub, lock, package):
    """Returns the dependency labels of one package."""
    names = by_name(lock)
    return sorted([dependency_label(hub, names, dep) for dep in package.get("extra", {}).get("cabal_deps", [])])

def package_build(name, version, components, deps, tools, libraries):
    """Returns the BUILD file of one package repository."""
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
    for exe in executables({"extra": {"cabal_components": components}}):
        exe_deps = deps + ([":" + name] if has_lib else []) + [":" + sublibrary_target(name, sub) for sub in sublibs]
        lines.append(_target("haskell_cabal_binary", "{}-exe-{}".format(name, exe), ['exe_name = "{}"'.format(exe)], exe_deps, tools))
    return "\n".join(lines)

def hub_build(hub, lock):
    """Returns the hub BUILD file with one alias per package and per executable."""
    lines = ['package(default_visibility = ["//visibility:public"])', ""]
    for name, package in sorted(by_name(lock).items()):
        repo = repo_name(hub, name)
        if "lib" in components(package):
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
        url = rctx.attr.urls,
        sha256 = rctx.attr.sha256,
        stripPrefix = rctx.attr.strip_prefix,
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
        "strip_prefix": attr.string(doc = "Directory prefix stripped from the sdist."),
        "tools": attr.string_list(doc = "Build tool labels of the package."),
        "urls": attr.string_list(mandatory = True, doc = "Sdist URLs."),
        "version": attr.string(mandatory = True, doc = "Package version."),
    },
)

def declare_packages(mctx, tag):
    """Declares the repositories of one cabal.lock tag."""
    lock = json.decode(mctx.read(tag.lock))
    problems = pin_problems(lock)
    if problems:
        fail("{}: {}".format(tag.lock, "; ".join(problems)))
    created = [tag.name]
    for name, package in sorted(by_name(lock).items()):
        archive = package["archives"][0]
        revised = has_revision(package)
        hackage_package(
            name = repo_name(tag.name, name),
            components = components(package),
            deps = package_deps(tag.name, lock, package),
            cabal_sha256 = package["extra"]["cabal_revision"]["sha256"] if revised else "",
            cabal_url = cabal_url(package) if revised else "",
            package = name,
            sha256 = archive["sha256"],
            strip_prefix = archive.get("strip_prefix", ""),
            tools = tool_labels(tag.name, package),
            urls = archive["urls"],
            version = package["version"],
        )
        created.append(repo_name(tag.name, name))
    hub_repo(name = tag.name, files = {"BUILD.bazel": hub_build(tag.name, lock)})
    return created
