"""CLI behavior in process and through the built binary."""

import hashlib
import json
import os
import re
import subprocess
import urllib.request

from click.testing import CliRunner
from ledger.cli.main import cli
from ledger.core import build_info

BINARY = os.path.abspath(os.environ["LEDGER_BIN"])


def run_binary(*args, cwd, env=None):
    merged = {**os.environ, **(env or {})}
    return subprocess.run(
        [BINARY, *args], cwd=cwd, env=merged, capture_output=True, text=True, timeout=60
    )


def test_list_in_process():
    result = CliRunner().invoke(cli, ["list"])
    assert result.exit_code == 0
    assert result.output.splitlines()[0] == "bolt-m6\tBolt M6\t45"


def test_quote_uses_config_file(tmp_path):
    config = tmp_path / "config.json"
    config.write_text(json.dumps({"currency": "SEK", "tax_percent": 50}))
    result = CliRunner().invoke(
        cli, ["--config", str(config), "quote", "nut-m6", "--qty", "3"]
    )
    assert result.exit_code == 0
    assert json.loads(result.output) == {
        "currency": "SEK",
        "net_cents": 60,
        "quantity": 3,
        "sku": "nut-m6",
        "tax_cents": 30,
        "total_cents": 90,
    }


def test_unknown_sku_and_bad_quantity_exit_nonzero():
    unknown = CliRunner().invoke(cli, ["quote", "ghost"])
    assert (unknown.exit_code, "unknown sku ghost" in unknown.output) == (1, True)
    bad = CliRunner().invoke(cli, ["quote", "nut-m6", "--qty", "0"])
    assert (bad.exit_code, "at least 1" in bad.output) == (2, True)


def test_missing_config_file_is_an_error(tmp_path):
    result = CliRunner().invoke(
        cli, ["--config", str(tmp_path / "absent.json"), "list"]
    )
    assert result.exit_code == 1
    assert "absent.json" in result.output


def test_binary_info_matches_generated_module(tmp_path):
    result = run_binary("info", cwd=tmp_path)
    assert result.returncode == 0, result.stderr
    assert result.stdout.splitlines() == [
        f"schema {build_info.SCHEMA_VERSION}",
        f"items {build_info.ITEM_COUNT}",
        f"sha256 {build_info.ITEMS_SHA256}",
    ]
    assert len(build_info.ITEMS_SHA256) == len(hashlib.sha256().hexdigest())


def test_binary_honours_config_from_environment(tmp_path):
    config = tmp_path / "config.json"
    config.write_text(json.dumps({"currency": "NOK", "tax_percent": 100}))
    result = run_binary(
        "quote",
        "washer-m6",
        "--qty",
        "5",
        cwd=tmp_path,
        env={"LEDGER_CONFIG": str(config)},
    )
    assert result.returncode == 0, result.stderr
    quote = json.loads(result.stdout)
    assert (quote["currency"], quote["total_cents"]) == ("NOK", 80)


def test_binary_serves_requests_then_exits(tmp_path):
    process = subprocess.Popen(
        [BINARY, "serve", "--max-requests", "2"],
        cwd=tmp_path,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        assert process.stdout is not None
        banner = process.stdout.readline()
        port = re.fullmatch(r"listening 127\.0\.0\.1:(\d+)\n", banner)
        assert port, banner
        base = f"http://127.0.0.1:{port.group(1)}"
        with urllib.request.urlopen(base + "/health") as response:
            assert json.load(response)["status"] == "ok"
        with urllib.request.urlopen(base + "/quote?sku=bolt-m6&qty=2") as response:
            assert json.load(response)["total_cents"] == 90
        assert process.wait(timeout=30) == 0
    finally:
        if process.poll() is None:
            process.kill()
        process.communicate()


def test_malformed_config_is_a_click_error(tmp_path):
    for body in ("[]", "null", '{"tax_percent": "5"}'):
        config = tmp_path / "config.json"
        config.write_text(body)
        result = CliRunner().invoke(cli, ["--config", str(config), "list"])
        assert result.exit_code == 1
        assert result.output.startswith("Error: ")
        assert "Traceback" not in result.output
