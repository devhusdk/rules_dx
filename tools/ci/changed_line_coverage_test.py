"""Runs the changed-line gate and real diff-cover over small real Git histories."""

import json
import os
import subprocess
import sys

from workflow_testing import ROOT

GATE = "tools/ci/changed_line_coverage.py"
APP = "".join(f"line_{number} = {number}\n" for number in range(1, 6))


def git(repo, *args):
    """Runs one git command in the repository and returns its output."""
    done = subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        cwd=repo,
        capture_output=True,
        check=True,
        encoding="utf-8",
    )
    return done.stdout.strip()


def commit(repo, files, message="change"):
    """Writes the files, commits them and returns the new commit."""
    for name, text in files.items():
        path = repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", message)
    return git(repo, "rev-parse", "HEAD")


def make_repo(tmp_path, name="repo"):
    """Returns a repository whose first commit holds app.py and a README."""
    repo = tmp_path / name
    repo.mkdir()
    git(repo, "init", "-q", "-b", "main")
    commit(repo, {"app.py": APP, "README.md": "hello\n"}, "base")
    return repo


def lcov(entries):
    """Returns an LCOV report for {path: {line: hits}}."""
    out = []
    for path, lines in entries.items():
        out.append(f"SF:{path}")
        out.extend(f"DA:{line},{hits}" for line, hits in sorted(lines.items()))
        out.append(f"LF:{len(lines)}")
        out.append(f"LH:{sum(1 for hits in lines.values() if hits)}")
        out.append("end_of_record")
    return "\n".join(out) + "\n"


def sources(path, absent=(), owned=None):
    """Writes a sources record and returns its path."""
    if owned is None:
        owned = {"app.py": "eligible", **{item: "absent" for item in absent}}
    path.write_text(
        json.dumps(
            {
                "record_version": 1,
                "cell": "linux_x86_64",
                "owned": owned,
                "absent": list(absent),
                "errors": [],
            }
        ),
        encoding="utf-8",
    )
    return path


def run_gate(
    repo,
    report,
    base="main~0",
    threshold=80,
    extra=(),
    pythonpath=(),
    absent=(),
    record=None,
):
    """Runs the gate in the repository and returns status and text."""
    if record is None:
        record = sources(report.parent / f"{report.stem}.sources.json", absent)
    env = dict(
        os.environ, PYTHONPATH=os.pathsep.join([*map(str, pythonpath), *sys.path])
    )
    done = subprocess.run(
        [
            sys.executable,
            str(ROOT / GATE),
            "--base",
            base,
            "--lcov",
            str(report),
            "--sources",
            str(record),
            "--threshold",
            str(threshold),
            *extra,
        ],
        cwd=repo,
        capture_output=True,
        encoding="utf-8",
        env=env,
    )
    return done.returncode, done.stdout + done.stderr


def feature(tmp_path, report_text, files, threshold=80, absent=(), owned=None):
    """Branches off main, commits the files and runs the gate against main."""
    repo = make_repo(tmp_path)
    git(repo, "checkout", "-q", "-b", "topic")
    commit(repo, files)
    report = tmp_path / "coverage.lcov"
    report.write_text(report_text, encoding="utf-8")
    record = sources(tmp_path / "sources.json", absent, owned)
    return repo, run_gate(
        repo, report, base="main", threshold=threshold, record=record
    )


def test_a_new_covered_line_passes_and_reports_the_revision_pair(tmp_path):
    repo, (code, text) = feature(
        tmp_path,
        lcov({"app.py": {1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1}}),
        {"app.py": APP + "line_6 = 6\n"},
    )
    assert code == 0, text
    assert "changed executable lines: 1 of 1 covered (100.00%)" in text
    assert "threshold: 80%" in text
    assert "complete: yes" in text
    assert "result: pass" in text
    assert f"head: {git(repo, 'rev-parse', 'HEAD')}" in text
    assert f"base: {git(repo, 'rev-parse', 'main')}" in text
    assert f"merge-base: {git(repo, 'rev-parse', 'main')}" in text


def test_a_new_uncovered_line_fails_and_names_its_location(tmp_path):
    _, (code, text) = feature(
        tmp_path,
        lcov({"app.py": {1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 0}}),
        {"app.py": APP + "line_6 = 6\n"},
    )
    assert code == 1, text
    assert "changed executable lines: 0 of 1 covered (0.00%)" in text
    assert "uncovered: app.py:6" in text
    assert "result: fail" in text


def test_the_threshold_is_exact_not_rounded(tmp_path):
    added = "".join(f"extra_{n} = {n}\n" for n in range(7))
    hits = {number: 1 for number in range(1, 6)}
    hits.update({6: 1, 7: 1, 8: 1, 9: 1, 10: 1, 11: 0, 12: 0})
    report = lcov({"app.py": hits})
    _, (code, text) = feature(tmp_path, report, {"app.py": APP + added}, threshold=72)
    assert code == 1, text
    assert "5 of 7 covered (71.43%)" in text
    (tmp_path / "again").mkdir()
    _, (code, text) = feature(
        tmp_path / "again", report, {"app.py": APP + added}, threshold=71
    )
    assert code == 0, text


def test_non_executable_only_changes_are_not_applicable(tmp_path):
    _, (code, text) = feature(
        tmp_path,
        lcov({"app.py": {1: 1, 2: 1, 3: 1, 4: 1, 5: 1}}),
        {"README.md": "hello\nmore words\n", "app.py": APP + "# only a comment\n"},
    )
    assert code == 0, text
    assert "changed executable lines: none" in text
    assert "result: not-applicable" in text
    assert "100.00" not in text


def test_a_pure_rename_adds_no_lines(tmp_path):
    repo = make_repo(tmp_path)
    git(repo, "checkout", "-q", "-b", "topic")
    git(repo, "mv", "app.py", "renamed.py")
    git(repo, "commit", "-q", "-m", "rename")
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"renamed.py": {1: 0, 2: 0}}), encoding="utf-8")
    code, text = run_gate(repo, report, base="main")
    assert code == 0, text
    assert "result: not-applicable" in text


def test_a_rename_with_an_edit_gates_only_the_edit(tmp_path):
    repo = make_repo(tmp_path)
    git(repo, "checkout", "-q", "-b", "topic")
    git(repo, "mv", "app.py", "renamed.py")
    commit(repo, {"renamed.py": APP + "line_6 = 6\n"})
    report = tmp_path / "coverage.lcov"
    report.write_text(
        lcov({"renamed.py": {1: 0, 2: 0, 3: 0, 4: 0, 5: 0, 6: 1}}), encoding="utf-8"
    )
    code, text = run_gate(repo, report, base="main")
    assert code == 0, text
    assert "1 of 1 covered" in text


def test_a_deletion_adds_no_lines(tmp_path):
    repo = make_repo(tmp_path)
    git(repo, "checkout", "-q", "-b", "topic")
    git(repo, "rm", "-q", "app.py")
    git(repo, "commit", "-q", "-m", "delete")
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"other.py": {1: 1}}), encoding="utf-8")
    code, text = run_gate(repo, report, base="main")
    assert code == 0, text
    assert "result: not-applicable" in text


def test_report_paths_map_to_workspace_relative_files(tmp_path):
    files = {"pkg/mod.py": "a = 1\nb = 2\n"}
    for index, spelled in enumerate(
        ("./pkg/mod.py", "pkg\\mod.py", "/work/space/pkg/mod.py")
    ):
        sub = tmp_path / f"spelling{index}"
        sub.mkdir()
        _, (code, text) = feature(sub, lcov({spelled: {1: 1, 2: 0}}), files)
        assert "1 of 2 covered" in text, f"{spelled}: {text}"
        assert "uncovered: pkg/mod.py:2" in text, f"{spelled}: {text}"
        assert code == 1, text


def test_a_new_root_file_sharing_a_basename_is_incomplete(tmp_path):
    for index, spelled in enumerate(("pkg/util.py", "/work/space/pkg/util.py")):
        repo = make_repo(tmp_path, f"repo{index}")
        commit(repo, {"pkg/util.py": "a = 1\n"}, "pkg")
        git(repo, "checkout", "-q", "-b", "topic")
        commit(repo, {"util.py": "b = 2\n"})
        report = tmp_path / f"coverage{index}.lcov"
        report.write_text(lcov({spelled: {1: 1}}), encoding="utf-8")
        code, text = run_gate(repo, report, base="main", absent=["util.py"])
        assert code == 1, f"{spelled}: {text}"
        assert "uninstrumented: util.py" in text, f"{spelled}: {text}"
        assert "result: incomplete" in text, f"{spelled}: {text}"


STUB = """import json
import sys

target = sys.argv[sys.argv.index("--format") + 1].removeprefix("json:")
with open(target, "w", encoding="utf-8") as handle:
    json.dump({record!r}, handle)
"""


def test_diff_cover_output_missing_totals_never_passes(tmp_path):
    full: dict[str, object] = {
        "total_num_lines": 0,
        "total_num_violations": 0,
        "src_stats": {},
    }
    cases = {key: {k: v for k, v in full.items() if k != key} for key in full}
    cases["string_total"] = dict(full, total_num_lines="0")
    cases["more_violations"] = dict(full, total_num_violations=1)
    for name, record in cases.items():
        stub = tmp_path / name / "stub" / "diff_cover"
        stub.mkdir(parents=True)
        (stub / "__init__.py").write_text("", encoding="utf-8")
        (stub / "diff_cover_tool.py").write_text(
            STUB.format(record=record), encoding="utf-8"
        )
        repo = make_repo(tmp_path / name)
        git(repo, "checkout", "-q", "-b", "topic")
        commit(repo, {"app.py": APP + "line_6 = 6\n"})
        report = tmp_path / name / "coverage.lcov"
        report.write_text(lcov({"app.py": {6: 1}}), encoding="utf-8")
        code, text = run_gate(repo, report, base="main", pythonpath=(stub.parent,))
        assert code == 1, f"{name}: {text}"
        assert "lacks" in text, f"{name}: {text}"
        assert "result:" not in text, f"{name}: {text}"


def test_a_missing_base_fails_instead_of_comparing_something_else(tmp_path):
    repo = make_repo(tmp_path)
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"app.py": {1: 1}}), encoding="utf-8")
    code, text = run_gate(repo, report, base="origin/missing")
    assert code == 1, text
    assert "base revision 'origin/missing' is not available" in text
    assert "result:" not in text


def test_a_shallow_checkout_without_the_base_fails(tmp_path):
    origin = make_repo(tmp_path, "origin")
    git(origin, "checkout", "-q", "-b", "topic")
    commit(origin, {"app.py": APP + "line_6 = 6\n"}, "one")
    commit(origin, {"app.py": APP + "line_6 = 6\nline_7 = 7\n"}, "two")
    clone = tmp_path / "clone"
    subprocess.run(
        [
            "git",
            "clone",
            "-q",
            "--depth",
            "1",
            "--branch",
            "topic",
            origin.as_uri(),
            str(clone),
        ],
        check=True,
        capture_output=True,
    )
    base = git(origin, "rev-parse", "main")
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"app.py": {6: 1, 7: 1}}), encoding="utf-8")
    code, text = run_gate(clone, report, base=base)
    assert code == 1, text
    assert "is not available" in text
    assert "shallow" in text


def test_a_shallow_checkout_without_a_merge_base_fails(tmp_path):
    origin = make_repo(tmp_path, "origin")
    git(origin, "checkout", "-q", "-b", "topic")
    commit(origin, {"app.py": APP + "line_6 = 6\n"}, "one")
    commit(origin, {"app.py": APP + "line_6 = 6\nline_7 = 7\n"}, "two")
    clone = tmp_path / "clone"
    subprocess.run(
        [
            "git",
            "clone",
            "-q",
            "--depth",
            "1",
            "--branch",
            "topic",
            origin.as_uri(),
            str(clone),
        ],
        check=True,
        capture_output=True,
    )
    git(clone, "fetch", "-q", "--depth", "1", "origin", "main")
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"app.py": {6: 1, 7: 1}}), encoding="utf-8")
    code, text = run_gate(clone, report, base="FETCH_HEAD")
    assert code == 1, text
    assert "no merge-base" in text
    assert "shallow" in text


def test_malformed_reports_never_pass(tmp_path):
    files = {"app.py": APP + "line_6 = 6\n"}
    bad = {
        "garbage": "this is not lcov\n",
        "bad_line": "SF:app.py\nDA:six,1\nend_of_record\n",
        "unterminated": "SF:app.py\nDA:6,1\n",
        "no_record": "DA:6,1\nend_of_record\n",
        "nested": "SF:app.py\nSF:other.py\nend_of_record\n",
    }
    for name, text in bad.items():
        sub = tmp_path / name
        sub.mkdir()
        _, (code, output) = feature(sub, text, files)
        assert code == 1, f"{name}: {output}"
        assert "result:" not in output, f"{name}: {output}"


def test_a_missing_report_never_passes(tmp_path):
    repo = make_repo(tmp_path)
    code, text = run_gate(repo, tmp_path / "absent.lcov", base="main")
    assert code == 1, text
    assert "unreadable" in text


def test_a_changed_source_the_report_never_saw_is_incomplete(tmp_path):
    _, (code, text) = feature(
        tmp_path,
        lcov({"app.py": {1: 1, 2: 1, 3: 1, 4: 1, 5: 1}}),
        {"fresh.py": "x = 1\n", "README.md": "hello\nmore\n"},
        absent=["fresh.py"],
    )
    assert code == 1, text
    assert "complete: no" in text
    assert "uninstrumented: fresh.py" in text
    assert "uninstrumented: README.md" not in text
    assert "result: incomplete" in text


def test_a_covered_source_and_a_test_file_pass(tmp_path):
    _, (code, text) = feature(
        tmp_path,
        lcov({"app.py": {1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1}}),
        {"app.py": APP + "line_6 = 6\n", "tests/test_app.py": "x = 1\n"},
        owned={"app.py": "eligible", "tests/test_app.py": "support"},
    )
    assert code == 0, text
    assert "complete: yes" in text
    assert "result: pass" in text


def test_a_file_no_instrumented_target_owns_is_skipped(tmp_path):
    _, (code, text) = feature(
        tmp_path,
        lcov({"app.py": {1: 1, 2: 1, 3: 1, 4: 1, 5: 1}}),
        {"other.py": "x = 1\n"},
    )
    assert "uninstrumented:" not in text
    assert "complete: yes" in text


def test_a_missing_sources_record_never_passes(tmp_path):
    repo = make_repo(tmp_path)
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"app.py": {1: 1}}), encoding="utf-8")
    code, text = run_gate(repo, report, base="main", record=tmp_path / "none.json")
    assert code == 1, text
    assert "unreadable" in text
    assert "result:" not in text


def test_an_invalid_or_empty_sources_record_never_passes(tmp_path):
    repo = make_repo(tmp_path)
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"app.py": {1: 1}}), encoding="utf-8")
    broken = tmp_path / "broken.json"
    broken.write_text("{", encoding="utf-8")
    empty = sources(tmp_path / "empty.json", owned={})
    for record in (broken, empty):
        code, text = run_gate(repo, report, base="main", record=record)
        assert code == 1, text
        assert "result:" not in text


def test_an_empty_report_never_passes(tmp_path):
    _, (code, text) = feature(tmp_path, "", {"app.py": APP + "line_6 = 6\n"})
    assert code == 1, text
    assert "records no source files" in text
    assert "result:" not in text


def test_an_out_of_range_threshold_is_rejected(tmp_path):
    repo = make_repo(tmp_path)
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"app.py": {1: 1}}), encoding="utf-8")
    code, text = run_gate(repo, report, base="main", threshold=101)
    assert code == 1, text
    assert "not a whole percent from 0 to 100" in text


def test_the_json_record_states_the_result(tmp_path):
    repo = make_repo(tmp_path)
    git(repo, "checkout", "-q", "-b", "topic")
    commit(repo, {"app.py": APP + "line_6 = 6\n"})
    report = tmp_path / "coverage.lcov"
    report.write_text(lcov({"app.py": {6: 0}}), encoding="utf-8")
    out = tmp_path / "result.json"
    code, text = run_gate(repo, report, base="main", extra=("--report-json", str(out)))
    assert code == 1, text
    record = json.loads(out.read_text(encoding="utf-8"))
    assert record["result"] == "fail"
    assert (record["covered"], record["eligible"]) == (0, 1)
    assert record["uncovered"] == ["app.py:6"]
