package common

import (
	"os"
	"path/filepath"
	"testing"
)

func TestNpmHub(t *testing.T) {
	cases := []struct {
		name, module, lock, want string
		files                    map[string]string
	}{
		{"no module file", "", "pnpm-lock.yaml", "npm", nil},
		{"default", `npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(name = "npm", pnpm_lock = "//:pnpm-lock.yaml")
`, "pnpm-lock.yaml", "npm", nil},
		{"named", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
`, "pnpm-lock.yaml", "web", nil},
		{"alias", `npm.npm_translate_lock(name = "web", pnpm_lock = "@//:pnpm-lock.yaml")
use_repo(npm, site = "web")
`, "pnpm-lock.yaml", "site", nil},
		{"kwargs", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml", **HUB)
`, "pnpm-lock.yaml", "npm", nil},
		{"two locks first", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
npm.npm_translate_lock(name = "cli", pnpm_lock = "@@//cli:pnpm-lock.yaml")
`, "pnpm-lock.yaml", "web", nil},
		{"two locks second", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
npm.npm_translate_lock(name = "cli", pnpm_lock = "@@//cli:pnpm-lock.yaml")
`, "cli/pnpm-lock.yaml", "cli", nil},
		{"unlisted lock", `npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
`, "other/pnpm-lock.yaml", "npm", nil},
		{name: "include with alias", module: `include("//tools:js.MODULE.bazel")
`, lock: "web/pnpm-lock.yaml", want: "web", files: map[string]string{"tools/js.MODULE.bazel": `npm.npm_translate_lock(name = "web_npm", pnpm_lock = "//web:pnpm-lock.yaml")
use_repo(npm, web = "web_npm")
`}},
		{name: "include without alias", module: `include(label = "//tools:js.MODULE.bazel")
`, lock: "web/pnpm-lock.yaml", want: "web_npm", files: map[string]string{"tools/js.MODULE.bazel": `npm.npm_translate_lock(name = "web_npm", pnpm_lock = "//web:pnpm-lock.yaml")
`}},
		{name: "alias in include", module: `npm.npm_translate_lock(name = "web_npm", pnpm_lock = "//web:pnpm-lock.yaml")
include("//tools:js.MODULE.bazel")
`, lock: "web/pnpm-lock.yaml", want: "web", files: map[string]string{"tools/js.MODULE.bazel": `use_repo(npm, web = "web_npm")
`}},
		{name: "nested include", module: `include("//a.MODULE.bazel")
`, lock: "pnpm-lock.yaml", want: "deep", files: map[string]string{
			"a.MODULE.bazel": `include("//b.MODULE.bazel")
`,
			"b.MODULE.bazel": `npm.npm_translate_lock(name = "deep", pnpm_lock = "//:pnpm-lock.yaml")
`}},
		{name: "include cycle", module: `include("//a.MODULE.bazel")
`, lock: "pnpm-lock.yaml", want: "web", files: map[string]string{
			"a.MODULE.bazel": `include("//b.MODULE.bazel")
`,
			"b.MODULE.bazel": `include("//a.MODULE.bazel")
npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
`}},
		{name: "external include", module: `include("@x//:js.MODULE.bazel")
`, lock: "pnpm-lock.yaml", want: "npm"},
		{name: "missing include", module: `include("//gone.MODULE.bazel")
npm.npm_translate_lock(name = "web", pnpm_lock = "//:pnpm-lock.yaml")
`, lock: "pnpm-lock.yaml", want: "web"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			for name, body := range tc.files {
				file := filepath.Join(root, filepath.FromSlash(name))
				if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
					t.Fatal(err)
				}
				if err := os.WriteFile(file, []byte(body), 0o644); err != nil {
					t.Fatal(err)
				}
			}
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

func TestOwningPackageJSONAndHint(t *testing.T) {
	root := t.TempDir()
	for _, f := range []string{"package.json", "libs/greet/package.json"} {
		if err := os.MkdirAll(filepath.Dir(filepath.Join(root, f)), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(root, f), []byte("{}"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	for pkg, want := range map[string]string{
		"libs/greet/src": "libs/greet/package.json",
		"libs/greet":     "libs/greet/package.json",
		"apps/cli":       "package.json",
		"":               "package.json",
	} {
		if got := OwningPackageJSON(root, pkg); got != want {
			t.Errorf("OwningPackageJSON(%q) = %q, want %q", pkg, got, want)
		}
	}
	if got := OwningPackageJSON(t.TempDir(), "a"); got != "" {
		t.Errorf("OwningPackageJSON without a manifest = %q", got)
	}
	if got, want := NpmMissingHint(root, "libs/greet", "chalk/ansi"), `add "chalk" to libs/greet/package.json dependencies`; got != want {
		t.Errorf("hint = %q, want %q", got, want)
	}
}

func TestNpmPackageName(t *testing.T) {
	for spec, want := range map[string]string{
		"chalk":                "chalk",
		"chalk/ansi-styles":    "chalk",
		"@scope/name":          "@scope/name",
		"@scope/name/sub/path": "@scope/name",
	} {
		if got := NpmPackageName(spec); got != want {
			t.Errorf("NpmPackageName(%q) = %q, want %q", spec, got, want)
		}
	}
}
