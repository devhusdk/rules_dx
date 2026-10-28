"""ansible-lint entry point with ansible-* shims on PATH."""

import importlib
import os
import shlex
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool, site_packages

WINDOWS_MESSAGE = (
    "ansible_lint: no upstream Windows build (windows_x86_64 is unsupported); "
    "remove ansible_lint from this family on Windows"
)
WINDOWS_EXIT = 69
COLLECTIONS_FLAG = "--dx-collections-path"
SHIM_MODULES = {
    "ansible": "ansible.cli.adhoc",
    "ansible-config": "ansible.cli.config",
    "ansible-galaxy": "ansible.cli.galaxy",
    "ansible-playbook": "ansible.cli.playbook",
}


def shim_main(name, args):
    """Runs one ansible CLI in-process under its shim name."""
    for site in reversed(site_packages()):
        sys.path.insert(0, str(site))
    module = importlib.import_module(SHIM_MODULES[name])
    sys.argv = [name] + list(args)
    return module.main()


def write_shims(directory, script):
    """Writes one executable shell shim per ansible CLI and returns the directory."""
    os.makedirs(directory, exist_ok=True)
    for name in SHIM_MODULES:
        path = os.path.join(directory, name)
        body = '#!/bin/sh\nexec %s %s --ansible-shim %s "$@"\n' % (
            shlex.quote(sys.executable),
            shlex.quote(script),
            shlex.quote(name),
        )
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(body)
        os.chmod(path, 0o755)
    return directory


def prepare_env(environ, collections=None):
    """Exports import roots, shim PATH, cache dirs and the collections path into environ."""
    tmp = environ.get("TMPDIR") or "/tmp"
    sites = [str(site) for site in site_packages()]
    existing = environ.get("PYTHONPATH")
    if existing:
        sites.append(existing)
    if sites:
        environ["PYTHONPATH"] = os.pathsep.join(sites)
    shims = write_shims(os.path.join(tmp, "ansible-bin"), os.path.abspath(__file__))
    environ["PATH"] = shims
    environ.setdefault("ANSIBLE_HOME", os.path.join(tmp, "ansible-home"))
    environ.setdefault("ANSIBLE_LOCAL_TEMP", os.path.join(tmp, "ansible-local"))
    environ.setdefault("XDG_CACHE_HOME", os.path.join(tmp, "ansible-cache"))
    environ["ANSIBLE_COLLECTIONS_SCAN_SYS_PATH"] = "False"
    environ.pop("ANSIBLE_COLLECTIONS_PATH", None)
    environ.pop("ANSIBLE_COLLECTIONS_PATHS", None)
    if collections:
        environ["ANSIBLE_COLLECTIONS_PATH"] = collections
    environ["ANSIBLE_LINT_SKIP_SCHEMA_UPDATE"] = "1"
    return shims


def main(argv=None):
    """Runs ansible-lint, or one ansible shim when invoked as a dispatcher."""
    argv = list(sys.argv[1:] if argv is None else argv)
    if os.name == "nt":
        print(WINDOWS_MESSAGE, file=sys.stderr)
        return WINDOWS_EXIT
    if argv[:1] == ["--ansible-shim"]:
        return shim_main(argv[1], argv[2:])
    collections = None
    if argv[:1] == [COLLECTIONS_FLAG]:
        if len(argv) < 2:
            print("ansible_lint: %s needs a value" % COLLECTIONS_FLAG, file=sys.stderr)
            return 2
        collections = argv[1]
        argv = argv[2:]
    prepare_env(os.environ, collections)
    sys.argv = ["ansible-lint"] + argv
    return import_tool("ansiblelint.__main__", "_run_cli_entrypoint")()


if __name__ == "__main__":
    sys.exit(main())
