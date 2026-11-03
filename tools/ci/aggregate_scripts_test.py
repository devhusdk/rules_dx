"""Behavioral fixtures for the evidence input validator and both aggregates."""

import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

AGGREGATE_FIXTURE_DIR = "tools/ci/aggregate_fixtures"
CI_GATE_FIXTURE_DIR = "tools/ci/ci_gate_fixtures"
WORKFLOW = ".github/workflows/reusable-consumer.yml"
CI_WORKFLOW = ".github/workflows/ci.yml"
EVIDENCE_WORKFLOW = ".github/workflows/evidence.yml"
AGGREGATE_HEREDOC = "python3 - <<'EOF'"
AGGREGATE_MARKER = "EOF"
CI_AGGREGATE_HEREDOC = "python3 - <<'DX_AGGREGATE'"
CI_AGGREGATE_MARKER = "DX_AGGREGATE"
EVIDENCE_HEREDOC = "python3 - <<'DX_EVIDENCE_INPUTS'"
EVIDENCE_MARKER = "DX_EVIDENCE_INPUTS"
INPUTS = (
    "DX_PLATFORMS",
    "DX_DISABLED_CHECKS",
    "DX_MIN_COVERAGE",
    "DX_MIN_CHANGED_LINE_COVERAGE",
    "DX_INVOCATION_ID",
)
GATE_JOBS = (
    "dogfood",
    "docs",
    "devcontainer-check",
    "hermetic",
    "network",
    "preflight",
    "workflow-check",
)
CONSUMER_JOBS = (
    "validate",
    "checks",
    "security-audit",
    "advisory-snapshots",
)
RESULTS = ("success", "failure", "skipped", "cancelled")


def embedded_script(workflow, opening, marker):
    """Returns one heredoc body from a workflow, dedented to its own file text."""
    lines = (ROOT / workflow).read_text(encoding="utf-8").splitlines()
    openings = [i for i, line in enumerate(lines) if line.strip() == opening]
    assert len(openings) == 1, (
        f"{workflow} runs {len(openings)} {marker} scripts, want 1"
    )
    base = len(lines[openings[0]]) - len(lines[openings[0]].lstrip(" "))
    body = []
    for line in lines[openings[0] + 1 :]:
        if line.strip() == marker:
            return "\n".join(body) + "\n"
        body.append(line[base:])
    raise AssertionError(f"{workflow} has no {marker} terminator")


def embedded_aggregate():
    """Returns the aggregate accounting the workflow runs, dedented to its own file."""
    return embedded_script(WORKFLOW, AGGREGATE_HEREDOC, AGGREGATE_MARKER)


def embedded_ci_aggregate():
    """Returns the outer gate accounting ci.yml runs, dedented to its own file."""
    return embedded_script(CI_WORKFLOW, CI_AGGREGATE_HEREDOC, CI_AGGREGATE_MARKER)


def fixture_names(directory):
    """Returns every fixture directory name, sorted."""
    return sorted(path.name for path in (ROOT / directory).iterdir() if path.is_dir())


def expectation(directory, case):
    """Returns the inputs, exit status, texts and outputs one fixture states."""
    lines = (ROOT / directory / case / "expect.txt").read_text(encoding="utf-8")
    inputs = {}
    status = None
    says = []
    outputs = {}
    for line in lines.splitlines():
        if line.startswith("input "):
            inputs = json.loads(line[len("input ") :])
        elif line.startswith("exit "):
            status = int(line[len("exit ") :])
        elif line.startswith("says "):
            says.append(line[len("says ") :])
        elif line.startswith("out "):
            key, _, value = line[len("out ") :].partition(" ")
            outputs[key] = json.loads(value)
    return inputs, status, says, outputs


def run_script(tmp_path, name, script, inputs):
    """Runs one workflow script under the fixture inputs and returns status and text."""
    path = tmp_path / name
    path.write_text(script, encoding="utf-8")
    env = dict(os.environ)
    for name in (*INPUTS, "CHECKS"):
        env.pop(name, None)
    for key, value in inputs.items():
        env[key] = value if isinstance(value, str) else json.dumps(value)
    return subprocess.run(
        [sys.executable, str(path)],
        capture_output=True,
        cwd=tmp_path,
        encoding="utf-8",
        env=env,
    )


def test_every_aggregate_fixture_states_one_expected_outcome():
    names = fixture_names(AGGREGATE_FIXTURE_DIR)
    assert len(names) > 0, f"{AGGREGATE_FIXTURE_DIR} holds no fixture"
    for case in names:
        inputs, status, says, outputs = expectation(AGGREGATE_FIXTURE_DIR, case)
        assert status in (0, 1), f"{case} declares no exit status"
        assert len(says) > 0, f"{case} expects no aggregate text"
        assert not outputs, f"{case} expects job outputs from the aggregate: {outputs}"
        needs = inputs["NEEDS"]
        assert "validate" in needs, f"{case} drops the validate job: {needs}"
        assert set(needs) <= set(CONSUMER_JOBS), f"{case} needs {sorted(needs)}"
        assert needs["validate"]["result"] in RESULTS, (
            f"{case} leaves the validate job undecided: {needs['validate']}"
        )
        assert all(
            needs[name]["result"] in RESULTS for name in needs if name != "validate"
        ), f"{case} holds an unknown job result: {needs}"


def test_aggregate_fixtures_report_the_expected_verdict(tmp_path):
    for case in fixture_names(AGGREGATE_FIXTURE_DIR):
        inputs, status, says, _ = expectation(AGGREGATE_FIXTURE_DIR, case)
        assert sorted(inputs) == ["CHECKS", "DISABLED", "NEEDS"], (
            f"{case} passes {sorted(inputs)}"
        )
        done = run_script(tmp_path, "aggregate.py", embedded_aggregate(), inputs)
        output = done.stdout + done.stderr
        assert done.returncode == status, (
            f"{case} exited {done.returncode}, expected {status}: {output}"
        )
        for text in says:
            assert text in output, f"{case} never reported [{text}]: {output}"


def test_every_ci_gate_fixture_states_one_expected_outcome():
    names = fixture_names(CI_GATE_FIXTURE_DIR)
    assert len(names) > 0, f"{CI_GATE_FIXTURE_DIR} holds no fixture"
    for case in names:
        inputs, status, says, outputs = expectation(CI_GATE_FIXTURE_DIR, case)
        assert status in (0, 1), f"{case} declares no exit status"
        assert len(says) > 0, f"{case} expects no gate text"
        assert not outputs, f"{case} expects job outputs from the gate: {outputs}"
        assert sorted(inputs) == ["NEEDS"], f"{case} passes {sorted(inputs)}"
        needs = inputs["NEEDS"]
        assert 0 < len(needs) <= len(GATE_JOBS), f"{case} needs {sorted(needs)}"
        assert set(needs) <= set(GATE_JOBS), f"{case} needs {sorted(needs)}"
        for name in needs:
            assert needs[name]["result"] in RESULTS, (
                f"{case} holds an unknown job result: {needs}"
            )


def test_ci_gate_fixtures_report_the_expected_verdict(tmp_path):
    for case in fixture_names(CI_GATE_FIXTURE_DIR):
        inputs, status, says, _ = expectation(CI_GATE_FIXTURE_DIR, case)
        script = embedded_ci_aggregate()
        done = run_script(tmp_path, "ci_aggregate.py", script, inputs)
        output = done.stdout + done.stderr
        assert done.returncode == status, (
            f"{case} exited {done.returncode}, expected {status}: {output}"
        )
        for text in says:
            assert text in output, f"{case} never reported [{text}]: {output}"


def payload_all_success(inputs):
    """Returns whether every needed job reports success."""
    return all(value["result"] == "success" for value in inputs["NEEDS"].values())


def payload_reports(result):
    """Returns a check that some needed job reports exactly this result."""
    return lambda inputs: any(
        value["result"] == result for value in inputs["NEEDS"].values()
    )


def payload_disabled_and_skipped(inputs):
    """Returns whether an explicitly disabled check stays skipped."""
    return bool(inputs.get("DISABLED")) and payload_reports("skipped")(inputs)


def payload_skipped_undisabled(inputs):
    """Returns whether an undisabled check was skipped."""
    return not inputs.get("DISABLED") and payload_reports("skipped")(inputs)


def payload_drops(jobs):
    """Returns a check that the payload omits one declared job."""
    return lambda inputs: set(jobs) - set(inputs["NEEDS"])


def payload_lists_checks(listed):
    """Returns a check that the validator wrote exactly this checks list."""
    return lambda inputs: inputs["CHECKS"] == listed


def test_the_result_truth_table_covers_every_aggregate_state(tmp_path):
    table = {
        AGGREGATE_FIXTURE_DIR: (
            "aggregate.py",
            embedded_aggregate(),
            (
                ("all_checks_green", 0, payload_all_success),
                ("failing_check_fails", 1, payload_reports("failure")),
                ("cancelled_check_fails", 1, payload_reports("cancelled")),
                ("disabled_check_stays_green", 0, payload_disabled_and_skipped),
                ("skipped_without_disabled_fails", 1, payload_skipped_undisabled),
                ("missing_check_fails", 1, payload_drops(CONSUMER_JOBS)),
                ("all_matrix_checks_disabled", 0, payload_lists_checks("[]")),
                ("validator_without_checks_output", 1, payload_lists_checks("")),
            ),
        ),
        CI_GATE_FIXTURE_DIR: (
            "ci_aggregate.py",
            embedded_ci_aggregate(),
            (
                ("all_green", 0, payload_all_success),
                ("failed_job_fails", 1, payload_reports("failure")),
                ("cancelled_job_fails", 1, payload_reports("cancelled")),
                ("skipped_job_fails", 1, payload_reports("skipped")),
                ("missing_job_fails", 1, payload_drops(GATE_JOBS)),
            ),
        ),
    }
    for directory, (name, script, rows) in table.items():
        for case, status, exhibits in rows:
            assert case in fixture_names(directory), f"{directory} lacks {case}"
            inputs, _, _, _ = expectation(directory, case)
            assert exhibits(inputs), f"{case} does not exhibit its named state"
            done = run_script(tmp_path, name, script, inputs)
            output = done.stdout + done.stderr
            assert done.returncode == status, (
                f"{case} exited {done.returncode}, expected {status}: {output}"
            )


def run_evidence(tmp_path, scope, platforms='["linux_x86_64"]', test_timeout="600"):
    """Runs the evidence input validator on one input set and returns status, text and outputs."""
    outputs = tmp_path / "evidence.outputs"
    done = run_script(
        tmp_path,
        "evidence_inputs.py",
        embedded_script(EVIDENCE_WORKFLOW, EVIDENCE_HEREDOC, EVIDENCE_MARKER),
        {
            "DX_SCOPE": scope,
            "DX_TASK": "test",
            "DX_PLATFORMS": platforms,
            "DX_DISPLAYS": "",
            "DX_TEST_TIMEOUT": test_timeout,
            "GITHUB_OUTPUT": str(outputs),
        },
    )
    written = outputs.read_text(encoding="utf-8") if outputs.exists() else None
    return done.returncode, done.stdout + done.stderr, written


def test_evidence_accepts_whitespace_separated_patterns(tmp_path):
    """Accepted scopes are written back as one line joined by single spaces."""
    cases = (
        ("//cli/cli/...", "//cli/cli/..."),
        (
            "//cli/process/... //tools/testing/process_probe/...",
            "//cli/process/... //tools/testing/process_probe/...",
        ),
        ("//a/...\t//b:t\n//c:*", "//a/... //b:t //c:*"),
    )
    for scope, expected in cases:
        status, output, written = run_evidence(tmp_path, scope)
        assert status == 0, f"{scope!r} exited {status}: {output}"
        assert written is not None, f"{scope!r} wrote no outputs"
        lines = written.splitlines()
        assert lines[0] == f"scope={expected}", f"{scope!r} wrote {lines[0]!r}"
        assert [line.partition("=")[0] for line in lines] == [
            "scope",
            "task",
            "platforms",
            "runners",
            "test_timeout",
        ], f"{scope!r} wrote extra keys: {lines}"
        (tmp_path / "evidence.outputs").unlink()


def test_evidence_rejects_bad_scopes(tmp_path):
    """Rejected scopes exit 1, name the bad pattern and write no outputs."""
    cases = (
        ("", "at least one target pattern"),
        (" \t\n", "at least one target pattern"),
        ("//a/... b/...", "'b/...'"),
        ("cli/...", "'cli/...'"),
        ("//$(id)", "'//$(id)'"),
        ("//a`id`", "'//a`id`'"),
        ("//a;id", "'//a;id'"),
        ("//a|b", "'//a|b'"),
        ('//"a"', "'//\"a\"'"),
        ("-//a", "'-//a'"),
        ("//a/... //$(id)", "'//$(id)'"),
    )
    for scope, says in cases:
        status, output, written = run_evidence(tmp_path, scope)
        assert status == 1, f"{scope!r} exited {status}: {output}"
        assert "evidence input rejected" in output, f"{scope!r}: {output}"
        assert says in output, f"{scope!r} does not say {says!r}: {output}"
        assert written is None, f"{scope!r} wrote outputs: {written!r}"


def test_evidence_passes_each_pattern_as_its_own_argument():
    """The run step splits the scope into an array instead of one quoted argument."""
    text = (ROOT / EVIDENCE_WORKFLOW).read_text(encoding="utf-8")
    assert 'read -ra patterns <<< "$DX_SCOPE"' in text
    assert '"${patterns[@]}"' in text
    assert '"$DX_SCOPE" 2>&1' not in text


def assert_rejected(case, status, output, written, says):
    """Asserts one rejected evidence input exited cleanly without writing outputs."""
    assert status == 1, f"{case!r} exited {status}: {output}"
    assert "evidence input rejected" in output, f"{case!r}: {output}"
    assert "Traceback" not in output, f"{case!r} raised: {output}"
    assert says in output, f"{case!r} does not say {says!r}: {output}"
    assert written is None, f"{case!r} wrote outputs: {written!r}"


def test_evidence_rejects_bad_platforms(tmp_path):
    """Bad platform lists exit 1 with a message instead of a traceback, writing nothing."""
    cases = (
        ('["linux_x86_64","linux_x86_64"]', "twice"),
        ("[1]", "JSON number"),
        ('[["linux_x86_64"]]', "JSON array"),
        ("{}", "JSON object"),
        ('"x"', "JSON string"),
        ("[]", "non-empty"),
        ('["plan9_x86_64"]', "unknown platforms"),
        ("[linux", "must be a JSON array"),
    )
    for platforms, says in cases:
        status, output, written = run_evidence(tmp_path, "//a/...", platforms=platforms)
        assert_rejected(platforms, status, output, written, says)


def test_evidence_rejects_bad_test_timeout(tmp_path):
    """A test_timeout that is not whole seconds exits 1 and writes nothing."""
    for test_timeout in ("", "1.5", "10s", "$(id)"):
        status, output, written = run_evidence(
            tmp_path, "//a/...", test_timeout=test_timeout
        )
        assert_rejected(test_timeout, status, output, written, "whole seconds")
