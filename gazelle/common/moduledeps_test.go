package common

import (
	"os"
	"path/filepath"
	"testing"
)

func TestModuleDepsIncludes(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "tools"), 0o755); err != nil {
		t.Fatal(err)
	}
	files := map[string]string{
		"MODULE.bazel":            "bazel_dep(name = \"root_dep\")\ninclude(\"//tools:deps.MODULE.bazel\")\n",
		"tools/deps.MODULE.bazel": "bazel_dep(name = \"included_dep\")\n",
	}
	for name, data := range files {
		if err := os.WriteFile(filepath.Join(root, filepath.FromSlash(name)), []byte(data), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	deps := ModuleDeps(root)
	for _, name := range []string{"root_dep", "included_dep"} {
		if !deps[name] {
			t.Errorf("ModuleDeps missing %q: %v", name, deps)
		}
	}
}
