package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"rules_dx/go/private/tools/analysis/unit"
	"strings"
	"testing"
)

func TestMain(m *testing.M) {
	if os.Getenv(unit.ChildEnv) != "" {
		os.Exit(run(os.Args, os.Stdout, os.Stderr))
	}
	os.Exit(m.Run())
}

func TestToolName(t *testing.T) {
	for argv0, want := range map[string]string{
		"/bin/dx_govet":                   "dx_govet",
		"out/dx_staticcheck.exe":          "dx_staticcheck",
		"dx-modfmt":                       "dx_modfmt",
		"bazel-out/k8-opt-exec/bin/dx_go": "dx_go",
	} {
		if got := toolName(argv0); got != want {
			t.Errorf("toolName(%q) = %q, want %q", argv0, got, want)
		}
	}
}

func TestDispatchesOnArgv0(t *testing.T) {
	path := filepath.Join(t.TempDir(), "go.mod")
	if err := os.WriteFile(path, []byte("module   example.com/a\ngo 1.22\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	var out, errOut bytes.Buffer
	if code := run([]string{"/x/dx_modfmt", "-c", "-l", path}, &out, &errOut); code != 1 {
		t.Fatalf("code = %d, stderr = %s", code, errOut.String())
	}
	if !strings.Contains(out.String(), path) {
		t.Errorf("stdout = %q, want the unformatted file listed", out.String())
	}
}

func TestDispatchesOnSubcommand(t *testing.T) {
	path := filepath.Join(t.TempDir(), "go.mod")
	if err := os.WriteFile(path, []byte("module example.com/a\n\ngo 1.22\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	var out, errOut bytes.Buffer
	if code := run([]string{"/x/dx_go", "dx_modfmt", "-c", "-l", path}, &out, &errOut); code != 0 {
		t.Fatalf("code = %d, stderr = %s", code, errOut.String())
	}
}

func TestSubcommandReachesTheToolInItsChild(t *testing.T) {
	dir := t.TempDir()
	src := filepath.Join(dir, "a.go")
	if err := os.WriteFile(src, []byte("package p\n\nfunc F() {}\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	raw, err := json.Marshal(unit.Config{ImportPath: "example.com/p", GoFiles: []string{src}, GOOS: "linux", GOARCH: "amd64"})
	if err != nil {
		t.Fatal(err)
	}
	cfg := filepath.Join(dir, "cfg.json")
	if err := os.WriteFile(cfg, raw, 0o644); err != nil {
		t.Fatal(err)
	}
	args := os.Args
	t.Cleanup(func() { os.Args = args })
	var out, errOut bytes.Buffer
	if code := run([]string{"/x/dx_go", "dx_govet", cfg}, &out, &errOut); code != 0 {
		t.Fatalf("code = %d, stderr = %s", code, errOut.String())
	}
	if strings.Contains(out.String(), `"error"`) {
		t.Fatalf("child did not run dx_govet: %s", out.String())
	}
}

func TestUnknownToolListsTheTools(t *testing.T) {
	var out, errOut bytes.Buffer
	if code := run([]string{"/x/dx_go"}, &out, &errOut); code != 2 {
		t.Fatalf("code = %d", code)
	}
	for name := range tools {
		if !strings.Contains(errOut.String(), name) {
			t.Errorf("stderr %q does not list %s", errOut.String(), name)
		}
	}
}
