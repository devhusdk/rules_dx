"""Tests for the first-build harness: statistics, merge, check, render and fake runs."""

import json
import os
import shutil
import stat
import sys

import first_build
import pytest
from workflow_testing import ROOT

FIXTURES = ROOT / "tools" / "ci" / "first_build_fixtures"

pytestmark = pytest.mark.skipif(
    os.name == "nt", reason="fake dx and bazel are shell scripts"
)


def doc(*totals):
    rows = []
    for index, total in enumerate(totals):
        rows.append(
            dict(
                name=f"lang{index}",
                init_s=None if total is None else 10,
                test_s=None if total is None else total - 10,
                total_s=total,
                download_mb=None,
                repos=None,
                silent_max_s=None,
                date=None,
                dx_version=None,
                dx_source=None,
                ceiling_s=None,
                skipped=None,
            )
        )
    return {
        "schema": 2,
        "runner": "Linux x86_64",
        "bazel_version": "9.2.0",
        "excludes": "x.",
        "method": {"runs": 3, "statistic": "median"},
        "languages": rows,
    }


def test_median_ignores_nulls():
    assert first_build.median([3, 1, 2]) == 2
    assert first_build.median([None, 4, 2]) == 3
    assert first_build.median([None]) is None


def test_ceiling_rounds_up_to_the_minute():
    assert first_build.ceiling_for(100) == 180
    assert first_build.ceiling_for(120) == 180
    assert first_build.ceiling_for(40) == 60


def part(name, total, **extra):
    return dict(
        name=name,
        init_s=5,
        test_s=total - 5,
        total_s=total,
        download_mb=1,
        repos=2,
        silent_max_s=3,
        dx_version="v",
        dx_source="checkout",
        date="2026-01-01",
        bazel_version="9.2.0",
        **extra,
    )


def test_merge_sets_ceiling_only_when_null_and_never_raises():
    old = doc(None, None)
    old["languages"][1]["ceiling_s"] = 100
    merged = first_build.merge(old, [part("lang0", 100), part("lang1", 500)], "d")
    assert merged["languages"][0]["ceiling_s"] == 180
    assert merged["languages"][1]["ceiling_s"] == 100
    assert merged["languages"][1]["total_s"] == 500


def test_merge_rejects_unknown_language():
    with pytest.raises(ValueError, match="unknown language nope"):
        first_build.merge(doc(None), [part("nope", 1)], "d")


def test_select_rejects_unknown_language():
    with pytest.raises(ValueError, match="unknown language nope"):
        first_build.select(doc(None), ["nope"])
    assert first_build.select(doc(None, None), []) == ["lang0", "lang1"]
    assert first_build.select(doc(None, None), ["lang1"]) == ["lang1"]


def test_select_leaves_out_skipped_and_rejects_naming_one():
    data = doc(None, None)
    data["languages"][0]["skipped"] = "no fixture"
    assert first_build.select(data, []) == ["lang1"]
    with pytest.raises(ValueError, match="lang0 is skipped: no fixture"):
        first_build.select(data, ["lang0"])
    data["languages"][1]["skipped"] = "x"
    with pytest.raises(ValueError, match="no languages"):
        first_build.select(data, [])


def test_merge_keeps_a_fixed_skipped():
    old = doc(None)
    old["languages"][0]["skipped"] = "no fixture"
    merged = first_build.merge(old, [part("lang0", 100)], "d")
    assert merged["languages"][0]["skipped"] == "no fixture"


def test_json_rows_match_fixture_directories():
    data = first_build.load(ROOT / "docs" / "first-build.json")
    base = ROOT / first_build.FIXTURES_REL
    dirs = {p.name for p in base.iterdir() if p.is_dir()}
    assert dirs <= set(first_build.names(data))
    for row in data["languages"]:
        path = base / row["name"]
        has = path.is_dir() and any(path.rglob("*.fixture"))
        assert has != bool(row.get("skipped")), row["name"]


def test_check_names_the_breach():
    data = doc(300, 100)
    data["languages"][0]["ceiling_s"] = 200
    over, unset = first_build.breaches(data)
    assert over == [
        "first-build: lang0 total 300s over ceiling 200s (init 10s, test 290s)"
    ]
    assert unset == ["lang1"]


def test_render_matches_checked_in_page():
    data = first_build.load(ROOT / "docs" / "first-build.json")
    page = (ROOT / "docs" / "first-build.md").read_text(encoding="utf-8")
    assert first_build.render(data) == page


def test_render_shows_pending_and_skipped():
    data = doc(None, 100)
    data["languages"][1]["skipped"] = "no GPU"
    page = first_build.render(data)
    assert "| `lang0` | pending |" in page
    assert "| `lang1` | skipped |" in page
    assert "- `lang1`: no GPU" in page


def fake_tools(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for name in ("dx", "bazel"):
        target = bin_dir / name
        shutil.copy(FIXTURES / name, target)
        target.chmod(target.stat().st_mode | stat.S_IEXEC)
    rules = tmp_path / "rules"
    (rules / "tools" / "bazelrc").mkdir(parents=True)
    for layer in first_build.PRESET_LAYERS:
        (rules / "tools" / "bazelrc" / f"{layer}.bazelrc").write_text("build --x\n")
    fixture = rules / first_build.FIXTURES_REL / "lang0"
    (fixture / "sub").mkdir(parents=True)
    for name in ("BUILD.bazel", "MODULE.bazel", ".bazelrc", "sub/x.txt"):
        (fixture / (name + ".fixture")).write_text(f"fixture {name}\n")
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    return bin_dir, rules, scratch


def run_fake(tmp_path, monkeypatch, tty=False, **env):
    bin_dir, rules, scratch = fake_tools(tmp_path)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("BAZELISK_HOME", str(tmp_path / "bh"))
    for key, value in env.items():
        monkeypatch.setenv(key, value)
    result = first_build.measure_run(
        "lang0", str(bin_dir / "dx"), rules, tty, "checkout", str(scratch)
    )
    return result, scratch


def test_measure_run_records_gap_notice_and_cleans_up(tmp_path, monkeypatch):
    run, scratch = run_fake(tmp_path, monkeypatch, FAKE_DX_SLEEP="1")
    assert run["notice_seen"]
    assert 0.9 <= run["silent_max_s"] < 3
    assert run["silent_after"] == "Analyzing"
    assert run["bazel_version"] == "bazel 9.2.0"
    assert run["dx_version"] == "dx 0.0.0-fake"
    assert run["total_s"] >= 1
    assert list(scratch.iterdir()) == []


def test_read_only_dirs_under_home_are_removed(tmp_path, monkeypatch):
    run, scratch = run_fake(tmp_path, monkeypatch, FAKE_DX_SLEEP="0", FAKE_DX_RO="1")
    assert run["notice_seen"]
    assert list(scratch.iterdir()) == []


def test_download_mb_counts_content_addressable_only(tmp_path, monkeypatch):
    run, _ = run_fake(tmp_path, monkeypatch, FAKE_DX_SLEEP="0")
    assert run["download_mb"] == pytest.approx(0.001)


def test_measure_run_in_a_pty(tmp_path, monkeypatch):
    run, _ = run_fake(tmp_path, monkeypatch, tty=True, FAKE_DX_SLEEP="1")
    assert run["notice_seen"]
    assert run["silent_max_s"] >= 0.9


def test_failing_dx_test_fails_the_run(tmp_path, monkeypatch):
    with pytest.raises(first_build.MeasureError, match="dx test failed"):
        run_fake(tmp_path, monkeypatch, FAKE_DX_TEST_EXIT="1", FAKE_DX_SLEEP="0")


def test_missing_notice_means_not_cold(tmp_path, monkeypatch):
    with pytest.raises(first_build.MeasureError, match="not cold"):
        run_fake(tmp_path, monkeypatch, FAKE_DX_COLD="0", FAKE_DX_SLEEP="0")


def test_measure_run_calls_init_never_new(tmp_path, monkeypatch):
    log = tmp_path / "dx.log"
    run_fake(tmp_path, monkeypatch, FAKE_DX_SLEEP="0", FAKE_DX_LOG=str(log))
    lines = log.read_text().splitlines()
    assert len(lines) == 1
    assert lines[0].startswith("init --apply demo-app --workspace ")
    assert "new" not in lines[0].split()


def test_missing_fixture_dir_is_an_error(tmp_path, monkeypatch):
    bin_dir, rules, scratch = fake_tools(tmp_path)
    shutil.rmtree(rules / first_build.FIXTURES_REL)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    with pytest.raises(first_build.MeasureError, match="no fixture at"):
        first_build.measure_run(
            "lang0", str(bin_dir / "dx"), rules, False, "checkout", str(scratch)
        )


def test_stage_fixture_strips_suffix_and_appends(tmp_path):
    src = tmp_path / "src"
    (src / "a" / "b").mkdir(parents=True)
    (src / "a" / "b" / "f.txt.fixture").write_text("deep")
    (src / "BUILD.bazel.fixture").write_text("build")
    (src / "MODULE.bazel.fixture").write_text("frag\n")
    (src / ".bazelrc.fixture").write_text("rc\n")
    dest = tmp_path / "dest"
    dest.mkdir()
    (dest / "MODULE.bazel").write_text("init")
    (dest / "BUILD.bazel").write_text("old")
    first_build.stage_fixture(src, dest)
    assert (dest / "a" / "b" / "f.txt").read_text() == "deep"
    assert (dest / "BUILD.bazel").read_text() == "build"
    assert (dest / "MODULE.bazel").read_text() == "init\n\nfrag\n"
    assert (dest / ".bazelrc").read_text() == "\nrc\n"


def test_stage_fixture_rejects_unsuffixed_file(tmp_path):
    src = tmp_path / "src"
    src.mkdir()
    (src / "BUILD.bazel").write_text("x")
    with pytest.raises(first_build.MeasureError, match="lacks the .fixture suffix"):
        first_build.stage_fixture(src, tmp_path)


def test_override_and_presets_follow_the_fixture_fragments(tmp_path):
    src = tmp_path / "src"
    src.mkdir()
    (src / "MODULE.bazel.fixture").write_text("frag\n")
    (src / ".bazelrc.fixture").write_text("fixturerc\n")
    rules = tmp_path / "rules"
    (rules / "tools" / "bazelrc").mkdir(parents=True)
    for layer in first_build.PRESET_LAYERS:
        (rules / "tools" / "bazelrc" / f"{layer}.bazelrc").write_text("build --x\n")
    demo = tmp_path / "demo"
    demo.mkdir()
    (demo / "MODULE.bazel").write_text("init\n")
    first_build.stage_fixture(src, demo)
    first_build.point_at_checkout(demo, rules)
    module = (demo / "MODULE.bazel").read_text()
    assert module.index("frag") < module.index("local_path_override")
    rc = (demo / ".bazelrc").read_text()
    assert rc.index("fixturerc") < rc.index("build --x")


def test_launcher_source_is_refused(tmp_path):
    with pytest.raises(first_build.MeasureError, match="not implemented"):
        first_build.measure_run(
            "lang0", "dx", tmp_path, False, "launcher", str(tmp_path)
        )


def test_summarize_takes_medians():
    runs = [
        dict(
            init_s=1,
            test_s=9,
            total_s=10,
            download_mb=5,
            repos=3,
            silent_max_s=2,
            silent_after="a",
            notice_seen=True,
            bazel_version="b",
            dx_version="v",
        ),
        dict(
            init_s=1,
            test_s=29,
            total_s=30,
            download_mb=5,
            repos=3,
            silent_max_s=9,
            silent_after="z",
            notice_seen=True,
            bazel_version="b",
            dx_version="v",
        ),
        dict(
            init_s=1,
            test_s=19,
            total_s=20,
            download_mb=5,
            repos=3,
            silent_max_s=4,
            silent_after="c",
            notice_seen=True,
            bazel_version="b",
            dx_version="v",
        ),
    ]
    summary = first_build.summarize("lang0", runs, "checkout", "d")
    assert summary["total_s"] == 20
    assert summary["silent_max_s"] == 4
    assert summary["silent_after"] == "z"


def test_matrix_prints_json(capsys):
    assert first_build.main(["matrix", "--languages", "c,zig"]) == 0
    assert json.loads(capsys.readouterr().out) == {"language": ["c", "zig"]}
    assert first_build.main(["matrix", "--languages", "nope"]) == 1
    assert first_build.main(["matrix", "--languages", "rust"]) == 1
    assert first_build.main(["matrix"]) == 0
    languages = json.loads(capsys.readouterr().out)["language"]
    assert "rust" not in languages and "c" in languages


if __name__ == "__main__":
    sys.exit(pytest.main([__file__]))
