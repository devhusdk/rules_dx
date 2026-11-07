"""Reads a mix.lock into Hex package pins."""

_STOP = [",", "}", "]", " ", "\n", "\t", "\r"]

def _word_end(text, start):
    for end in range(start, len(text)):
        if text[end] in _STOP:
            return end
    return len(text)

def parse_terms(text):
    """Returns the nested lists of a mix.lock; strings stay plain, atoms keep their colon."""
    stack = [[]]
    skip = 0
    for i in range(len(text)):
        if skip > 0:
            skip -= 1
            continue
        char = text[i]
        if char == "#":
            end = text.find("\n", i)
            skip = (end if end >= 0 else len(text)) - i
        elif char == '"':
            end = text.find('"', i + 1)
            if end < 0:
                fail("mix.lock has an unterminated string")
            stack[-1].append(text[i + 1:end])
            skip = end - i
            if text[end + 1:end + 2] == ":":
                skip += 1
        elif char in ["{", "["]:
            stack.append([])
        elif char in ["}", "]"]:
            if len(stack) < 2:
                fail("mix.lock closes a bracket it did not open")
            done = stack.pop()
            stack[-1].append(done)
        elif char not in _STOP and char != "%":
            end = _word_end(text, i)
            stack[-1].append(text[i:end])
            skip = end - i - 1
    if len(stack) != 1:
        fail("mix.lock has an unclosed bracket")
    return stack[0]

def _atom(term):
    return term[1:] if type(term) == "string" and term.startswith(":") else term

def _dependency(dep):
    options = dep[2] if len(dep) > 2 else []
    optional = False
    package = _atom(dep[0])
    for i in range(len(options) - 1):
        if options[i] == "optional:" and options[i + 1] == "true":
            optional = True
        if options[i] == "hex:":
            package = _atom(options[i + 1])
    return None if optional else package

def mix_lock_packages(text):
    """Returns one struct-like dict per Hex entry: package, version, sha256 and deps."""
    terms = parse_terms(text)
    if len(terms) != 1:
        fail("mix.lock must hold one map")
    entries = terms[0]
    packages = []
    for i in range(0, len(entries) - 1, 2):
        key, entry = entries[i], entries[i + 1]
        kind = _atom(entry[0])
        if kind != "hex":
            fail("mix.lock entry {} is a {} dependency; only hex entries are supported".format(key, kind))
        if len(entry) < 8 or entry[6] != "hexpm":
            fail("mix.lock entry {} is not from the hexpm repository".format(key))
        deps = [_dependency(dep) for dep in entry[5]]
        packages.append({
            "deps": [dep for dep in deps if dep],
            "package": _atom(entry[1]),
            "sha256": entry[7],
            "version": entry[2],
        })
    return packages
