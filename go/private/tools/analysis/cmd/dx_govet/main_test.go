package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"golang.org/x/tools/go/analysis"
	"rules_dx/go/private/tools/analysis/unit"
)

func has(list []*analysis.Analyzer, name string) bool {
	for _, a := range list {
		if a.Name == name {
			return true
		}
	}
	return false
}

func TestStandardRunsTheDefaults(t *testing.T) {
	if err := configure([]string{"-profile=standard"}); err != nil {
		t.Fatal(err)
	}
	if !has(active, "printf") {
		t.Error("printf should run")
	}
	for _, name := range []string{"shadow", "nilness", "unusedwrite"} {
		if has(active, name) {
			t.Errorf("%s must be off under standard", name)
		}
	}
}

func TestStrictAddsShadowNilnessUnusedwrite(t *testing.T) {
	if err := configure([]string{"-profile=strict"}); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"printf", "shadow", "nilness", "unusedwrite"} {
		if !has(active, name) {
			t.Errorf("%s should run under strict", name)
		}
	}
	if has(active, "fieldalignment") {
		t.Error("fieldalignment stays off")
	}
}

func TestUnknownProfile(t *testing.T) {
	err := configure([]string{"-profile=loose"})
	if err == nil || !strings.Contains(err.Error(), "standard or strict") {
		t.Errorf("err = %v", err)
	}
}

func writeConfig(t *testing.T, body string) string {
	path := filepath.Join(t.TempDir(), "govet.json")
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestConfigExcludesFindingsPerAnalyzer(t *testing.T) {
	path := writeConfig(t, `{"shadow": {"exclude_files": {"gen_.*\\.go$": "generated"}}}`)
	if err := configure([]string{"-profile=strict", "-config=" + path}); err != nil {
		t.Fatal(err)
	}
	finding := func(file string) json.RawMessage {
		return json.RawMessage(`[{"posn":"` + file + `:3:4","message":"declaration of x"}]`)
	}
	tree := unit.Tree{"p": {
		"shadow":   finding("pkg/gen_a.go"),
		"nilness":  finding("pkg/gen_a.go"),
		"shadow2":  finding("pkg/gen_a.go"),
		"copylock": finding("pkg/b.go"),
	}}
	got := filter(nil, tree)["p"]
	if _, ok := got["shadow"]; ok {
		t.Error("shadow finding in gen_a.go should be dropped")
	}
	for _, name := range []string{"nilness", "shadow2", "copylock"} {
		if _, ok := got[name]; !ok {
			t.Errorf("%s finding was dropped", name)
		}
	}
}

func TestShadowKeptOutsideExcludedFiles(t *testing.T) {
	path := writeConfig(t, `{"shadow": {"exclude_files": {"gen_.*": "generated"}}}`)
	if err := configure([]string{"-profile=strict", "-config=" + path}); err != nil {
		t.Fatal(err)
	}
	tree := unit.Tree{"p": {"shadow": json.RawMessage(`[{"posn":"pkg/b.go:3:4","message":"m"}]`)}}
	if _, ok := filter(nil, tree)["p"]["shadow"]; !ok {
		t.Error("shadow finding in b.go must stay")
	}
}

func TestConfigRejectsUnknownAnalyzer(t *testing.T) {
	path := writeConfig(t, `{"nosuch": {}}`)
	err := configure([]string{"-profile=strict", "-config=" + path})
	if err == nil || !strings.Contains(err.Error(), "valid names:") {
		t.Errorf("err = %v", err)
	}
}
