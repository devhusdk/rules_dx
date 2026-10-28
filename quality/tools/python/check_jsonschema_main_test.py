"""Tests for the check_jsonschema entry point."""

import importlib
import json
import os
import subprocess
import sys
import tempfile
import unittest

import check_jsonschema_main

# The drafts dx accepts, mirrored by `DRAFTS` in quality/adapter/src/json_schema.rs.
ACCEPTED = {
    "http://json-schema.org/draft-04/schema",
    "http://json-schema.org/draft-06/schema",
    "http://json-schema.org/draft-07/schema",
    "https://json-schema.org/draft/2019-09/schema",
    "https://json-schema.org/draft/2020-12/schema",
}
DRAFT_03 = "http://json-schema.org/draft-03/schema"

DRIVER = """
import runpy, socket, sys
def refuse(*args, **kwargs):
    raise OSError("network blocked")
socket.socket.connect = refuse
socket.create_connection = refuse
sys.argv = sys.argv[1:]
runpy.run_path(sys.argv[0], run_name="__main__")
"""


class CheckJsonschemaMainTest(unittest.TestCase):
    """Checks the offline reference map."""

    def run_tool(self, schema, instance, mapping):
        """Runs the wrapper with sockets blocked and returns the process."""
        with tempfile.TemporaryDirectory() as tmp:
            paths = {}
            for name, body in (("schema.json", schema), ("data.json", instance)):
                paths[name] = os.path.join(tmp, name)
                with open(paths[name], "w", encoding="utf-8") as handle:
                    json.dump(body, handle)
            refs = os.path.join(tmp, "refs.json")
            with open(refs, "w", encoding="utf-8") as handle:
                json.dump(mapping(tmp), handle)
            main = os.path.join(
                os.path.dirname(os.path.abspath(check_jsonschema_main.__file__)),
                "check_jsonschema_main.py",
            )
            return subprocess.run(
                [
                    sys.executable,
                    "-c",
                    DRIVER,
                    main,
                    "--dx-ref-map",
                    refs,
                    "--schemafile",
                    paths["schema.json"],
                    "--no-cache",
                    paths["data.json"],
                ],
                capture_output=True,
                text=True,
                check=False,
            )

    def test_patch_target_exists(self):
        """The private hook the wrapper replaces is still there."""
        resolver = importlib.import_module("check_jsonschema.schema_loader.resolver")
        self.assertTrue(callable(resolver.create_retrieve_callable))

    def test_pop_ref_map_removes_the_flag(self):
        """Both spellings leave the tool's own argv clean."""
        argv = ["x", "--dx-ref-map", "m.json", "--a"]
        self.assertEqual(check_jsonschema_main.pop_ref_map(argv), "m.json")
        self.assertEqual(argv, ["x", "--a"])
        argv = ["x", "--dx-ref-map=n.json"]
        self.assertEqual(check_jsonschema_main.pop_ref_map(argv), "n.json")
        self.assertEqual(argv, ["x"])

    def test_mapped_ref_resolves_without_network(self):
        """A pinned https ref is served from disk."""
        schema = {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.test/root.json",
            "properties": {"name": {"$ref": "defs.json"}},
        }

        def mapping(tmp):
            defs = os.path.join(tmp, "defs.json")
            with open(defs, "w", encoding="utf-8") as handle:
                json.dump({"type": "string"}, handle)
            return {"https://example.test/defs.json": defs}

        good = self.run_tool(schema, {"name": "a"}, mapping)
        self.assertEqual(good.returncode, 0, good.stderr)
        bad = self.run_tool(schema, {"name": 1}, mapping)
        self.assertEqual(bad.returncode, 1, bad.stderr)

    def test_unmapped_https_ref_fails(self):
        """An https ref outside the map fails instead of downloading."""
        schema = {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.test/root.json",
            "properties": {"name": {"$ref": "other.json"}},
        }
        result = self.run_tool(schema, {"name": "a"}, lambda tmp: {})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("other.json", result.stdout + result.stderr)

    def test_accepted_drafts_match_jsonschema(self):
        """dx accepts exactly the metaschemas jsonschema knows, minus draft-03."""
        validators = importlib.import_module("jsonschema.validators")
        known = {str(uri).rstrip("#") for uri in validators._META_SCHEMAS}
        self.assertEqual(known - {DRAFT_03}, ACCEPTED)


if __name__ == "__main__":
    unittest.main()
