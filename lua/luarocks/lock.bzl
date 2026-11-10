"""Pure luarocks.lock logic: parsing, validation, dependency edges and generated BUILD files.

Nothing here runs LuaRocks or Lua. The lock is read line by line and never evaluated.
"""

load("//core:pins.bzl", _spoke_name = "spoke_name")

_NAME_CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_"

_MODULE_CHARS = _NAME_CHARS + "-"

_HEX_CHARS = "0123456789abcdef"

def spoke_name(hub, rock):
    """Returns the repository name of one rock: <hub>__<rock> with - and . written as _."""
    return _spoke_name(hub, rock, "-.")

def module_path(module):
    """Returns the repository path of a Lua module name: a.b -> a/b.lua."""
    return module.replace(".", "/") + ".lua"

def _is_bare_key(key):
    return len(key) > 0 and all([c in _NAME_CHARS for c in key.elems()])

def _parse_entry(line):
    """Parses `key = "value"` or `["key"] = "value"`; returns (key, value) or None."""
    line = line.strip()
    if line.endswith(","):
        line = line[:-1].rstrip()
    if line.startswith("[\""):
        end = line.find("\"]")
        if end < 0:
            return None
        key = line[2:end]
        rest = line[end + 2:].strip()
    else:
        index = line.find("=")
        if index < 0:
            return None
        key = line[:index].strip()
        rest = line[index:]
        if not _is_bare_key(key):
            return None
    if not key or "\"" in key or not rest.startswith("="):
        return None
    value = rest[1:].strip()
    if len(value) < 2 or not value.startswith("\"") or not value.endswith("\"") or "\"" in value[1:-1]:
        return None
    return key, value[1:-1]

def parse_lock(text):
    """Parses luarocks.lock; returns (pins dict of rock to version, errors)."""
    pins = {}
    errors = []
    depth = 0
    in_dependencies = False
    for number, raw in enumerate(text.splitlines()):
        line = raw.strip()
        where = "luarocks.lock:%d" % (number + 1)
        if not line or line.startswith("--"):
            continue
        if in_dependencies:
            if line in ("}", "},"):
                in_dependencies = False
                depth -= 1
                continue
            entry = _parse_entry(line)
            if entry == None:
                errors.append("%s: cannot read %r; expected name = \"version\"" % (where, line))
            elif entry[0] in pins:
                errors.append("%s: %s is pinned twice" % (where, entry[0]))
            else:
                pins[entry[0]] = entry[1]
        elif depth == 0 and line == "return {":
            depth = 1
        elif depth == 1 and line == "dependencies = {":
            in_dependencies = True
            depth = 2
        elif depth == 1 and line in ("}", "},"):
            depth = 0
        else:
            errors.append("%s: unsupported line %r" % (where, line))
    if depth != 0:
        errors.append("luarocks.lock: unbalanced braces")
    return pins, errors

def parse_sidecar(text):
    """Parses luarocks.digests.json; returns (packages dict, errors)."""
    if not text.strip():
        return {}, []
    data = json.decode(text)
    if type(data) != "dict" or type(data.get("packages")) != "dict":
        return {}, ["luarocks.digests.json must be {\"packages\": {...}}"]
    return data["packages"], []

def _module_problem(module, path):
    if type(path) != "string" or not path or path.startswith("/") or ".." in path.split("/"):
        return "module %s has the path %r; it must be a relative path inside the source" % (module, path)
    parts = module.split(".")
    if not all([part and all([c in _MODULE_CHARS for c in part.elems()]) for part in parts]):
        return "module name %r is not a dotted Lua module name" % module
    if parts[-1] == "init":
        return "module %s maps to %s, which names the module %s; list it as %s" % (
            module,
            module_path(module),
            ".".join(parts[:-1]) or "init",
            ".".join(parts[:-1]) or "a name other than init",
        )
    return ""

def _pin_problem(pin):
    sha256 = (pin.get("source") or {}).get("sha256", "")
    if type(sha256) != "string" or len(sha256) != 64 or not all([c in _HEX_CHARS for c in sha256.elems()]):
        return "has no 64 hex character sha256"
    return ""

def _cycle(deps):
    """Returns the sorted rocks left after removing everything that does not sit on or above a cycle."""
    remaining = {name: list(edges) for name, edges in deps.items()}
    for _ in range(len(remaining) + 1):
        done = [name for name, edges in remaining.items() if not edges]
        if not done:
            break
        for name in done:
            remaining.pop(name)
        for name in list(remaining.keys()):
            remaining[name] = [edge for edge in remaining[name] if edge in remaining]
    return sorted(remaining.keys())

def analyze(hub, lock_text, sidecar_text):
    """Validates one lock and sidecar, returning errors and per-rock package records."""
    errors = []
    locked, lock_errors = parse_lock(lock_text)
    errors += lock_errors
    pins, sidecar_errors = parse_sidecar(sidecar_text)
    errors += sidecar_errors
    locked.pop("lua", None)
    packages = {}
    spokes = {}
    for name in sorted(locked.keys()):
        version = locked[name]
        spoke = spoke_name(hub, name)
        if spoke in spokes:
            errors.append("%s and %s both map to the repository %s" % (spokes[spoke], name, spoke))
            continue
        spokes[spoke] = name
        pin = pins.get(name)
        if type(pin) != "dict":
            errors.append("%s %s has no entry in luarocks.digests.json" % (name, version))
            continue
        if pin.get("version") != version:
            errors.append("%s is %s in luarocks.lock but %s in luarocks.digests.json; re-pin it" % (name, version, pin.get("version")))
            continue
        source = pin.get("source") or {}
        urls = source.get("urls") or []
        modules = pin.get("modules") or {}
        if not urls:
            errors.append("%s has no source urls in luarocks.digests.json" % name)
        if not modules:
            errors.append("%s lists no modules in luarocks.digests.json" % name)
        for module in sorted(modules.keys()):
            problem = _module_problem(module, modules[module])
            if problem:
                errors.append("%s: %s" % (name, problem))
        problem = _pin_problem(pin)
        packages[name] = struct(
            name = name,
            version = version,
            spoke = spoke,
            urls = urls,
            strip_prefix = source.get("strip_prefix", ""),
            sha256 = "" if problem else source["sha256"],
            unpinned = problem,
            modules = modules,
            deps = [],
        )
    edges = {}
    for name in sorted(packages.keys()):
        found = []
        for dep in sorted(pins[name].get("deps") or []):
            if dep == "lua" or dep == name:
                continue
            if dep not in locked:
                errors.append("%s depends on %s, which is not in luarocks.lock" % (name, dep))
            elif dep in packages and dep not in found:
                found.append(dep)
        edges[name] = found
    loop = _cycle(edges)
    if loop:
        errors.append("dependency cycle among: %s" % ", ".join(loop))
    for pin_name in sorted(pins.keys()):
        if pin_name not in locked:
            errors.append("luarocks.digests.json pins %s, which is not in luarocks.lock; remove it" % pin_name)
    resolved = {
        name: struct(
            name = name,
            version = packages[name].version,
            spoke = packages[name].spoke,
            urls = packages[name].urls,
            strip_prefix = packages[name].strip_prefix,
            sha256 = packages[name].sha256,
            unpinned = packages[name].unpinned,
            modules = packages[name].modules,
            deps = edges[name],
        )
        for name in packages
    }
    return struct(errors = errors, packages = resolved)

def pin_entry_json(name, sha256):
    """Returns the sha256 line to commit for one rock."""
    return json.encode_indent({name: {"source": {"sha256": sha256}}}, indent = "  ")

def spoke_build(package, rules_bzl, dep_spokes):
    """Returns the BUILD file of one rock repository."""
    return """load("{bzl}", "lua_library")

package(default_visibility = ["//visibility:public"])

lua_library(
    name = {name},
    srcs = {srcs},
    deps = {deps},
)
""".format(
        bzl = rules_bzl,
        name = repr(package.name),
        srcs = repr(sorted([module_path(module) for module in package.modules.keys()])),
        deps = repr(["@%s//:%s" % (spoke, rock) for spoke, rock in dep_spokes]),
    )

def hub_package_build(rock, spoke):
    """Returns the BUILD file the hub holds for one rock; the target is named like the rock."""
    return """package(default_visibility = ["//visibility:public"])

alias(
    name = "{rock}",
    actual = "@{spoke}//:{rock}",
)
""".format(rock = rock, spoke = spoke)

def hub_root_build(packages, rules_bzl):
    """Returns the hub's root BUILD file with :all_packages."""
    return """load("{bzl}", "lua_library")

package(default_visibility = ["//visibility:public"])

lua_library(
    name = "all_packages",
    deps = {deps},
)
""".format(bzl = rules_bzl, deps = repr(["//%s" % name for name in sorted(packages.keys())]))
