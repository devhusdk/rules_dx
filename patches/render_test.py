import unittest

import render

MODULE = '''
single_version_override(
    module_name = "foo",
    patch_strip = 1,
    patches = ["//patches/foo:a.patch"],
    version = "2.0",
)
'''

README = "text\n\n| Module | Version | Patches | Why |\n| --- | --- | --- | --- |\n| `foo` | 1.0 | `a.patch` | Because |\n"


class RenderTest(unittest.TestCase):
    def test_sets_version(self):
        self.assertIn("| `foo` | 2.0 | `a.patch` | Because |", render.render(MODULE, README))

    def test_header_untouched(self):
        self.assertIn("| Module | Version | Patches | Why |", render.render(MODULE, README))

    def test_unknown_module(self):
        with self.assertRaises(SystemExit):
            render.render("", README)

    def test_unknown_patch(self):
        with self.assertRaises(SystemExit):
            render.render(MODULE.replace("a.patch", "b.patch"), README)

    def test_unlisted_patch(self):
        with self.assertRaises(SystemExit):
            render.render(MODULE.replace('["//patches/foo:a.patch"]', '["//patches/foo:a.patch", "//patches/foo:b.patch"]'), README)

    def test_patches_across_rows(self):
        readme = README + "| `foo` | 1.0 | `b.patch` | More |\n"
        module = MODULE.replace('["//patches/foo:a.patch"]', '["//patches/foo:a.patch", "//patches/foo:b.patch"]')
        self.assertIn("| `foo` | 2.0 | `b.patch` | More |", render.render(module, readme))


if __name__ == "__main__":
    unittest.main()
