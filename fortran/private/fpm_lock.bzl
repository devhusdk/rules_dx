"""Reads an fpm.lock into pinned Fortran package archives."""

_STRINGS = ["sha256", "strip_prefix", "url", "version"]

def _string(value, where):
    if len(value) < 2 or value[0] != '"' or value[-1] != '"':
        fail("fpm.lock {} must be a quoted string".format(where))
    return value[1:-1]

def _strings(value, where):
    if not (value.startswith("[") and value.endswith("]")):
        fail("fpm.lock {} must be a list of quoted strings".format(where))
    inner = value[1:-1].strip()
    return [_string(item.strip(), where) for item in inner.split(",") if item.strip()]

def fpm_lock_packages(text):
    """Returns one dict per [dependencies.<name>] table: name, version, url, sha256, strip_prefix and deps."""
    tables = {}
    order = []
    current = None
    for raw in text.split("\n"):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("["):
            if not (line.startswith("[dependencies.") and line.endswith("]")):
                fail("fpm.lock supports only [dependencies.<name>] tables, found {}".format(line))
            current = line[len("[dependencies."):-1].strip('"')
            if current in tables:
                fail("fpm.lock lists {} twice".format(current))
            tables[current] = {}
            order.append(current)
            continue
        if current == None or "=" not in line:
            fail("fpm.lock line outside a [dependencies.<name>] table: {}".format(line))
        key, value = [part.strip() for part in line.split("=", 1)]
        where = "{}.{}".format(current, key)
        if key in _STRINGS:
            tables[current][key] = _string(value, where)
        elif key == "dependencies":
            tables[current][key] = _strings(value, where)
        else:
            fail("fpm.lock {} has the unsupported key {}".format(current, key))
    packages = []
    for name in order:
        table = tables[name]
        for key in ["sha256", "url", "version"]:
            if key not in table:
                fail("fpm.lock {} has no {}".format(name, key))
        for dep in table.get("dependencies", []):
            if dep not in tables:
                fail("fpm.lock {} depends on {}, which is not locked".format(name, dep))
        packages.append({
            "deps": table.get("dependencies", []),
            "name": name,
            "sha256": table["sha256"].lower(),
            "strip_prefix": table.get("strip_prefix", ""),
            "url": table["url"],
            "version": table["version"],
        })
    return packages
