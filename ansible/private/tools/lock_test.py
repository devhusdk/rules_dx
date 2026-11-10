"""Covers lock rendering from recorded Galaxy answers, with no network."""

import json
import unittest

import lock

RECORDED = {
    "community.general": {
        "highest_version": {"version": "9.0.0"},
        "9.0.0": {
            "download_url": "https://galaxy.example/community-general-9.0.0.tar.gz",
            "artifact": {"sha256": "a" * 64},
            "metadata": {"dependencies": {"ansible.utils": "*"}},
        },
    },
    "ansible.utils": {
        "highest_version": {"version": "5.1.2"},
        "5.1.2": {
            "download_url": "https://galaxy.example/ansible-utils-5.1.2.tar.gz",
            "artifact": {"sha256": "b" * 64},
            "metadata": {"dependencies": {}},
        },
    },
}


def fake_fetch(url):
    rest = url[len(lock.GALAXY) + 1:].rstrip("/").split("/")
    collection = RECORDED[rest[0] + "." + rest[1]]
    return collection[rest[3]] if len(rest) > 2 else collection


class LockTest(unittest.TestCase):
    def test_adds_transitive_dependencies_sorted(self):
        parsed = json.loads(lock.update("", "community.general", None, fake_fetch))
        self.assertEqual([c["name"] for c in parsed["collections"]], ["ansible.utils", "community.general"])
        general = parsed["collections"][1]
        self.assertEqual(general["version"], "9.0.0")
        self.assertEqual(general["sha256"], "a" * 64)
        self.assertEqual(general["urls"], ["https://galaxy.example/community-general-9.0.0.tar.gz"])

    def test_keeps_locked_dependency(self):
        old = {"name": "ansible.utils", "sha256": "c" * 64, "urls": ["u"], "version": "1.0.0"}
        text = json.dumps({"collections": [old]})
        parsed = json.loads(lock.update(text, "community.general", None, fake_fetch))
        self.assertEqual(parsed["collections"][0], old)

    def test_explicit_version_replaces_entry(self):
        old = {"name": "ansible.utils", "sha256": "c" * 64, "urls": ["u"], "version": "1.0.0"}
        text = json.dumps({"collections": [old]})
        parsed = json.loads(lock.update(text, "ansible.utils", "5.1.2", fake_fetch))
        self.assertEqual(parsed["collections"][0]["version"], "5.1.2")


if __name__ == "__main__":
    unittest.main()
