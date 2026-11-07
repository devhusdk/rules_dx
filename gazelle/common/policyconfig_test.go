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
load("@rules_dx//quality/private:generated_policy.bzl", "dx_generated_policy")

ruff_config(
    name = "ruff",
    src = "//:ruff.toml",
    data = ["ruff/standard.toml"],
)

eslint_config(
    name = "eslint",
    src = "//:eslint.config.mjs",
)

dx_generated_policy(
    name = "policy",
    configs = [
        ":ruff",
        ":eslint",
    ],
    profile = "standard",
)
`
	if err := os.WriteFile(filepath.Join(dir, "BUILD.bazel"), []byte(build), 0o644); err != nil {
		t.Fatal(err)
	}
	got := InitOwnedConfigs(root)
	if len(got) != 2 || got["ruff"] != "//:ruff.toml" || got["eslint"] != "//:eslint.config.mjs" {
		t.Errorf("InitOwnedConfigs = %v", got)
	}
	if _, ok := got["policy"]; ok {
		t.Errorf("InitOwnedConfigs read the policy rule: %v", got)
	}
	if err := os.WriteFile(filepath.Join(dir, "BUILD.bazel"), []byte("ruff_config(\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if got := InitOwnedConfigs(root); got != nil {
		t.Errorf("unparsable file = %v, want nil", got)
	}
}

func TestInitOwnedConfigsPrefersGenerated(t *testing.T) {
	root := t.TempDir()
	write := func(rel, text string) {
		path := filepath.Join(root, rel)
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write(".dx/BUILD.bazel", "# gazelle:ignore\n\nruff_config(\n    name = \"ruff\",\n    src = \"//:ruff.toml\",\n)\n")
	if got := InitOwnedConfigs(root); len(got) != 1 || got["ruff"] != "//:ruff.toml" {
		t.Errorf("generated only = %v", got)
	}
	write(".dx/config/BUILD.bazel", "ruff_config(\n    name = \"ruff\",\n    src = \"//:.ruff.toml\",\n)\n\ntaplo_config(\n    name = \"taplo\",\n    src = \"//:taplo.toml\",\n)\n")
	got := InitOwnedConfigs(root)
	if len(got) != 2 || got["ruff"] != "//:ruff.toml" || got["taplo"] != "//:taplo.toml" {
		t.Errorf("both = %v, want .dx/BUILD.bazel to win", got)
	}
}
