"""Reports patches whose upstream fix has shipped in a release.

Each patch under patches/ names its upstream change in a header line above the
diff, which every patch tool skips:

    Upstream: bazelbuild/rules_rust#4390
    Upstream: aran/rules_flutter@b12fd3fbfb130bb1b90ba03c790f3a8ce101363f
    Upstream: none (rules_dx only: gates the toolchains behind //config:toolchains_on)
    Upstream: fixed upstream in v0.9.0

A header may name several refs separated by commas. A ref is a pull request, an
issue, or a full commit sha for a fix that landed without a pull request. The
report lists refs whose pull request is merged, or closed by the commit that
landed it, and contained in a release tag, merged bazelbuild/bazel-central-registry
pull requests, commits contained in a release tag, patches marked fixed in a
release, refs that were closed without a merge, and patches with no header.

A carry is rules_dx code that stands in for an upstream fix, such as extra
toolchain args. It names the fix in a comment line of a BUILD or .bzl file,
which the report reads like a patch header:

    # Upstream: hermeticbuild/hermetic-llvm#973
"""

import json
import os
import re
import sys
import urllib.error
import urllib.request

_HEADER = re.compile(r"^Upstream:\s*(.+?)\s*$")
_CARRY = re.compile(r"^\s*#\s*Upstream:\s*(.+?)\s*$")
_REF = re.compile(r"^([\w.-]+/[\w.-]+)#(\d+)$")
_COMMIT = re.compile(r"^([\w.-]+/[\w.-]+)@([0-9a-f]{40})$")
_FIXED = re.compile(r"^fixed upstream in (\S+)$")
# Keep in sync with _DIFF_START in tools/ci/repos.bzl.
_DIFF_START = ("--- ", "diff ")

# The newest releases (or tags, for repositories without releases) checked for
# the merge commit. Upstream fixes that matter land in a recent release.
_RECENT = 20

# Repositories that publish a pull request when it merges, with no release or tag.
_SHIPS_ON_MERGE = ("bazelbuild/bazel-central-registry",)


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


def carry_headers(text):
    """Returns the value of every `# Upstream:` comment line in a BUILD or .bzl file."""
    return [match.group(1) for match in map(_CARRY.match, text.splitlines()) if match]


def is_carry_file(name):
    """Returns whether a file name is one that may hold carry markers."""
    base = name.replace(os.sep, "/").rpartition("/")[2]
    return base in ("BUILD", "BUILD.bazel") or base.endswith(".bzl")


def refs(value):
    """Returns [(repo, number or sha)] for a header value; [] for `none (...)`.

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
        if match:
            found.append((match.group(1), int(match.group(2))))
            continue
        match = _COMMIT.match(part.strip())
        if not match:
            raise ValueError("not an owner/repo#N ref or owner/repo@<40-hex sha>: %r" % part.strip())
        found.append((match.group(1), match.group(2)))
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
    if pull["merged"] and repo in _SHIPS_ON_MERGE:
        return ("released", "the BCR")
    if pull["merged"]:
        sha, merged_at = pull["merge_commit_sha"], pull["merged_at"]
    else:
        landed = landed_commit(api, repo, number)
        if landed is None:
            return ("closed", None)
        sha, merged_at = landed
    tag = shipped_in(api, repo, sha, merged_at)
    return ("released", tag) if tag else ("open", None)


def commit_status(api, repo, sha):
    """Returns ("released", tag) or ("open", None) for a commit ref."""
    try:
        commit = api.get("/repos/%s/commits/%s" % (repo, sha))
    except urllib.error.HTTPError as error:
        if error.code != 422:
            raise
        commit = None
    if commit is None:
        raise ValueError("%s@%s does not exist" % (repo, sha))
    # The committer date, not the author date: a rebase keeps the author date.
    tag = shipped_in(api, repo, sha, commit["commit"]["committer"]["date"])
    return ("released", tag) if tag else ("open", None)


def landed_commit(api, repo, number):
    """Returns (sha, closed_at) when a commit closed the pull request, else None.

    Repositories that import pull requests with Copybara, such as Bazel, land
    them as a separate commit and close the pull request unmerged.
    """
    events = api.get("/repos/%s/issues/%d/events?per_page=100" % (repo, number)) or []
    for event in reversed(events):
        if event.get("event") == "closed" and event.get("commit_id"):
            return (event["commit_id"], event["created_at"])
    return None


def patch_files(root):
    found = []
    for directory, _, files in os.walk(root):
        found.extend(os.path.join(directory, name) for name in files if name.endswith(".patch"))
    return sorted(found)


def carry_files(workspace):
    """Returns every BUILD and .bzl file under workspace, sorted.

    Skips hidden directories, Bazel's convenience symlinks and node_modules.
    """
    found = []
    for directory, dirs, files in os.walk(workspace):
        dirs[:] = [d for d in dirs if not d.startswith((".", "bazel-")) and d != "node_modules"]
        found.extend(os.path.join(directory, name) for name in files if is_carry_file(name))
    return sorted(found)


def _classify(api, name, value, released, closed, bad):
    """Appends one header or carry value of file name to its report section."""
    fixed = _FIXED.match(value)
    if fixed:
        released.append("- `%s`: fixed in %s" % (name, fixed.group(1)))
        return
    try:
        found = refs(value)
    except ValueError as error:
        bad.append("- `%s`: %s" % (name, error))
        return
    for repo, ref in found:
        try:
            if isinstance(ref, str):
                state, tag = commit_status(api, repo, ref)
            else:
                state, tag = status(api, repo, ref)
        except ValueError as error:
            bad.append("- `%s`: %s" % (name, error))
            continue
        if isinstance(ref, str):
            label = "%s@%s" % (repo, ref[:7])
            link = "https://github.com/%s/commit/%s" % (repo, ref)
        else:
            label = "%s#%d" % (repo, ref)
            link = "https://github.com/%s/pull/%d" % (repo, ref)
        if state == "released":
            released.append("- `%s`: [%s](%s) is in %s" % (name, label, link, tag))
        elif state == "closed":
            closed.append("- `%s`: [%s](%s)" % (name, label, link))


def report(api, root):
    """Returns the Markdown report, or "" when nothing needs a person.

    root is the patches directory; carries are read from its parent, the
    workspace. A file without a carry marker is never listed as missing one.
    """
    released, closed, missing, bad = [], [], [], []
    workspace = os.path.dirname(root)
    for path in patch_files(root):
        # Forward slashes on every host, so the issue body reads the same.
        name = os.path.relpath(path, workspace).replace(os.sep, "/")
        with open(path, encoding="utf-8") as handle:
            value = header(handle.read())
        if value is None:
            missing.append("- `%s`" % name)
            continue
        _classify(api, name, value, released, closed, bad)
    for path in carry_files(workspace):
        name = os.path.relpath(path, workspace).replace(os.sep, "/")
        with open(path, encoding="utf-8") as handle:
            values = carry_headers(handle.read())
        for value in values:
            _classify(api, name, value, released, closed, bad)
    sections = [
        ("Upstream fix released: bump the module and delete the patch or carry", released),
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
