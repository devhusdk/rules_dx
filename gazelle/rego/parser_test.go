package rego

import (
	"slices"
	"testing"
)

func TestParsePackage(t *testing.T) {
	cases := []struct{ name, src, want string }{
		{"simple", "package greet\n", "greet"},
		{"nested", "# header\npackage authz.rules.v2\n\nallow := true\n", "authz.rules.v2"},
		{"commented out", "# package old\npackage current\n", "current"},
		{"indented", "  package spaced\n", "spaced"},
		{"none", "allow := true\n", ""},
	}
	for _, tc := range cases {
		if got := ParsePackage([]byte(tc.src)); got != tc.want {
			t.Errorf("%s: got %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestParseImports(t *testing.T) {
	src := `package app

import rego.v1
import future.keywords.if
import input.user
import data.authz.allow
import data.lib.strings as str
import data.lib.strings
# import data.commented.out
import data.zeta # trailing comment

x := "import data.in.string"
`
	got := ParseImports([]byte(src))
	want := []string{"authz.allow", "lib.strings", "zeta"}
	if !slices.Equal(got, want) {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestParseImportsNone(t *testing.T) {
	if got := ParseImports([]byte("package a\nimport rego.v1\n")); len(got) != 0 {
		t.Errorf("got %q, want none", got)
	}
}
