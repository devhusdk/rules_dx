package common

import (
	"os"
	"path/filepath"
	"testing"
)

func TestInitOwnedConfigs(t *testing.T) {
	root := t.TempDir()
	if got := InitOwnedConfigs(root); got != nil {
		t.Errorf("missing file = %v, want nil", got)
	}
	dir := filepath.Join(root, ".dx", "config")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	build := `load("@rules_dx//quality:native_config.bzl", "eslint_config", "ruff_config")
load("@rules_dx//quality:policy.bzl", "workspace_policy")

ruff_config(
    name = "ruff",
    src = "//:ruff.toml",
    data = ["ruff/standard.toml"],
)

eslint_config(
    name = "eslint",
    src = "//:eslint.config.mjs",
)

workspace_policy(
    name = "policy",
    configs = [":ruff", ":eslint"],
)
`
	if err := os.WriteFile(filepath.Join(dir, "BUILD.bazel"), []byte(build), 0o644); err != nil {
		t.Fatal(err)
	}
	got := InitOwnedConfigs(root)
	if len(got) != 2 || got["ruff"] != "//:ruff.toml" || got["eslint"] != "//:eslint.config.mjs" {
		t.Errorf("InitOwnedConfigs = %v", got)
	}
	if err := os.WriteFile(filepath.Join(dir, "BUILD.bazel"), []byte("ruff_config(\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if got := InitOwnedConfigs(root); got != nil {
		t.Errorf("unparsable file = %v, want nil", got)
	}
}
