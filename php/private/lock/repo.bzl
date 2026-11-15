"""The composer lock: its validation, the repository rule that fetches it and the `lock` tag."""

_HEX = "0123456789abcdef"

def lock_problems(lock):
    """Returns why a decoded dx lock cannot build a hub: one message per problem, empty when it can."""
    problems = []
    if type(lock) != "dict":
        return ["is not a JSON object"]
    if lock.get("schema") != 1:
        problems.append("has schema %r, not 1" % lock.get("schema"))
    packages = lock.get("packages")
    if type(packages) != "list":
        return problems + ["has no packages list"]
    if lock.get("ecosystem") != "composer":
        problems.append("has ecosystem %r, not composer" % lock.get("ecosystem"))
    names = {}
    for package in packages:
        if type(package) == "dict" and package.get("name"):
            names[package["name"]] = names.get(package["name"], 0) + 1
    for package in packages:
        if type(package) != "dict":
            problems.append("has a package that is not a JSON object")
            continue
        name = package.get("name", "")
        if not name:
            problems.append("has a package without a name")
            continue
        if names[name] > 1:
            problems.append("package %s appears %d times" % (name, names[name]))
        if not package.get("version"):
            problems.append("package %s has no version" % name)
        extra = package.get("extra", {})
        if type(extra) != "dict" or type(extra.get("composer_manifest")) != "dict":
            problems.append("package %s has no composer_manifest object" % name)
        for dep in package.get("deps", []):
            if dep not in names:
                problems.append("package %s depends on %s, which is not in the lock" % (name, dep))
        for archive in package.get("archives", []):
            digest = archive.get("sha256", "")
            if len(digest) != 64 or any([char not in _HEX for char in digest.elems()]):
                problems.append("package %s has an archive without a 64 hex character sha256" % name)
            urls = archive.get("urls", [])
            if not urls or any([not url.startswith("https://") for url in urls]):
                problems.append("package %s has an archive without https urls" % name)
            if type(archive.get("strip_prefix", "")) != "string":
                problems.append("package %s has an archive whose strip_prefix is not a string" % name)
    return problems

def _lock_repo_impl(rctx):
    lock = json.decode(rctx.attr.lock)
    for package in lock["packages"]:
        name = package["name"]
        for archive in package.get("archives", []):
            rctx.download_and_extract(
                url = archive["urls"],
                sha256 = archive["sha256"],
                output = name,
                strip_prefix = archive.get("strip_prefix", ""),
                type = archive.get("type", ""),
            )
        rctx.file(name + "/BUILD.bazel", rctx.attr.builds[name])
    rctx.file("BUILD.bazel", "")
    return rctx.repo_metadata(reproducible = True)

lock_repo = repository_rule(
    implementation = _lock_repo_impl,
    attrs = {
        "builds": attr.string_dict(mandatory = True, doc = "The BUILD file text of each package, by package name."),
        "lock": attr.string(mandatory = True, doc = "The decoded dx lock as JSON text."),
    },
)

lock_tag = tag_class(
    attrs = {
        "lock": attr.label(mandatory = True, allow_single_file = [".json"], doc = "The dx lock file of the repository."),
        "name": attr.string(mandatory = True, doc = "The name of the repository."),
    },
)

def lock_repos(mctx, build_file):
    """Creates one repository per `lock` tag of the module context; `build_file(package)` returns the BUILD file text of a decoded lock package."""
    for module in mctx.modules:
        for tag in module.tags.lock:
            text = mctx.read(tag.lock)
            lock = json.decode(text)
            problems = lock_problems(lock)
            if problems:
                fail("%s %s" % (tag.lock, "; ".join(problems)))
            lock_repo(
                name = tag.name,
                lock = text,
                builds = {package["name"]: build_file(package) for package in lock["packages"]},
            )
