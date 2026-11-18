"""Reports patches whose upstream fix has shipped in a release.

Each patch under patches/ names its upstream change in a header line above the
diff, which every patch tool skips:

    Upstream: bazelbuild/rules_rust#4390
    Upstream: none (rules_dx only: gates the toolchains behind //config:toolchains_on)
    Upstream: fixed upstream in v0.9.0

A header may name several refs separated by commas. A ref is a pull request or
an issue. The report lists refs whose pull request is merged and contained in a
release tag, patches marked fixed in a release, refs that were closed without a
merge, and patches with no header.
"""

import json
import os
import re
import sys
import urllib.error
import urllib.request

_HEADER = re.compile(r"^Upstream:\s*(.+?)\s*$")
_REF = re.compile(r"^([\w.-]+/[\w.-]+)#(\d+)$")
_FIXED = re.compile(r"^fixed upstream in (\S+)$")
_DIFF_START = ("--- ", "diff ")

# The newest releases (or tags, for repositories without releases) checked for
# the merge commit. Upstream fixes that matter land in a recent release.
_RECENT = 20


def header(text):
    """Returns the Upstream value of a patch, or None when it has none.

    Only lines before the first diff line count, so a diff that adds an
    `Upstream:` line does not count as a header.
    """
    for line in text.splitlines():
        if line.startswith(_DIFF_START):
            return None
        match = _HEADER.match(line)
        if match:
            return match.group(1)
    return None


def refs(value):
    """Returns [(repo, number)] for a header value; [] for `none (...)`.

    Raises ValueError for a value that is neither, so a typo does not read as
    "no upstream".
    """
    if value.startswith("none"):
        if not re.match(r"^none \(.+\)$", value):
            raise ValueError("'none' needs a reason in parentheses: %r" % value)
        return []
    found = []
    for part in value.split(","):
        match = _REF.match(part.strip())
        if not match:
            raise ValueError("not an owner/repo#N ref: %r" % part.strip())
        found.append((match.group(1), int(match.group(2))))
    return found


class GitHub:
    """Minimal GitHub REST client. Reads GITHUB_TOKEN or GH_TOKEN when set."""

    def __init__(self, token):
        self._token = token

    def get(self, path):
        request = urllib.request.Request("https://api.github.com" + path)
        request.add_header("Accept", "application/vnd.github+json")
        request.add_header("User-Agent", "rules_dx-patch-upstream")
        if self._token:
            request.add_header("Authorization", "Bearer " + self._token)
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            raise


def shipped_in(api, repo, sha, merged_at):
    """Returns the newest release or tag of repo that contains sha, else None."""
    releases = api.get("/repos/%s/releases?per_page=%d" % (repo, _RECENT)) or []
    names = [r["tag_name"] for r in releases if not r["draft"] and not r["prerelease"]]
    if names:
        # A release published before the merge cannot contain it; ISO 8601
        # UTC timestamps compare as strings.
        names = [
            r["tag_name"]
            for r in releases
            if not r["draft"] and not r["prerelease"] and (r.get("published_at") or "") >= merged_at
        ]
    else:
        names = [t["name"] for t in api.get("/repos/%s/tags?per_page=%d" % (repo, _RECENT)) or []]
    for name in names:
        compare = api.get("/repos/%s/compare/%s...%s" % (repo, name, sha))
        # "behind" or "identical": sha is an ancestor of (or equal to) the tag.
        if compare and compare["status"] in ("behind", "identical"):
            return name
    return None


def status(api, repo, number):
    """Returns ("released", tag), ("closed", None) or ("open", None) for a ref."""
    issue = api.get("/repos/%s/issues/%d" % (repo, number))
    if issue is None:
        raise ValueError("%s#%d does not exist" % (repo, number))
    if issue["state"] == "open":
        return ("open", None)
    if "pull_request" not in issue:
        # A closed issue: someone has to find the fix and its release.
        return ("closed", None)
    pull = api.get("/repos/%s/pulls/%d" % (repo, number))
    if not pull["merged"]:
        return ("closed", None)
    tag = shipped_in(api, repo, pull["merge_commit_sha"], pull["merged_at"])
    return ("released", tag) if tag else ("open", None)


def patch_files(root):
    found = []
    for directory, _, files in os.walk(root):
        found.extend(os.path.join(directory, name) for name in files if name.endswith(".patch"))
    return sorted(found)


def report(api, root):
    """Returns the Markdown report, or "" when nothing needs a person."""
    released, closed, missing, bad = [], [], [], []
    for path in patch_files(root):
        # Forward slashes on every host, so the issue body reads the same.
        name = os.path.relpath(path, os.path.dirname(root)).replace(os.sep, "/")
        with open(path, encoding="utf-8") as handle:
            value = header(handle.read())
        if value is None:
            missing.append("- `%s`" % name)
            continue
        fixed = _FIXED.match(value)
        if fixed:
            released.append("- `%s`: fixed in %s" % (name, fixed.group(1)))
            continue
        try:
            found = refs(value)
        except ValueError as error:
            bad.append("- `%s`: %s" % (name, error))
            continue
        for repo, number in found:
            try:
                state, tag = status(api, repo, number)
            except ValueError as error:
                bad.append("- `%s`: %s" % (name, error))
                continue
            link = "https://github.com/%s/pull/%d" % (repo, number)
            if state == "released":
                released.append("- `%s`: [%s#%d](%s) is in %s" % (name, repo, number, link, tag))
            elif state == "closed":
                closed.append("- `%s`: [%s#%d](%s)" % (name, repo, number, link))
    sections = [
        ("Upstream fix released: bump the module and delete the patch", released),
        ("Upstream ref closed without a merge: find where the fix went", closed),
        ("No `Upstream:` header", missing),
        ("Unreadable `Upstream:` header", bad),
    ]
    out = []
    for title, lines in sections:
        if lines:
            out.append("## %s\n\n%s\n" % (title, "\n".join(lines)))
    return "\n".join(out)


def main(argv):
    root = argv[1] if len(argv) > 1 else os.path.join(os.environ.get("BUILD_WORKSPACE_DIRECTORY", "."), "patches")
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    # Bytes, so Windows writes LF and UTF-8 like the other runners.
    sys.stdout.buffer.write(report(GitHub(token), root).encode("utf-8"))


if __name__ == "__main__":
    main(sys.argv)
