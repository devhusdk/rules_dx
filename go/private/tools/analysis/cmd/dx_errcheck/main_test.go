package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/kisielk/errcheck/errcheck"
)

func flagValue(name string) string {
	return errcheck.Analyzer.Flags.Lookup(name).Value.String()
}

func TestStandardLeavesBlankAndAssertOff(t *testing.T) {
	if err := configure([]string{"-profile=standard"}); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"blank", "assert"} {
		if got := flagValue(name); got != "false" {
			t.Errorf("%s = %s", name, got)
		}
	}
}

func TestStrictTurnsBlankAndAssertOn(t *testing.T) {
	if err := configure([]string{"-profile=strict"}); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"blank", "assert"} {
		if got := flagValue(name); got != "true" {
			t.Errorf("%s = %s", name, got)
		}
	}
}

func TestExcludeFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "exclude.txt")
	if err := os.WriteFile(path, []byte("os.Remove\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := configure([]string{"-profile=standard", "-exclude=" + path}); err != nil {
		t.Fatal(err)
	}
	if got := flagValue("exclude"); got != path {
		t.Errorf("exclude = %s", got)
	}
}

func TestUnknownProfile(t *testing.T) {
	err := configure([]string{"-profile=loose"})
	if err == nil || !strings.Contains(err.Error(), "standard or strict") {
		t.Errorf("err = %v", err)
	}
}
