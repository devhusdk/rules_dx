"""Resolves a cabal project into cabal.project.freeze and a Hackage sidecar, or checks them."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

SIDECAR_VERSION = 1
NO_HOST_TOOL = "dx-no-host-tool"
FREEZE_CONSTRAINT = re.compile(r"any\.([A-Za-z0-9_-]+)\s*==\s*([0-9][0-9.]*)")
INDEX_STATE = re.compile(r"^\s*index-state\s*:\s*(\S+)", re.MULTILINE)


def sha256(data):
    """Returns the hex digest of bytes, with line endings normalised for text."""
    return hashlib.sha256(data).hexdigest()


def text_sha256(path):
    """Returns the digest of one text file with CRLF read as LF."""
    with open(path, "rb") as handle:
        return sha256(handle.read().replace(b"\r\n", b"\n"))


def inputs_sha256(paths):
    """Returns one digest over the named input files, ordered by name."""
    outer = hashlib.sha256()
    for path in sorted(paths, key=os.path.basename):
        outer.update(os.path.basename(path).encode() + b"\0" + text_sha256(path).encode() + b"\n")
    return outer.hexdigest()


def rlocation(path):
    """Returns the real path of one runfile named by its rlocation path."""
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    if manifest and os.path.isfile(manifest):
        with open(manifest, encoding="utf-8") as handle:
            for line in handle:
                key, _, value = line.rstrip("\n").partition(" ")
                if key == path:
                    return value
    roots = [os.environ.get("RUNFILES_DIR"), sys.argv[0] + ".runfiles"]
    for root in roots:
        if root and os.path.exists(os.path.join(root, path)):
            return os.path.join(root, path)
    raise SystemExit("deps_resolve: cannot find runfile " + path)


def index_state(project_path):
    """Returns the index-state pinned in cabal.project, or fails."""
    with open(project_path, encoding="utf-8") as handle:
        match = INDEX_STATE.search(handle.read())
    if not match:
        raise SystemExit("deps_resolve: cabal.project must pin 'index-state:' so a resolve is repeatable")
    return match.group(1)


def freeze_pairs(freeze_text):
    """Returns the sorted (name, version) pairs a freeze file pins."""
    return sorted(set(FREEZE_CONSTRAINT.findall(freeze_text)))


def cabal_revisions(index_path, wanted):
    """Returns {(name, version, cabal_sha256): revision} read from the Hackage index tarball."""
    names = {"{0}/{1}/{0}.cabal".format(name, version): (name, version) for name, version in wanted}
    seen = {}
    found = {}
    with tarfile.open(index_path) as index:
        for member in index:
            if member.name not in names:
                continue
            digest = sha256(index.extractfile(member).read())
            ordinal = seen.get(member.name, 0)
            seen[member.name] = ordinal + 1
            name, version = names[member.name]
            found.setdefault((name, version, digest), ordinal)
    return found


def component_kind(component):
    """Returns the component name when it is a library or executable, else None."""
    if component == "lib" or component.startswith("exe:"):
        return component
    return None


def sidecar_from_plan(plan, revisions):
    """Builds the package list of the sidecar from a cabal plan.json document."""
    entries = plan["install-plan"]
    names = {entry["id"]: entry["pkg-name"] for entry in entries}
    packages = {}
    for entry in entries:
        key = (entry["pkg-name"], entry["pkg-version"])
        source = entry.get("pkg-src", {}).get("type")
        package = packages.setdefault(key, {
            "name": key[0],
            "version": key[1],
            "source": "hackage" if source == "repo-tar" else ("local" if entry["type"] == "configured" else "ghc"),
            "components": set(),
            "dependencies": set(),
        })
        if entry["type"] == "configured" and entry.get("pkg-src-sha256"):
            package["sdist_sha256"] = entry["pkg-src-sha256"]
            package["cabal_sha256"] = entry["pkg-cabal-sha256"]
            package["cabal_revision"] = revisions.get((key[0], key[1], entry["pkg-cabal-sha256"]))
        component = component_kind(entry.get("component-name", "lib"))
        if component is None:
            continue
        package["components"].add(component)
        for dep in entry.get("depends", []) + entry.get("exe-depends", []):
            if names.get(dep) not in (None, key[0]):
                package["dependencies"].add(names[dep])
    out = []
    for key in sorted(packages):
        package = packages[key]
        package["components"] = sorted(package["components"])
        package["dependencies"] = sorted(package["dependencies"])
        out.append(package)
    return out


def check(args):
    """Fails when the freeze or the sidecar is stale against the sources."""
    inputs = [rlocation(path) for path in args.input]
    freeze = rlocation(args.freeze)
    sidecar_path = rlocation(args.sidecar)
    with open(sidecar_path, encoding="utf-8") as handle:
        sidecar = json.load(handle)
    problems = []
    if sidecar.get("version") != SIDECAR_VERSION:
        problems.append("sidecar version is not {}".format(SIDECAR_VERSION))
    if sidecar.get("inputs_sha256") != inputs_sha256(inputs):
        problems.append("the .cabal files or cabal.project changed since the last resolve")
    if sidecar.get("freeze_sha256") != text_sha256(freeze):
        problems.append("the freeze file differs from the one the sidecar was written for")
    with open(freeze, encoding="utf-8") as handle:
        frozen = freeze_pairs(handle.read())
    listed = sorted((pkg["name"], pkg["version"]) for pkg in sidecar.get("packages", []) if pkg["source"] != "local")
    if frozen != listed:
        problems.append("the freeze file and the sidecar pin different packages")
    if problems:
        sys.stderr.write("haskell_deps_resolve_test: stale files in {}:\n  {}\nRun: bazel run {}\n".format(args.package, "\n  ".join(problems), args.label))
        return 1
    return 0


def run(command, cwd, env):
    """Runs one command and fails with its output when it fails."""
    result = subprocess.run(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False)
    if result.returncode != 0:
        raise SystemExit("deps_resolve: {} failed:\n{}".format(" ".join(command), result.stdout))
    return result.stdout


def sibling_ghc_pkg(ghc):
    """Returns the ghc-pkg that sits next to ghc."""
    suffix = ".exe" if os.name == "nt" else ""
    return os.path.join(os.path.dirname(ghc), "ghc-pkg" + suffix)


def host_compiler_wrapper(ghc, scratch):
    """Returns a ghc that links with the host cc when the pinned bindist names no host tool."""
    info = subprocess.run([ghc, "--info"], stdout=subprocess.PIPE, text=True, check=True).stdout
    if NO_HOST_TOOL not in info or os.name == "nt":
        return ghc
    wrapper = os.path.join(scratch, "ghc")
    with open(wrapper, "w", encoding="utf-8", newline="\n") as handle:
        handle.write('#!/bin/sh\nexec "{}" -pgmc cc -pgml cc -pgma cc "$@"\n'.format(ghc))
    os.chmod(wrapper, 0o755)
    return wrapper


def write(args):
    """Resolves the project in a temp dir and writes the freeze and sidecar into the workspace."""
    package_dir = os.path.join(os.environ["BUILD_WORKSPACE_DIRECTORY"], args.package)
    names = [os.path.basename(path) for path in args.input]
    cabal = rlocation(args.cabal)
    ghc = rlocation(args.ghc)
    with tempfile.TemporaryDirectory(prefix="dx_haskell_resolve_") as scratch:
        project = os.path.join(scratch, "project")
        shutil.copytree(package_dir, project, ignore=shutil.ignore_patterns("cabal.project.freeze", "dist-newstyle", "haskell_deps.json"))
        state = index_state(os.path.join(project, "cabal.project"))
        cabal_dir = os.path.join(scratch, "cabal")
        env = dict(os.environ, CABAL_DIR=cabal_dir, PATH=os.path.dirname(ghc) + os.pathsep + os.environ.get("PATH", ""))
        env.pop("CABAL_CONFIG", None)
        base = [cabal, "--with-compiler=" + host_compiler_wrapper(ghc, scratch), "--with-hc-pkg=" + sibling_ghc_pkg(ghc)]
        run(base + ["update", "--index-state=" + state], project, env)
        run(base + ["freeze"], project, env)
        run(base + ["build", "--dry-run", "all"], project, env)
        with open(os.path.join(project, "dist-newstyle", "cache", "plan.json"), encoding="utf-8") as handle:
            plan = json.load(handle)
        packages = sidecar_from_plan(plan, {})
        wanted = [(pkg["name"], pkg["version"]) for pkg in packages if pkg["source"] == "hackage"]
        index = os.path.join(cabal_dir, "packages", "hackage.haskell.org", "01-index.tar")
        if not os.path.exists(index):
            index += ".gz"
        revisions = cabal_revisions(index, wanted)
        packages = sidecar_from_plan(plan, revisions)
        for package in packages:
            if package["source"] == "hackage" and package["cabal_revision"] is None:
                raise SystemExit("deps_resolve: no index entry matches the .cabal of {name}-{version}".format(**package))
        freeze_path = os.path.join(project, "cabal.project.freeze")
        sidecar = {
            "version": SIDECAR_VERSION,
            "index_state": state,
            "inputs_sha256": inputs_sha256([os.path.join(package_dir, name) for name in names]),
            "freeze_sha256": text_sha256(freeze_path),
            "packages": packages,
        }
        shutil.copyfile(freeze_path, os.path.join(package_dir, "cabal.project.freeze"))
        with open(os.path.join(package_dir, "haskell_deps.json"), "w", encoding="utf-8", newline="\n") as handle:
            json.dump(sidecar, handle, indent=2, sort_keys=True)
            handle.write("\n")
    return 0


def main(argv):
    """Parses the command line and runs write or check."""
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["write", "check"])
    parser.add_argument("--package", required=True)
    parser.add_argument("--label", default="")
    parser.add_argument("--cabal")
    parser.add_argument("--ghc")
    parser.add_argument("--input", action="append", default=[])
    parser.add_argument("--freeze")
    parser.add_argument("--sidecar")
    args = parser.parse_args(argv)
    return check(args) if args.mode == "check" else write(args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
