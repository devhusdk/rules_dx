package common

import (
	"testing"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

type extendedIgnoreConfig struct {
	IgnoreConfig
	extra string
}

func TestInheritedIgnoresReadsAnyIgnoreSource(t *testing.T) {
	entries := []*IgnoreEntry{{Value: "Widget", Path: "pkg"}}
	cases := []struct {
		name string
		ext  any
		want int
	}{
		{"absent", nil, 0},
		{"foreign type", "not a config", 0},
		{"nil config", (*IgnoreConfig)(nil), 0},
		{"nil source", (*extendedIgnoreConfig)(nil), 0},
		{"empty config", &IgnoreConfig{}, 0},
		{"plain config", &IgnoreConfig{Ignores: entries}, 1},
		{"extended config", &extendedIgnoreConfig{IgnoreConfig: IgnoreConfig{Ignores: entries}}, 1},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			c := config.New()
			if tc.ext != nil {
				c.Exts["lang"] = tc.ext
			}
			if got := len(InheritedIgnores(c, "lang")); got != tc.want {
				t.Fatalf("entries = %d, want %d", got, tc.want)
			}
		})
	}
	if got := InheritedIgnores(nil, "lang"); got != nil {
		t.Fatalf("nil config entries = %+v", got)
	}
}

func TestMergeIgnoresLeavesTheParentSliceAlone(t *testing.T) {
	if got := MergeIgnores(nil, nil); got != nil {
		t.Fatalf("empty merge = %+v, want nil", got)
	}
	parent := make([]*IgnoreEntry, 1, 4)
	parent[0] = &IgnoreEntry{Value: "Old", Path: "pkg"}
	merged := MergeIgnores(parent, []*IgnoreEntry{{Value: "New", Path: "pkg/sub"}})
	if len(merged) != 2 || merged[0].Value != "Old" || merged[1].Value != "New" {
		t.Fatalf("merged = %+v", merged)
	}
	if len(parent) != 1 {
		t.Fatalf("parent slice grew to %+v", parent)
	}
	merged[0] = &IgnoreEntry{Value: "Replaced"}
	if parent[0].Value != "Old" {
		t.Fatalf("parent slice shares its backing array: %+v", parent[0])
	}
}

func TestDeclaredIgnoresNeedsAFile(t *testing.T) {
	declared, errs := DeclaredIgnores("lang", "pkg", nil)
	if declared != nil || errs != nil {
		t.Fatalf("nil file = %+v, %v", declared, errs)
	}
	declared, errs = DeclaredIgnores("lang", "pkg", &rule.File{})
	if declared != nil || errs != nil {
		t.Fatalf("empty file = %+v, %v", declared, errs)
	}
}

func TestMatchingIgnoreSkipsNilEntries(t *testing.T) {
	c := config.New()
	c.Exts["lang"] = &IgnoreConfig{Ignores: []*IgnoreEntry{
		{Value: "Widget", Path: "pkg"},
		nil,
		{Value: "Gadget", Path: "pkg/sub"},
	}}
	if got := MatchingIgnore(c, "lang", "Widget"); got == nil || got.Path != "pkg" {
		t.Fatalf("nearest match = %+v", got)
	}
	if got := MatchingIgnore(c, "lang", "Missing"); got != nil {
		t.Fatalf("unmatched import = %+v", got)
	}
}

func TestStaleIgnoresReportsUnusedEntriesOnly(t *testing.T) {
	got := StaleIgnores("lang", []*IgnoreEntry{
		{Value: "Used", Path: "pkg", Used: true},
		nil,
		{Value: "Stale", Path: "pkg"},
	})
	want := []string{"lang: //pkg: stale # gazelle:dx_ignore_import lang Stale matches no literal reference"}
	if len(got) != len(want) || got[0] != want[0] {
		t.Fatalf("stale = %+v, want %+v", got, want)
	}
}

func TestUsedIgnoresDedupsByPathAndValue(t *testing.T) {
	c := config.New()
	c.Exts["lang"] = &IgnoreConfig{Ignores: []*IgnoreEntry{
		{Value: "a", Path: "pkg", Used: true},
		{Value: "b", Path: "pkg", Used: true},
		{Value: "a", Path: "pkg", Used: true},
		{Value: "a", Path: "other", Used: true},
		{Value: "c", Path: "pkg"},
		nil,
	}}
	want := [][2]string{{"pkg", "a"}, {"pkg", "b"}, {"other", "a"}}
	got := UsedIgnores(c, "lang")
	if len(got) != len(want) {
		t.Fatalf("used ignores = %+v, want %+v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Errorf("ignore %d = %v, want %v", i, got[i], want[i])
		}
	}
}

func TestUsedImportsKeepsOnlyUsedEntries(t *testing.T) {
	got := UsedImports("lang", []*IgnoreEntry{
		{Value: "a", Path: "pkg", Used: true},
		{Value: "b", Path: "pkg"},
		nil,
	})
	want := []IgnoredImport{{Path: "pkg", Language: "lang", Value: "a"}}
	if len(got) != len(want) || got[0] != want[0] {
		t.Fatalf("used imports = %+v, want %+v", got, want)
	}
}
