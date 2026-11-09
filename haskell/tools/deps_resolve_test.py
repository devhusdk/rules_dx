"""Covers plan.json conversion, the index revisions and the offline staleness check."""

import hashlib
import io
import json
import os
import tarfile
import tempfile
import unittest

import deps_resolve

CABAL_BYTES = b"name: foo\nversion: 1.0\n"
CABAL_SHA = hashlib.sha256(CABAL_BYTES).hexdigest()

PLAN = {
    "install-plan": [
        {"type": "pre-existing", "id": "base-4.20", "pkg-name": "base", "pkg-version": "4.20", "depends": []},
        {
            "type": "configured", "id": "foo-1.0-inplace", "pkg-name": "foo", "pkg-version": "1.0",
            "pkg-src": {"type": "repo-tar"}, "pkg-src-sha256": "aa", "pkg-cabal-sha256": CABAL_SHA,
            "component-name": "lib", "depends": ["base-4.20"],
        },
        {
            "type": "configured", "id": "foo-1.0-exe", "pkg-name": "foo", "pkg-version": "1.0",
            "pkg-src": {"type": "repo-tar"}, "pkg-src-sha256": "aa", "pkg-cabal-sha256": CABAL_SHA,
            "component-name": "exe:foo-tool", "depends": ["foo-1.0-inplace", "base-4.20"],
        },
        {
            "type": "configured", "id": "foo-1.0-setup", "pkg-name": "foo", "pkg-version": "1.0",
            "pkg-src": {"type": "repo-tar"}, "pkg-src-sha256": "aa", "pkg-cabal-sha256": CABAL_SHA,
            "component-name": "setup", "depends": ["base-4.20"],
        },
        {
            "type": "configured", "id": "app-0.1-inplace", "pkg-name": "app", "pkg-version": "0.1",
            "pkg-src": {"type": "local"}, "component-name": "lib", "depends": ["foo-1.0-inplace"],
        },
    ],
}


def write_index(path, entries):
    """Writes a tar of (name, bytes) entries."""
    with tarfile.open(path, "w") as tar:
        for name, data in entries:
            info = tarfile.TarInfo(name)
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))


class DepsResolveTest(unittest.TestCase):
    """Tests the pure parts of the resolver."""

    def test_plan_to_packages(self):
        packages = deps_resolve.sidecar_from_plan(PLAN, {("foo", "1.0", CABAL_SHA): 2})
        by_name = {package["name"]: package for package in packages}
        self.assertEqual(by_name["base"]["source"], "ghc")
        self.assertEqual(by_name["app"]["source"], "local")
        self.assertEqual(by_name["app"]["dependencies"], ["foo"])
        foo = by_name["foo"]
        self.assertEqual(foo["source"], "hackage")
        self.assertEqual(foo["components"], ["exe:foo-tool", "lib"])
        self.assertEqual(foo["dependencies"], ["base"])
        self.assertEqual(foo["cabal_revision"], 2)
        self.assertEqual(foo["sdist_sha256"], "aa")

    def test_remote_tarball_is_a_url_package(self):
        url = "https://example.invalid/bar-2.0.tar.gz"
        plan = {"install-plan": [
            {"type": "pre-existing", "id": "base-4.20", "pkg-name": "base", "pkg-version": "4.20", "depends": []},
            {
                "type": "configured", "id": "bar-2.0-exe", "pkg-name": "bar", "pkg-version": "2.0",
                "pkg-src": {"type": "remote-tar", "uri": url},
                "pkg-src-sha256": "bb", "component-name": "exe:bar", "depends": ["base-4.20"],
            },
        ]}
        bar = {package["name"]: package for package in deps_resolve.sidecar_from_plan(plan, {})}["bar"]
        self.assertEqual(bar["source"], "url")
        self.assertEqual(bar["url"], url)
        self.assertEqual(bar["sdist_sha256"], "bb")
        self.assertEqual(bar["components"], ["exe:bar"])
        self.assertNotIn("cabal_revision", bar)

    def test_revision_is_the_position_among_cabal_entries(self):
        with tempfile.TemporaryDirectory() as scratch:
            index = os.path.join(scratch, "01-index.tar")
            write_index(index, [
                ("foo/1.0/foo.cabal", b"name: foo\nversion: 1.0\n-- rev0\n"),
                ("bar/1.0/bar.cabal", b"x"),
                ("foo/1.0/foo.cabal", CABAL_BYTES),
            ])
            found = deps_resolve.cabal_revisions(index, [("foo", "1.0")])
        self.assertEqual(found[("foo", "1.0", CABAL_SHA)], 1)

    def test_freeze_pairs(self):
        text = "active-repositories: hackage.haskell.org\nconstraints: any.base ==4.20.2.0,\n             any.foo ==1.0,\n             foo +flag\n"
        self.assertEqual(deps_resolve.freeze_pairs(text), [("base", "4.20.2.0"), ("foo", "1.0")])

    def test_index_state_is_required(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = os.path.join(scratch, "cabal.project")
            with open(path, "w", encoding="utf-8") as handle:
                handle.write("packages: .\n")
            with self.assertRaises(SystemExit):
                deps_resolve.index_state(path)
            with open(path, "w", encoding="utf-8") as handle:
                handle.write("packages: .\nindex-state: 2026-01-01T00:00:00Z\n")
            self.assertEqual(deps_resolve.index_state(path), "2026-01-01T00:00:00Z")


SNAPSHOT = b"""name: lts-1
packages:
- hackage: aeson-2.2.1.0@sha256:aa,100
  pantry-tree:
    sha256: bb
- hackage: text-2.0.2
"""
SNAPSHOT_SHA = hashlib.sha256(SNAPSHOT).hexdigest()
STACK_LOCK = """packages:
- completed:
    hackage: text-2.1@sha256:cc,200
    pantry-tree:
      sha256: dd
  original:
    hackage: text-2.1
snapshots:
- completed:
    sha256: {}
    size: 10
    url: https://example.test/lts-1.yaml
  original: lts-1
""".format(SNAPSHOT_SHA)


class StackLockTest(unittest.TestCase):
    def test_snapshot_then_extra_deps(self):
        pins = deps_resolve.stack_lock_pins(STACK_LOCK, lambda url: SNAPSHOT)
        self.assertEqual(pins, {"aeson": "2.2.1.0", "text": "2.1"})

    def test_snapshot_digest_mismatch_fails(self):
        with self.assertRaises(SystemExit):
            deps_resolve.stack_lock_pins(STACK_LOCK, lambda url: SNAPSHOT + b"x")

    def test_freeze_round_trips_through_freeze_pairs(self):
        text = deps_resolve.stack_freeze_text({"aeson": "2.2.1.0", "text": "2.1"})
        self.assertEqual(deps_resolve.freeze_pairs(text), [("aeson", "2.2.1.0"), ("text", "2.1")])

    def test_sidecar_freeze_skips_local_packages(self):
        packages = [{"name": "app", "version": "0.1", "source": "local"}, {"name": "text", "version": "2.1", "source": "hackage"}]
        text = deps_resolve.sidecar_freeze_text(packages, "2026-01-01T00:00:00Z")
        self.assertEqual(deps_resolve.freeze_pairs(text), [("text", "2.1")])
        self.assertIn("index-state: hackage.haskell.org 2026-01-01T00:00:00Z", text)


class CheckTest(unittest.TestCase):
    """Tests the staleness check against real files."""

    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        root = self.scratch.name
        os.environ["RUNFILES_DIR"] = root
        self.addCleanup(os.environ.pop, "RUNFILES_DIR")
        self.files = {}
        for name, text in {"app.cabal": "name: app\n", "cabal.project": "packages: .\nindex-state: 2026-01-01T00:00:00Z\n", "cabal.project.freeze": "constraints: any.foo ==1.0\n"}.items():
            self.write(name, text)
        self.write_sidecar()

    def write(self, name, text):
        """Writes one runfile."""
        with open(os.path.join(self.scratch.name, name), "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)

    def path(self, name):
        """Returns the on-disk path of one runfile."""
        return os.path.join(self.scratch.name, name)

    def write_sidecar(self):
        """Writes a sidecar that matches the current files."""
        sidecar = {
            "version": deps_resolve.SIDECAR_VERSION,
            "inputs_sha256": deps_resolve.inputs_sha256([self.path("app.cabal"), self.path("cabal.project")]),
            "freeze_sha256": deps_resolve.text_sha256(self.path("cabal.project.freeze")),
            "packages": [{"name": "foo", "version": "1.0", "source": "hackage"}, {"name": "app", "version": "0.1", "source": "local"}],
        }
        self.write("haskell_deps.json", json.dumps(sidecar))

    def run_check(self):
        """Runs the check and returns its exit code."""
        return deps_resolve.main(["check", "--package=p", "--freeze=cabal.project.freeze", "--sidecar=haskell_deps.json", "--input=app.cabal", "--input=cabal.project"])

    def test_fresh_files_pass(self):
        self.assertEqual(self.run_check(), 0)

    def test_edited_freeze_fails(self):
        self.write("cabal.project.freeze", "constraints: any.foo ==1.1\n")
        self.assertEqual(self.run_check(), 1)

    def test_edited_cabal_file_fails(self):
        self.write("app.cabal", "name: app\nbuild-depends: foo\n")
        self.assertEqual(self.run_check(), 1)


if __name__ == "__main__":
    unittest.main()
