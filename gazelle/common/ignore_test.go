package common

import (
	"reflect"
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

func TestIgnoreEntriesNilReceiver(t *testing.T) {
	if got := (*IgnoreConfig)(nil).IgnoreEntries(); got != nil {
		t.Fatalf("nil entries = %+v, want nil", got)
	}
	entries := []*IgnoreEntry{{Value: "Widget", Path: "pkg"}}
	if got := (&IgnoreConfig{Ignores: entries}); len(got.IgnoreEntries()) != 1 {
		t.Fatalf("entries = %+v, want the declared entry", got.IgnoreEntries())
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
		{Value: "Used", Path: "pkg", UsedBy: []string{"pkg"}},
		nil,
		{Value: "Stale", Path: "pkg"},
		{Value: "Elsewhere", Path: "other"},
	}, generatedRels("pkg"))
	want := []GenerationError{{
		Language: "lang",
		Package:  InPackage("pkg"),
		Message:  "lang: //pkg: stale # gazelle:dx_ignore_import lang Stale matches no literal reference",
	}}
	if len(got) != len(want) || got[0] != want[0] {
		t.Fatalf("stale = %+v, want %+v", got, want)
	}
}

func TestStaleIgnoresZeroGeneratedReportsNothing(t *testing.T) {
	if got := StaleIgnores("lang", []*IgnoreEntry{{Value: "Stale", Path: "pkg"}}, GeneratedRels{}); got != nil {
		t.Fatalf("stale = %+v, want none", got)
	}
}

func generatedRels(rels ...string) GeneratedRels {
	var g GeneratedRels
	for _, rel := range rels {
		g.Add(rel)
	}
	return g
}

func TestUsedIgnoresDedupsByPathAndValue(t *testing.T) {
	c := config.New()
	c.Exts["lang"] = &IgnoreConfig{Ignores: []*IgnoreEntry{
		{Value: "a", Path: "pkg", UsedBy: []string{"pkg/y"}},
		{Value: "b", Path: "pkg", UsedBy: []string{"pkg"}},
		{Value: "a", Path: "pkg", UsedBy: []string{"pkg/x", "pkg/y"}},
		{Value: "a", Path: "other", UsedBy: []string{"other"}},
		{Value: "c", Path: "pkg"},
		nil,
	}}
	want := []IgnoredImport{
		{Path: "pkg", Language: "lang", Value: "a", UsedBy: []string{"pkg/x", "pkg/y"}},
		{Path: "pkg", Language: "lang", Value: "b", UsedBy: []string{"pkg"}},
		{Path: "other", Language: "lang", Value: "a", UsedBy: []string{"other"}},
	}
	if got := UsedIgnores(c, "lang"); !reflect.DeepEqual(got, want) {
		t.Fatalf("used ignores = %+v, want %+v", got, want)
	}
}

func TestMarkUsedKeepsUsersSortedAndUnique(t *testing.T) {
	entry := &IgnoreEntry{Value: "a", Path: "pkg"}
	if entry.Used() {
		t.Fatal("fresh entry reports used")
	}
	for _, rel := range []string{"pkg/b", "pkg", "pkg/b", "pkg/a"} {
		entry.MarkUsed(rel)
	}
	if want := []string{"pkg", "pkg/a", "pkg/b"}; !entry.Used() || !reflect.DeepEqual(entry.UsedBy, want) {
		t.Fatalf("used by = %v, want %v", entry.UsedBy, want)
	}
}
