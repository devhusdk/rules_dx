"""pytest plugin that turns the pytest-benchmark JSON into dx-bench.json."""

import json
import os
import pathlib

import pytest


def convert(benchmark_json):
    """Returns the dx-bench/v1 document for one pytest-benchmark document."""
    results = [
        {
            "name": entry["fullname"],
            "unit": "s",
            "mean": entry["stats"]["mean"],
            "stddev": entry["stats"]["stddev"],
            "iterations": entry["stats"]["rounds"],
        }
        for entry in benchmark_json["benchmarks"]
    ]
    return {"schema": "dx-bench/v1", "results": results}


@pytest.hookimpl(tryfirst=True)
def pytest_configure(config):
    """Points pytest-benchmark at benchmark.json in the undeclared outputs."""
    out_dir = os.environ.get("TEST_UNDECLARED_OUTPUTS_DIR")
    if out_dir and getattr(config.option, "benchmark_json", None) is None:
        config.option.benchmark_json = os.path.join(out_dir, "benchmark.json")


def pytest_unconfigure(config):
    """Writes dx-bench.json once pytest-benchmark has written its JSON file."""
    source = getattr(config.option, "benchmark_json", None)
    out_dir = os.environ.get("TEST_UNDECLARED_OUTPUTS_DIR")
    if source is None or not out_dir:
        return
    path = pathlib.Path(source)
    if not path.is_file() or not path.stat().st_size:
        return
    document = convert(json.loads(path.read_text(encoding="utf-8")))
    pathlib.Path(out_dir, "dx-bench.json").write_text(json.dumps(document), encoding="utf-8")
