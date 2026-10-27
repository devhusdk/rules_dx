"""Reports whether MODULE.bazel and the files it includes declare rules_dx at one expected version."""

import os
import re
import sys

MODULE_FILE = "MODULE.bazel"
PIN_ENV = "RULES_DX_PIN"

CALL = re.compile(r"^(bazel_dep|module|include)[ \t]*\(", re.MULTILINE)
NAME = re.compile(r'\bname[ \t]*=[ \t]*"rules_dx"')
LABEL = re.compile(r"""\s*(?:label\s*=\s*)?("[^"]*"|'[^']*')\s*,?\s*""")
INCLUDE_SUFFIX = ".MODULE.bazel"
VERSION = re.compile(r'\bversion[ \t]*=[ \t]*("[^"]*"|[^\s,()]+)')


def strip_comments(text):
    """Returns text with every # comment outside a string literal removed."""
    kept = []
    quote = ""
    index = 0
    while index < len(text):
        char = text[index]
        if quote:
            if char == "\\" and index + 1 < len(text):
                kept.append(text[index : index + 2])
                index += 2
                continue
            kept.append(char)
            if char == quote:
                quote = ""
        elif char in "\"'":
            quote = char
            kept.append(char)
        elif char == "#":
            end = text.find("\n", index)
            index = len(text) if end < 0 else end
            continue
        else:
            kept.append(char)
        index += 1
    return "".join(kept)


def call_arguments(text, opening):
    """Returns the arguments of the call whose opening paren is at opening."""
    depth = 0
    quote = ""
    index = opening
    while index < len(text):
        char = text[index]
        if quote:
            if char == "\\" and index + 1 < len(text):
                index += 2
                continue
            if char == quote:
                quote = ""
        elif char in "\"'":
            quote = char
        elif char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                return text[opening + 1 : index]
        index += 1
    return None


def declarations(text):
    """Returns the line, call name and arguments of every top-level call."""
    found = []
    for match in CALL.finditer(text):
        arguments = call_arguments(text, match.end() - 1)
        if arguments is None:
            continue
        line = text.count("\n", 0, match.start()) + 1
        found.append((line, match.group(1), arguments))
    return found


def rules_dx_declarations(text):
    """Returns every declaration naming rules_dx."""
    return [entry for entry in declarations(text) if NAME.search(entry[2])]


def include_path(label):
    """Returns (relative path, None) for a valid include label, else (None, why)."""
    if not label.startswith("//") or ":" not in label[2:]:
        return None, "is not a //package:file label"
    package, name = label[2:].split(":", 1)
    segments = (package.split("/") if package else []) + name.split("/")
    for segment in segments:
        if segment in ("", ".", "..") or "\\" in segment or ":" in segment:
            return None, "leaves the workspace"
    if not name.endswith(INCLUDE_SUFFIX) or segments[-1].startswith("."):
        return None, f"must name a {INCLUDE_SUFFIX} file"
    return "/".join(segments), None


def walk(root, rel, seen, found):
    """Appends every rules_dx declaration in rel, following includes in order.

    Returns None on success or one diagnostic message.
    """
    path = os.path.join(root, rel)
    try:
        with open(path, encoding="utf-8") as handle:
            text = strip_comments(handle.read())
    except OSError as error:
        return error
    for line, call, arguments in declarations(text):
        if call != "include":
            if NAME.search(arguments):
                found.append((rel, line, call, arguments))
            continue
        literal = LABEL.fullmatch(arguments)
        if literal is None:
            return (
                f"{rel}:{line} include() needs a literal "
                '"//package:name.MODULE.bazel" label'
            )
        label = literal.group(1)[1:-1]
        target, why = include_path(label)
        if target is None:
            return f"{rel}:{line} include {label} {why}"
        if label in seen:
            return f"{rel}:{line} includes {label} more than once"
        seen.add(label)
        problem = walk(root, target, seen, found)
        if isinstance(problem, OSError):
            return (
                f"{rel}:{line} includes {label}, but {target} cannot be read: {problem}"
            )
        if problem is not None:
            return problem
    return None


def fail(message):
    """Reports one diagnostic and returns a failing exit status."""
    print(f"dx-ci: {message}", file=sys.stderr)
    return 1


def verify(path, pin):
    """Reports whether path declares rules_dx at pin, and returns the status."""
    if pin == "" or '"' in pin or "\\" in pin or any(c.isspace() for c in pin):
        return fail(f"{PIN_ENV}={pin!r} is not a plain rules_dx version string")
    root, rel = os.path.split(path)
    root = root or "."
    found = []
    problem = walk(root, rel, set(), found)
    if isinstance(problem, OSError):
        return fail(f"{path} cannot be read: {problem}")
    if problem is not None:
        return fail(problem)
    if not found:
        return fail(
            f'{path} declares no bazel_dep(name = "rules_dx", version = "{pin}")'
        )
    if len(found) > 1:
        where = " and ".join(f"{f}:{line} ({call})" for f, line, call, _ in found)
        return fail(
            f"{path} declares rules_dx {len(found)} times, at {where}. "
            "Keep one declaration."
        )
    file, line, call, arguments = found[0]
    path = file
    version = VERSION.search(arguments)
    if version is None:
        return fail(
            f'{path}:{line} {call}(name = "rules_dx") declares no version. '
            f'Add version = "{pin}".'
        )
    if not version.group(1).startswith('"'):
        return fail(
            f"{path}:{line} {call} version must be a quoted string, "
            f'found {version.group(1)}. Write version = "{pin}".'
        )
    if version.group(1)[1:-1] != pin:
        return fail(
            f"{path}:{line} declares rules_dx {version.group(1)}, "
            f'the workflow expects "{pin}". Pin both to the same version.'
        )
    print(
        f'{path}:{line} {call}(name = "rules_dx", version = "{pin}") '
        "matches the workflow pin"
    )
    return 0


def main(argv):
    """Verifies one module file and returns its exit status."""
    os.chdir(os.environ.get("BUILD_WORKING_DIRECTORY", "."))
    path = argv[1] if len(argv) > 1 else MODULE_FILE
    return verify(path, os.environ.get(PIN_ENV, ""))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
