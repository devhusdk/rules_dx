"""Parses staged Fortran sources with FORD and dumps their public API as JSON.

Only FORD's parser runs: nothing is compiled, no page is rendered and no
output directory is written. Any failure exits 1 and leaves no output file.
"""

import argparse
import json
import os
import pathlib
import platform
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool

FORMAT = 1

# Where each kind of object is declared, to find its line: FORD keeps none.
_DECLARATIONS = {
    "module": r"^\s*module\s+{name}\b",
    "type": r"^\s*type\b(?!\s*\().*\b{name}\s*$",
    "function": r"\bfunction\s+{name}\b",
    "subroutine": r"\bsubroutine\s+{name}\b",
    "interface": r"^\s*(abstract\s+)?interface\s+{name}\b",
    "variable": r"^(?!\s*(public|private|protected|use|import)\b).*::.*\b{name}\b",
    "component": r"^(?!\s*(public|private|protected|use|import)\b).*::.*\b{name}\b",
    "binding": r"^\s*(procedure|generic|final)\b.*\b{name}\b",
}


def _parse(argv):
    """Parses the command line."""
    parser = argparse.ArgumentParser(prog="ford_dump")
    parser.add_argument("--root", required=True)
    parser.add_argument("--out", required=True)
    return parser.parse_args(argv)


def _version():
    """Returns the installed FORD version, or "unknown" without metadata."""
    metadata = import_tool("importlib.metadata", "version")
    return metadata("FORD") or "unknown"


def _doc(entity):
    """Returns the doc comment lines of one entity as Markdown text."""
    lines = [line.rstrip() for line in entity.doc_list]
    if lines and all(not line or line.startswith(" ") for line in lines):
        lines = [line[1:] if line.startswith(" ") else line for line in lines]
    return "\n".join(lines).strip()


class _Dumper:
    """Collects the public entities of the parsed project."""

    def __init__(self, root):
        self.root = pathlib.Path(root)
        self.names = {}
        for current, _, files in os.walk(self.root):
            for name in files:
                path = pathlib.Path(current) / name
                self.names[os.path.realpath(path)] = path.relative_to(self.root).as_posix()
        self.sources = {}
        self.objects = []

    def _file(self, entity):
        """Returns the root-relative path of the file declaring the entity."""
        source = entity.source_file
        path = pathlib.Path(source.path)
        if not path.is_absolute():
            path = pathlib.Path(os.getcwd()) / path
        real = os.path.realpath(path)
        if real in self.names:
            return self.names[real]
        try:
            return pathlib.Path(os.path.abspath(path)).relative_to(self.root.absolute()).as_posix()
        except ValueError as error:
            raise RuntimeError(f"{source.path} is not under the staged root") from error

    def _lines(self, file):
        """Returns the source lines of one staged file."""
        if file not in self.sources:
            text = (self.root / file).read_text(encoding="utf-8", errors="replace")
            self.sources[file] = text.splitlines()
        return self.sources[file]

    def _line(self, file, kind, name, start=0):
        """Returns the 1-based line of the first declaration of a name from a line on."""
        word = re.compile(_DECLARATIONS[kind].format(name=re.escape(name)), re.IGNORECASE)
        lines = self._lines(file)
        for index in range(max(start, 1) - 1, len(lines)):
            text = lines[index].split("!")[0]
            if word.search(text):
                return index + 1
        return max(start, 1)

    def _add(self, kind, path, entity, parent, signature, **extra):
        """Records one object."""
        file = self._file(entity)
        line = self._line(file, kind, entity.name, extra.pop("after", 0))
        record = {
            "kind": kind,
            "path": path,
            "name": entity.name,
            "parent": parent,
            "doc": _doc(entity),
            "signature": signature,
            "file": file,
            "line": line,
            "params": [],
            "returns": None,
            "extends": "",
        }
        record.update(extra)
        self.objects.append(record)
        return record

    def _arguments(self, procedure):
        """Returns the documented arguments of one procedure."""
        params = []
        for arg in procedure.args:
            params.append(
                {
                    "name": arg.name,
                    "type": arg.full_type,
                    "intent": arg.intent or "",
                    "optional": bool(arg.optional),
                    "doc": _doc(arg),
                }
            )
        return params

    def _procedure(self, procedure, module, parent):
        """Records one function, subroutine or abstract interface body."""
        is_function = procedure.proctype.lower() == "function"
        kind = "function" if is_function else "subroutine"
        path = f"{parent}.{procedure.name}"
        names = ", ".join(arg.name for arg in procedure.args)
        attributes = " ".join(str(attrib) for attrib in procedure.attribs)
        signature = f"{kind} {procedure.name}({names})"
        if attributes:
            signature = f"{attributes} {signature}"
        returns = None
        if is_function and procedure.retvar is not None:
            returns = {"type": procedure.retvar.full_type, "doc": _doc(procedure.retvar)}
            signature += f" result({procedure.retvar.name})"
        self._add(
            kind,
            path,
            procedure,
            parent,
            signature,
            params=self._arguments(procedure),
            returns=returns,
            module=module,
        )

    def _type(self, ftype, module):
        """Records one derived type with its public components and bindings."""
        path = f"{module}.{ftype.name}"
        extends = ""
        if ftype.extends is not None:
            extends = getattr(ftype.extends, "name", str(ftype.extends))
        attributes = ", ".join(str(attrib) for attrib in ftype.attribs)
        head = "type" + (f", {attributes}" if attributes else "")
        if extends:
            head += f", extends({extends})"
        record = self._add("type", path, ftype, module, f"{head} :: {ftype.name}", extends=extends, module=module)
        for variable in ftype.variables:
            if variable.permission != "public":
                continue
            self._add(
                "component",
                f"{path}.{variable.name}",
                variable,
                path,
                f"{variable.full_type} :: {variable.name}",
                module=module,
                after=record["line"],
            )
        for binding in ftype.boundprocs:
            if binding.permission != "public":
                continue
            target = ", ".join(str(name) for name in binding.bindings)
            kind = "deferred" if binding.deferred else "procedure"
            self._add(
                "binding",
                f"{path}.{binding.name}",
                binding,
                path,
                f"{kind} :: {binding.name} => {target}" if target and target != binding.name else f"{kind} :: {binding.name}",
                module=module,
                after=record["line"],
            )

    def _variable(self, variable, module):
        """Records one public module variable or named constant."""
        head = variable.full_type
        if variable.parameter:
            head += ", parameter"
        signature = f"{head} :: {variable.name}"
        if variable.initial:
            signature += f" = {variable.initial}"
        self._add("variable", f"{module}.{variable.name}", variable, module, signature, module=module)

    def module(self, module):
        """Records one module and everything public in it."""
        self._add("module", module.name, module, "", f"module {module.name}", module=module.name)
        name = module.name
        for variable in module.variables:
            if variable.permission == "public":
                self._variable(variable, name)
        for ftype in module.types:
            if ftype.permission == "public":
                self._type(ftype, name)
        for interface in module.interfaces:
            if interface.permission != "public":
                continue
            procedures = [proc.name for proc in list(interface.subroutines) + list(interface.functions) + list(getattr(interface, "modprocs", []))]
            self._add(
                "interface",
                f"{name}.{interface.name}",
                interface,
                name,
                f"interface {interface.name}",
                module=name,
                members=sorted(procedures),
            )
        for procedure in list(module.functions) + list(module.subroutines):
            if procedure.permission == "public":
                self._procedure(procedure, name, name)
        for procedure in module.absinterfaces:
            if procedure.permission == "public":
                self._procedure(procedure, name, name)


def dump(root):
    """Returns the JSON text of the sources under one root, or raises on any failure."""
    settings_class = import_tool("ford.settings", "ProjectSettings")
    project_class = import_tool("ford.fortran_project", "Project")
    settings = settings_class(
        src_dir=[pathlib.Path(root)],
        project_url="",
        graph=False,
        quiet=True,
        warn=False,
    )
    project = project_class(settings)
    if not project.files:
        raise RuntimeError("no Fortran source files found")
    project.correlate()
    dumper = _Dumper(root)
    for module in sorted(project.modules, key=lambda m: m.name):
        dumper.module(module)
    document = {
        "format": FORMAT,
        "ford": _version(),
        "python": platform.python_version(),
        "objects": dumper.objects,
    }
    return json.dumps(document, sort_keys=True, indent=1)


def main(argv):
    """Runs the dump and returns the exit code."""
    args = _parse(argv)
    try:
        text = dump(args.root)
    except Exception as error:
        print(f"ford_dump: {error}", file=sys.stderr)
        return 1
    pathlib.Path(args.out).write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
