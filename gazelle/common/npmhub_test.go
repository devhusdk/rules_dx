package common

import (
	"os"
	"path/filepath"
	"testing"
)

func TestNpmHub(t *testing.T) {
	cases := []struct {
		name, module, lock, want string
	}{
		{"no module file", "", "pnpm-lock.yaml", "npm"},
		{"default", `npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(name = "npm", pnpm_lock = "//:pnpm-lock.yaml")
`, "pnpm-lock.yaml", "npm"},
		{"named", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
`, "pnpm-lock.yaml", "web"},
		{"alias", `npm.npm_translate_lock(name = "web", pnpm_lock = "@//:pnpm-lock.yaml")
use_repo(npm, site = "web")
`, "pnpm-lock.yaml", "site"},
		{"kwargs", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml", **HUB)
`, "pnpm-lock.yaml", "npm"},
		{"two locks first", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
npm.npm_translate_lock(name = "cli", pnpm_lock = "@@//cli:pnpm-lock.yaml")
`, "pnpm-lock.yaml", "web"},
		{"two locks second", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
npm.npm_translate_lock(name = "cli", pnpm_lock = "@@//cli:pnpm-lock.yaml")
`, "cli/pnpm-lock.yaml", "cli"},
		{"unlisted lock", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
`, "other/pnpm-lock.yaml", "npm"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			if tc.module != "" {
				if err := os.WriteFile(filepath.Join(root, "MODULE.bazel"), []byte(tc.module), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			if got := NpmHub(root, tc.lock); got != tc.want {
				t.Errorf("NpmHub = %q, want %q", got, tc.want)
			}
		})
	}
}
