"""Pin helpers shared by the package-hub repository rules."""

visibility(["//ansible/...", "//bicep/...", "//core/...", "//csharp/...", "//d/...", "//elixir/...", "//fortran/...", "//haskell/...", "//lua/...", "//ocaml/...", "//perl/...", "//php/...", "//powershell/...", "//ruby/...", "//swift/...", "//terraform/...", "//zig/..."])

_HEX = "0123456789abcdef"

def hub_name(hub, directory):
    """Returns the hub repository name of the set in `directory`: `hub`, else <hub>_<directory> with `/` written as `.`."""
    if not directory:
        return hub
    return hub + "_" + directory.replace("/", ".")

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
    problems.extend(_closure_problems(packages, ""))
    for group, closure in lock.get("groups", {}).items():
        if type(closure) != "list":
            problems.append("group %s has no packages list" % group)
            continue
        problems.extend(_closure_problems(closure, "group %s " % group))
    return problems

def _closure_problems(packages, where):
    """Returns the problems of one package list of a lock; deps resolve inside the list and each name appears once in it."""
    problems = []
    counts = {}
    for package in packages:
        if type(package) == "dict" and type(package.get("name")) == "string" and package["name"]:
            counts[package["name"]] = counts.get(package["name"], 0) + 1
    reported = {}
    for package in packages:
        if type(package) != "dict":
            problems.append("%shas a package that is not a JSON object" % where)
            continue
        name = package.get("name", "")
        if type(name) != "string" or not name:
            problems.append("%shas a package without a name" % where)
            continue
        if counts[name] > 1 and name not in reported:
            reported[name] = True
            problems.append("%spackage %s appears %d times" % (where, name, counts[name]))
        if not package.get("version"):
            problems.append("%spackage %s has no version" % (where, name))
        for dep in package.get("deps", []):
            if dep not in counts:
                problems.append("%spackage %s depends on %s, which is not in the lock" % (where, name, dep))
        for archive in package.get("archives", []):
            digest = archive.get("sha256", "")
            if type(digest) != "string" or len(digest) != 64 or digest.lstrip(_HEX):
                problems.append("%spackage %s has an archive without a 64 hex character sha256" % (where, name))
            urls = archive.get("urls", [])
            if not urls or any([not url.startswith("https://") for url in urls]):
                problems.append("%spackage %s has an archive without https urls" % (where, name))
            if type(archive.get("strip_prefix", "")) != "string":
                problems.append("%spackage %s has an archive whose strip_prefix is not a string" % (where, name))
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

def lock_repos(mctx, build_file, validate = None):
    """Creates one repository per `lock` tag of the module context; `build_file(package)` returns the BUILD file text of a decoded lock package and `validate(lock)`, when set, returns the ecosystem problems of a lock that `lock_problems` accepts."""
    for module in mctx.modules:
        for tag in module.tags.lock:
            text = mctx.read(tag.lock)
            lock = json.decode(text)
            problems = lock_problems(lock)
            if not problems and validate:
                problems = validate(lock)
            if problems:
                fail("%s %s" % (tag.lock, "; ".join(problems)))
            lock_repo(
                name = tag.name,
                lock = text,
                builds = {package["name"]: build_file(package) for package in lock["packages"]},
            )
