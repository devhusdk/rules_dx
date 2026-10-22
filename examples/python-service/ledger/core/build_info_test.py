"""Generated module, package identity and resolved dependency checks."""

import hashlib
import os
import unittest
from importlib import resources

import click
import ledger.core
from ledger.core import build_info, gen_build_info


class BuildInfoTest(unittest.TestCase):
    def test_generated_module_matches_packaged_data(self):
        raw = resources.files("ledger.core").joinpath("items.json").read_bytes()
        self.assertEqual(build_info.ITEMS_SHA256, hashlib.sha256(raw).hexdigest())
        self.assertEqual(build_info.ITEM_COUNT, 3)
        self.assertEqual(build_info.SKUS, ("bolt-m6", "nut-m6", "washer-m6"))
        self.assertEqual(build_info.SCHEMA_VERSION, 1)

    def test_render_reflects_input_bytes(self):
        text = gen_build_info.render(b'[{"sku": "a", "name": "A", "price_cents": 1}]')
        self.assertIn("ITEM_COUNT = 1", text)
        self.assertIn('SKUS = (\n    "a",\n)', text)

    def test_package_identity(self):
        core_dir = os.path.dirname(ledger.core.__file__)
        self.assertTrue(core_dir.endswith(os.path.join("ledger", "core")), core_dir)
        self.assertEqual(os.path.dirname(build_info.__file__), core_dir)
        self.assertTrue(os.path.exists(os.path.join(core_dir, "items.json")))
        self.assertIsNone(getattr(ledger, "__file__", None))

    def test_dependency_comes_from_the_locked_hub(self):
        self.assertEqual(click.__version__, "8.1.8")
        self.assertIn("site-packages", click.__file__)
        self.assertFalse(
            any(
                part in click.__file__
                for part in (
                    "/usr/lib/python",
                    "/usr/local/lib/python",
                    "dist-packages",
                )
            ),
            click.__file__,
        )


if __name__ == "__main__":
    unittest.main()
