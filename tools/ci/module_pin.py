"""Reports whether a MODULE.bazel declares rules_dx at one expected version.

The consumer and docs workflows pass the expected version through the
RULES_DX_PIN environment variable and run this file from standard input.
The module is read as text: the file is never imported, executed or
evaluated. Only a top-level `bazel_dep` or `module` call naming rules_dx
counts as a declaration, and its version must be a quoted string literal.
"""

import os
import re
import sys

MODULE_FILE = "MODULE.bazel"
PIN_ENV = "RULES_DX_PIN"

CALL = re.compile(r"^(bazel_dep|module)[ \t]*\(", re.MULTILINE)
NAME = re.compile(r'\bname[ \t]*=[ \t]*"rules_dx"')
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


def fail(message):
    """Reports one diagnostic and returns a failing exit status."""
    print(f"dx-ci: {message}", file=sys.stderr)
    return 1


def verify(path, pin):
    """Reports whether path declares rules_dx at pin, and returns the status."""
    if pin == "" or '"' in pin or "\\" in pin or any(c.isspace() for c in pin):
        return fail(f"{PIN_ENV}={pin!r} is not a plain rules_dx version string")
    try:
        with open(path, encoding="utf-8") as handle:
            text = strip_comments(handle.read())
    except OSError as error:
        return fail(f"{path} cannot be read: {error}")
    found = rules_dx_declarations(text)
    if not found:
        return fail(
            f'{path} declares no bazel_dep(name = "rules_dx", version = "{pin}")'
        )
    if len(found) > 1:
        where = " and ".join(f"line {line} ({call})" for line, call, _ in found)
        return fail(
            f"{path} declares rules_dx {len(found)} times, at {where}. "
            "Keep one declaration."
        )
    line, call, arguments = found[0]
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
    path = argv[1] if len(argv) > 1 else MODULE_FILE
    return verify(path, os.environ.get(PIN_ENV, ""))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
