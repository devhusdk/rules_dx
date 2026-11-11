package rego

import (
	"slices"
	"testing"
)

func TestIsTestSource(t *testing.T) {
	for name, want := range map[string]bool{
		"greet_test.rego": true,
		"greet.rego":      false,
		"test.rego":       false,
		"greet_test.go":   false,
	} {
		if got := IsTestSource(name); got != want {
			t.Errorf("IsTestSource(%q) = %v, want %v", name, got, want)
		}
	}
}

func TestCandidates(t *testing.T) {
	cases := map[string][]string{
		"authz.rules.allow": {"authz.rules", "authz"},
		"authz.allow":       {"authz"},
		"authz":             nil,
	}
	for name, want := range cases {
		if got := Candidates(name); !slices.Equal(got, want) {
			t.Errorf("Candidates(%q) = %q, want %q", name, got, want)
		}
	}
}
