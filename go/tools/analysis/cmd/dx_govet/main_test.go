package main

import (
	"strings"
	"testing"

	"golang.org/x/tools/go/analysis"
)

func has(list []*analysis.Analyzer, name string) bool {
	for _, a := range list {
		if a.Name == name {
			return true
		}
	}
	return false
}

func TestDefaults(t *testing.T) {
	got, err := selectNamed(nil)
	if err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"printf", "nilness", "unusedwrite"} {
		if !has(got, name) {
			t.Errorf("%s should run by default", name)
		}
	}
	if has(got, "shadow") {
		t.Error("shadow must be off unless named")
	}
}

func TestAddAndRemove(t *testing.T) {
	got, err := selectNamed([]string{"shadow", "-nilness"})
	if err != nil {
		t.Fatal(err)
	}
	if !has(got, "shadow") || has(got, "nilness") || !has(got, "printf") {
		t.Errorf("unexpected set %v", got)
	}
}

func TestUnknownName(t *testing.T) {
	for _, name := range []string{"nosuch", "-nosuch", "fieldalignment"} {
		_, err := selectNamed([]string{name})
		if err == nil || !strings.Contains(err.Error(), "valid names:") || !strings.Contains(err.Error(), "shadow") {
			t.Errorf("%s: err = %v", name, err)
		}
	}
}

func TestConfigureFlag(t *testing.T) {
	if err := configure([]string{"-analyzers=shadow"}); err != nil {
		t.Fatal(err)
	}
	if !has(active, "shadow") {
		t.Error("shadow not active")
	}
	if err := configure([]string{"-analyzers=nosuch"}); err == nil {
		t.Error("want an error")
	}
	if err := configure(nil); err != nil || has(active, "shadow") {
		t.Errorf("configure(nil) must restore defaults, err = %v", err)
	}
}
