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

README = "text\n\n| Module | Version | Patches | Why |\n| --- | --- | --- | --- |\n| `foo` | 1.0 | `a.patch` | Root only. Because |\n"


class RenderTest(unittest.TestCase):
    def test_sets_version(self):
        self.assertIn("| `foo` | 2.0 | `a.patch` | Root only. Because |", render.render(MODULE, README))

    def test_header_untouched(self):
        self.assertIn("| Module | Version | Patches | Why |", render.render(MODULE, README))

    def test_unclassified_row(self):
        with self.assertRaises(SystemExit):
            render.render(MODULE, README.replace("Root only. ", ""))

    def test_consumer_fix_row(self):
        readme = README.replace("Root only. Because", "Because. Consumer fix: #7")
        self.assertIn("Consumer fix: #7 |", render.render(MODULE, readme))

    def test_unknown_module(self):
        with self.assertRaises(SystemExit):
            render.render("", README)

    def test_unknown_patch(self):
        with self.assertRaises(SystemExit):
            render.render(MODULE.replace("a.patch", "b.patch"), README)

    def test_unlisted_patch(self):
        with self.assertRaises(SystemExit):
            render.render(MODULE.replace('["//patches/foo:a.patch"]', '["//patches/foo:a.patch", "//patches/foo:b.patch"]'), README)

    def test_unlisted_module(self):
        module = MODULE + MODULE.replace("foo", "bar")
        with self.assertRaises(SystemExit):
            render.render(module, README)

    def test_patches_across_rows(self):
        readme = README + "| `foo` | 1.0 | `b.patch` | Consumer fix: #1 |\n"
        module = MODULE.replace('["//patches/foo:a.patch"]', '["//patches/foo:a.patch", "//patches/foo:b.patch"]')
        self.assertIn("| `foo` | 2.0 | `b.patch` | Consumer fix: #1 |", render.render(module, readme))

    def test_repo_rule_patch_unlisted(self):
        module = MODULE + 'dx_pins.archive(\n    name = "bar",\n    patches = ["//patches/bar:x.patch"],\n)\n'
        with self.assertRaises(SystemExit):
            render.render(module, README)

    def test_repo_rule_patch_listed(self):
        module = MODULE + 'dx_pins.archive(\n    name = "bar",\n    patches = ["//patches/bar:x.patch"],\n)\n'
        readme = README + "\n## bar\n\nPatches `x.patch`.\n"
        self.assertIn("## bar", render.render(module, readme))

    def test_repo_rule_patch_in_other_section(self):
        module = MODULE + 'dx_pins.archive(\n    name = "bar",\n    patches = ["//patches/bar:x.patch"],\n)\n'
        readme = README + "\n## baz\n\nPatches `x.patch`.\n"
        with self.assertRaises(SystemExit):
            render.render(module, readme)


if __name__ == "__main__":
    unittest.main()
