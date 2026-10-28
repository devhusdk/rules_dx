"""Tests for the rst2html entry point."""

import os
import subprocess
import sys
import unittest

import rst2html_main

VALE_ARGV = [
    "rst2html",
    "--quiet",
    "--halt=5",
    "--report=5",
    "--link-stylesheet",
    "--no-file-insertion",
    "--no-toc-backlinks",
    "--no-footnote-backlinks",
    "--no-section-numbering",
]


class Rst2HtmlTest(unittest.TestCase):
    """Feeds Vale's exact argv and checks the HTML."""

    def run_main(self, body: bytes) -> str:
        """Converts one body through stdin in a child process and returns stdout."""
        main = os.path.join(os.path.dirname(os.path.abspath(rst2html_main.__file__)), "rst2html_main.py")
        result = subprocess.run(
            [sys.executable, main] + VALE_ARGV[1:],
            input=body,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        return result.stdout.decode("utf-8")

    def test_converts_non_ascii_stdin_to_html(self):
        """A heading and non-ASCII text land in the HTML."""
        html = self.run_main("Titel\n=====\n\nBlåbærgrød med *fed*.\n".encode())
        self.assertIn("<h1", html)
        self.assertIn("Blåbærgrød", html)
        self.assertIn("<em>fed</em>", html)


if __name__ == "__main__":
    unittest.main()
