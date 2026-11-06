"""Readers for the opam file fields that `ocaml.packages` needs."""

SKIPPED_FILTERS = ["dev", "with-dev-setup", "with-doc", "with-test"]

TOOLCHAIN_PACKAGES = [
    "dune",
    "ocaml",
    "ocaml-base-compiler",
    "ocaml-config",
    "ocaml-options-vanilla",
    "ocaml-system",
    "ocaml-variants",
]

def is_toolchain_package(name):
    """Returns whether the pinned toolchain provides this opam package."""
    return name in TOOLCHAIN_PACKAGES or name.startswith("base-")

def _field(text, key):
    if text.startswith(key):
        return len(key)
    at = text.find("\n" + key)
    return -1 if at < 0 else at + 1 + len(key)

def _scan(text, start, opener, closer):
    """Returns the text between the opener at or after start and its closer."""
    begin = text.find(opener, start)
    if begin < 0:
        fail("opam field has no " + opener)
    depth = 0
    in_string = False
    skip = False
    for i in range(begin, len(text)):
        c = text[i]
        if skip:
            skip = False
        elif in_string:
            if c == "\\":
                skip = True
            elif c == "\"":
                in_string = False
        elif c == "\"":
            in_string = True
        elif c == opener:
            depth += 1
        elif c == closer:
            depth -= 1
            if depth == 0:
                return text[begin + 1:i]
    fail("opam field has no closing " + closer)

def _entries(body):
    """Splits a `depends` list body into (name, filter) pairs."""
    result = []
    i = 0
    n = len(body)
    for _ in range(n):
        if i >= n:
            break
        c = body[i]
        if c == "\"":
            end = body.find("\"", i + 1)
            result.append([body[i + 1:end], ""])
            i = end + 1
        elif c == "{" and result:
            inner = _scan(body, i, "{", "}")
            result[-1][1] = inner
            i += len(inner) + 2
        elif c == "#":
            end = body.find("\n", i)
            i = n if end < 0 else end
        else:
            i += 1
    return [(name, filt) for name, filt in result]

def _locked_version(filt):
    """Returns the version an entry pins with `= "v"`, or None when it does not apply."""
    version = None
    for term in filt.split("&"):
        term = term.strip()
        if term in SKIPPED_FILTERS:
            return None
        if "|" in term:
            return None
        if term.startswith("=") and not term.startswith("=="):
            version = term[1:].strip().strip("\"")
    return version

def parse_depends(text):
    """Returns {package: version} for the depends entries that pin a version.

    Entries limited to tests, docs or development versions are left out.
    """
    at = _field(text, "depends:")
    if at < 0:
        return {}
    result = {}
    for name, filt in _entries(_scan(text, at, "[", "]")):
        version = _locked_version(filt)
        if version != None:
            result[name] = version
    return result

def parse_depend_names(text):
    """Returns the names in `depends` that a build needs, without test, doc and dev ones."""
    at = _field(text, "depends:")
    if at < 0:
        return []
    names = []
    for name, filt in _entries(_scan(text, at, "[", "]")):
        if not any([term.strip() in SKIPPED_FILTERS for term in filt.split("&")]):
            names.append(name)
    return names

def parse_url(text):
    """Returns struct(src, sha256) from the `url` section of an opam file."""
    at = _field(text, "url")
    if at < 0:
        fail("the opam file has no url section")
    body = _scan(text, at, "{", "}")
    src = None
    sha256 = None
    at_src = body.find("src:")
    if at_src >= 0:
        quoted = body[at_src + 4:].split("\"")
        if len(quoted) > 1:
            src = quoted[1]
    for line in body.replace("[", "\n").replace("]", "\n").split("\n"):
        line = line.strip()
        for part in line.split("\""):
            if part.startswith("sha256="):
                sha256 = part[len("sha256="):]
    if not src:
        fail("the opam url section has no src")
    if not sha256:
        fail("the opam url section has no sha256 checksum for " + src)
    return struct(src = src, sha256 = sha256)
