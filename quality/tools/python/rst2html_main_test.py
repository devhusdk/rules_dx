"""Tests for the rst2html entry point."""

import io
import sys
import unittest
from unittest import mock

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
        """Converts one body through stdin in this process and returns stdout.

        It runs in-process because on Windows a child interpreter does not find the venv.
        """
        stdout = io.BytesIO()
        stdin = io.TextIOWrapper(io.BytesIO(body), encoding="utf-8")
        out = io.TextIOWrapper(stdout, encoding="utf-8")
        with mock.patch.object(sys, "argv", VALE_ARGV), mock.patch.object(sys, "stdin", stdin), mock.patch.object(
            sys, "stdout", out
        ):
            rst2html_main.main()
            out.flush()
        return stdout.getvalue().decode("utf-8")

    def test_converts_non_ascii_stdin_to_html(self):
        """A heading and non-ASCII text land in the HTML."""
        html = self.run_main("Titel\n=====\n\nBlåbærgrød med *fed*.\n".encode())
        self.assertIn("<h1", html)
        self.assertIn("Blåbærgrød", html)
        self.assertIn("<em>fed</em>", html)


if __name__ == "__main__":
    unittest.main()
