"""Renders patches/README.md with the Version column taken from MODULE.bazel."""

import re
import sys

_OVERRIDE = re.compile(r"\n[a-z_]+_override\((.*?)\n\)", re.S)
_DEP = re.compile(r'bazel_dep\(name = "([^"]+)", version = "([^"]+)"')
_PATCH = re.compile(r'"//patches/[^:"]+:([^"]+\.patch)"')
_LABEL = re.compile(r'"(?:@rules_dx)?//patches/([^:"]+):([^"]+\.patch)"')


def overrides(module_text):
    """Returns {module: (version, [patch names])} for each override."""
    found = {}
    deps = dict(_DEP.findall(module_text))
    for body in _OVERRIDE.findall(module_text):
        name = re.search(r'module_name = "([^"]+)"', body)
        version = re.search(r'version = "([^"]+)"', body)
        if not name:
            continue
        pinned = version.group(1) if version else deps.get(name.group(1))
        if not pinned:
            continue
        found[name.group(1)] = (pinned, _PATCH.findall(body))
    return found


def render(module_text, readme_text):
    """Returns the README with each table row's Version set from MODULE.bazel."""
    known = overrides(module_text)
    listed = {}
    out = []
    for line in readme_text.split("\n"):
        cells = line.split(" | ", 3)
        if len(cells) == 4 and cells[0].startswith("| `") and not cells[0].startswith("| `---"):
            module = cells[0][3:].split("`", 1)[0]
            if module not in known:
                raise SystemExit("patches/README.md: %s has no override in MODULE.bazel" % module)
            if not (cells[3].startswith("Root only.") or re.search(r"Consumer fix: #\d+ \|?$", cells[3])):
                raise SystemExit("patches/README.md: %s row has no class" % module)
            version, patches = known[module]
            for patch in re.findall(r"`([^`]+\.patch)`", cells[2]):
                if patch not in patches:
                    raise SystemExit("patches/README.md: %s lists %s, which MODULE.bazel does not apply" % (module, patch))
            listed.setdefault(module, set()).update(re.findall(r"`([^`]+\.patch)`", cells[2]))
            cells[1] = version
            line = " | ".join(cells)
        out.append(line)
    for module, (_, patches) in known.items():
        for patch in patches:
            if patch not in listed.get(module, ()):
                raise SystemExit("patches/README.md: %s does not list %s, which MODULE.bazel applies" % (module, patch))
    sections = {}
    for block in readme_text.split("\n## ")[1:]:
        heading, _, body = block.partition("\n")
        for name in re.findall(r"[\w-]+", heading):
            sections[name] = sections.get(name, "") + body
    for directory, patch in sorted(set(_LABEL.findall(module_text))):
        if patch in known.get(directory, (None, ()))[1]:
            continue
        if "`%s`" % patch not in sections.get(directory, ""):
            raise SystemExit("patches/README.md: no section for %s names %s, which MODULE.bazel applies" % (directory, patch))
    return "\n".join(out)


def main(argv):
    with open(argv[1], encoding="utf-8") as handle:
        module_text = handle.read()
    with open(argv[2], encoding="utf-8") as handle:
        readme_text = handle.read()
    sys.stdout.write(render(module_text, readme_text))


if __name__ == "__main__":
    main(sys.argv)
