package main

import (
	"strings"
	"testing"

	"github.com/kisielk/errcheck/errcheck"
)

func TestBlankFlag(t *testing.T) {
	flag := errcheck.Analyzer.Flags.Lookup("blank")
	if flag.Value.String() != "false" {
		t.Fatal("blank must start off")
	}
	if err := configure([]string{"-flags=blank,assert"}); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"blank", "assert"} {
		if got := errcheck.Analyzer.Flags.Lookup(name).Value.String(); got != "true" {
			t.Errorf("%s = %s", name, got)
		}
	}
}

func TestUnknownFlag(t *testing.T) {
	err := configure([]string{"-flags=asserts"})
	if err == nil || !strings.Contains(err.Error(), "valid names: assert, blank") {
		t.Errorf("err = %v", err)
	}
}
