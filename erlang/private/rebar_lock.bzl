"""Reads a rebar.lock into Hex package pins."""

load("//elixir/private:mix_lock.bzl", "parse_terms")

def _erlang_to_terms(text):
    lines = [line for line in text.split("\n") if not line.strip().startswith("%")]
    text = "\n".join(lines).replace('<<"', '"').replace('">>', '"')
    return text.replace("}.", "}").replace("].", "]")

def _hashes(table, kind):
    for item in table:
        if item[0] == kind:
            return {pair[0]: pair[1] for pair in item[1]}
    fail("rebar.lock has no {} table".format(kind))

def rebar_lock_packages(text):
    """Returns one dict per package of a rebar.lock: package, version, sha256 and level."""
    terms = parse_terms(_erlang_to_terms(text))
    if len(terms) != 2 or terms[0][0] != "1.2.0":
        fail("rebar.lock must be lock format 1.2.0: a versioned entry list followed by the hash tables")
    hashes = _hashes(terms[1], "pkg_hash_ext")
    packages = []
    for entry in terms[0][1]:
        key, source, level = entry[0], entry[1], entry[2]
        if source[0] != "pkg":
            fail("rebar.lock entry {} is a {} dependency; only pkg entries are supported".format(key, source[0]))
        if key not in hashes:
            fail("rebar.lock has no pkg_hash_ext for {}".format(key))
        packages.append({
            "level": int(level),
            "package": source[1],
            "sha256": hashes[key].lower(),
            "version": source[2],
        })
    return packages
