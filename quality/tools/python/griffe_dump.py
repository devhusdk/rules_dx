"""Loads one Python package statically with griffe and dumps its API as JSON.

Nothing under the root is imported or run: inspection is off, no extensions
load and the only search path is the staged source root. Any failure exits 1
and leaves no output file.
"""

import argparse
import ast
import json
import logging
import pathlib
import platform
import sys

from tool_bootstrap import import_tool

FORMAT = 1
DOCSTYLES = ("google", "numpy", "sphinx")


class _FailOnError(logging.Handler):
    """Remembers every record at ERROR or above."""

    def __init__(self):
        super().__init__(level=logging.ERROR)
        self.errors = []

    def emit(self, record):
        """Stores the formatted message."""
        self.errors.append(record.getMessage())


def _encoder():
    """Returns a JSON encoder that writes every griffe expression as source text."""
    base = import_tool("griffe", "JSONEncoder")
    expression = import_tool("griffe", "Expr")

    class Encoder(base):
        """Renders expressions as strings so the mapper never parses their tree."""

        def default(self, o):
            """Encodes one object."""
            if isinstance(o, expression):
                return str(o)
            return super().default(o)

    return Encoder


def _parse(argv):
    """Parses the command line."""
    parser = argparse.ArgumentParser(prog="griffe_dump")
    parser.add_argument("--root", required=True)
    parser.add_argument("--package", required=True)
    parser.add_argument("--docstyle", required=True, choices=DOCSTYLES)
    parser.add_argument("--out", required=True)
    return parser.parse_args(argv)


def _version():
    """Returns the installed griffelib version."""
    metadata = import_tool("importlib.metadata", "version")
    return metadata("griffelib")


def _check_sources(root, package):
    """Parses every source of the package without running it.

    griffe logs syntax errors and compiled modules at debug level only, so the
    check is done here and does not depend on log levels.
    """
    base = pathlib.Path(root)
    paths = [base / package]
    paths += [base / f"{package}{suffix}" for suffix in (".py", ".pyi")]
    for path in paths:
        files = sorted(path.rglob("*")) if path.is_dir() else [path]
        for file in files:
            if not file.is_file():
                continue
            if file.suffix in (".so", ".pyd"):
                raise RuntimeError(
                    f"compiled module {file.relative_to(base)} is not supported"
                )
            if file.suffix in (".py", ".pyi"):
                try:
                    ast.parse(file.read_text(encoding="utf-8"), str(file))
                except (SyntaxError, ValueError) as error:
                    raise RuntimeError(f"{file.relative_to(base)}: {error}") from error


def dump(root, package, docstyle):
    """Returns the JSON text of one package, or raises on any failure."""
    _check_sources(root, package)
    loader_class = import_tool("griffe", "GriffeLoader")
    handler = _FailOnError()
    logger = logging.getLogger("griffe")
    logger.addHandler(handler)
    try:
        loader = loader_class(
            search_paths=[root],
            allow_inspection=False,
            docstring_parser=docstyle,
        )
        module = loader.load(package)
        loader.resolve_aliases(implicit=False, external=False)
    finally:
        logger.removeHandler(handler)
    if handler.errors:
        raise RuntimeError("; ".join(handler.errors))
    path = module.filepath
    if not isinstance(path, pathlib.Path) or not path.resolve().is_relative_to(
        pathlib.Path(root).resolve()
    ):
        raise RuntimeError(
            f"package '{package}' is not a source package under the root"
        )
    document = {
        "format": FORMAT,
        "package": package,
        "docstyle": docstyle,
        "griffe": _version(),
        "python": platform.python_version(),
        "module": module,
    }
    return json.dumps(document, cls=_encoder(), full=True, sort_keys=True)


def main(argv):
    """Runs the dump and returns the exit code."""
    args = _parse(argv)
    try:
        text = dump(args.root, args.package, args.docstyle)
    except Exception as error:  # pylint: disable=broad-exception-caught
        print(f"griffe_dump: {error}", file=sys.stderr)
        return 1
    pathlib.Path(args.out).write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
