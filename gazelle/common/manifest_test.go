package common

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"unicode/utf8"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/merger"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

var recorderPrefixes = []string{"dispatch", "rust"}

func manifestConfig() *config.Config {
	return &config.Config{
		RepoRoot:             "/repo",
		ValidBuildFileNames:  []string{"BUILD.bazel"},
		ModuleToApparentName: func(string) string { return "" },
	}
}

func manifestLoads() func(func(string) string) []rule.LoadInfo {
	return func(func(string) string) []rule.LoadInfo {
		return []rule.LoadInfo{{Name: "@rules_dx//dot/rules:defs.bzl", Symbols: []string{dotKind}}}
	}
}

func manifestRecorder(t *testing.T, prefix string) *ManifestRecorder {
	t.Helper()
	return &ManifestRecorder{
		Prefix:        prefix,
		OutPath:       filepath.Join(t.TempDir(), "intended.json"),
		Mode:          "default",
		Scopes:        []ScopeElement{{Element: "//...", Dirs: []string{""}}},
		ApparentLoads: manifestLoads(),
	}
}

func mustLoadData(t *testing.T, path, pkg, content string) *rule.File {
	t.Helper()
	f, err := rule.LoadData(path, pkg, []byte(content))
	if err != nil {
		t.Fatalf("LoadData: %v", err)
	}
	return f
}

func applyEdits(t *testing.T, original []byte, edits []IntendedEdit) []byte {
	t.Helper()
	var out []byte
	cursor := uint64(0)
	for _, edit := range edits {
		if edit.Start < cursor || edit.Start > edit.End || edit.End > uint64(len(original)) {
			t.Fatalf("edit %+v out of order or bounds for %d bytes", edit, len(original))
		}
		out = append(out, original[cursor:edit.Start]...)
		out = append(out, edit.Replacement...)
		cursor = edit.End
	}
	return append(out, original[cursor:]...)
}

func checkEditsValid(t *testing.T, original []byte, edits []IntendedEdit) {
	t.Helper()
	previous := uint64(0)
	for _, edit := range edits {
		if edit.Start < previous || edit.Start > edit.End || edit.End > uint64(len(original)) {
			t.Fatalf("edits not ordered within bounds: %+v", edits)
		}
		for _, at := range []uint64{edit.Start, edit.End} {
			if !utf8.ValidString(string(original[:at])) {
				t.Fatalf("edit boundary %d splits a UTF-8 sequence", at)
			}
		}
		if string(original[edit.Start:edit.End]) == string(edit.Replacement) {
			t.Fatalf("no-op edit %+v", edit)
		}
		previous = edit.Start
	}
}

func stableContent(t *testing.T, rec *ManifestRecorder, path, pkg, seed string) string {
	t.Helper()
	content := []byte(seed)
	for i := 0; i < 3; i++ {
		f := mustLoadData(t, path, pkg, string(content))
		file, changed, err := rec.Witness(PackageRecord{Rel: pkg, File: f, Cfg: manifestConfig()})
		if err != nil {
			t.Fatalf("witness: %v", err)
		}
		if !changed {
			return string(content)
		}
		content = applyEdits(t, content, file.Edits)
	}
	t.Fatalf("witness did not stabilize for %s", path)
	return ""
}

func readIntended(t *testing.T, path string) IntendedManifest {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	var manifest IntendedManifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		t.Fatalf("Unmarshal: %v", err)
	}
	return manifest
}

func TestLoadManifestRecorderEnvironment(t *testing.T) {
	for _, prefix := range recorderPrefixes {
		t.Run(prefix, func(t *testing.T) {
			t.Setenv(EnvIntendedManifest, "")
			if rec, err := LoadManifestRecorder(prefix); rec != nil || err != nil {
				t.Fatalf("disabled recorder: %v, %v", rec, err)
			}
			out := filepath.Join(t.TempDir(), "intended.json")
			t.Setenv(EnvIntendedManifest, out)
			t.Setenv(EnvGenerateMode, "")
			t.Setenv(EnvGenerateScope, "")
			rec, err := LoadManifestRecorder(prefix)
			if err != nil {
				t.Fatalf("default recorder: %v", err)
			}
			if rec.Prefix != prefix || rec.OutPath != out || rec.Mode != "default" {
				t.Fatalf("default recorder = %+v", rec)
			}
			if len(rec.Scopes) != 1 || rec.Scopes[0].Element != "//..." || len(rec.Scopes[0].Dirs) != 1 || rec.Scopes[0].Dirs[0] != "" {
				t.Fatalf("scopes = %+v, want repo-wide default", rec.Scopes)
			}
			t.Setenv(EnvGenerateMode, "check")
			t.Setenv(EnvGenerateScope, `[{"element":"//pkg/...","dirs":["pkg"]}]`)
			rec, err = LoadManifestRecorder(prefix)
			if err != nil || rec.Mode != "check" {
				t.Fatalf("explicit recorder = %+v, %v", rec, err)
			}
			if len(rec.Scopes) != 1 || rec.Scopes[0].Element != "//pkg/..." || rec.Scopes[0].Dirs[0] != "pkg" {
				t.Fatalf("scopes = %+v, want explicit scope", rec.Scopes)
			}
			if rec.ScopeIndex("pkg/inner") != 0 || rec.ScopeIndex("other") != -1 {
				t.Fatalf("scope index of pkg/inner = %d, other = %d", rec.ScopeIndex("pkg/inner"), rec.ScopeIndex("other"))
			}
		})
	}
}

func TestLoadManifestRecorderRejectsBadInput(t *testing.T) {
	for _, prefix := range recorderPrefixes {
		t.Run(prefix, func(t *testing.T) {
			out := filepath.Join(t.TempDir(), "intended.json")
			t.Setenv(EnvIntendedManifest, out)
			t.Setenv(EnvGenerateMode, "print")
			_, err := LoadManifestRecorder(prefix)
			want := prefix + `: ` + EnvGenerateMode + ` must be "check" or "default", got "print"`
			if err == nil || err.Error() != want {
				t.Fatalf("bad mode error = %v, want %q", err, want)
			}
			t.Setenv(EnvGenerateMode, "check")
			for _, raw := range []string{`[{"element":`, `[]`, `null`} {
				t.Setenv(EnvGenerateScope, raw)
				_, err = LoadManifestRecorder(prefix)
				if err == nil || !strings.HasPrefix(err.Error(), prefix+": ") || !strings.Contains(err.Error(), EnvGenerateScope) {
					t.Fatalf("scope %q error = %v, want a prefixed %s error", raw, err, EnvGenerateScope)
				}
			}
		})
	}
}

func TestUniqueRulesDropsNilAndRepeats(t *testing.T) {
	first := rule.NewRule(dotKind, "first")
	got := UniqueRules([]*rule.Rule{nil, first, first}, nil, []*rule.Rule{first, rule.NewRule(dotKind, "second")})
	if len(got) != 2 || got[0] != first || got[1].AttrString("name") != "second" {
		t.Fatalf("unique rules = %+v, want first then second", got)
	}
	if UniqueRules() != nil {
		t.Fatalf("UniqueRules() = %+v, want nil", UniqueRules())
	}
}

func TestRecordKeepsSiblingRulesOnlyWhenAsked(t *testing.T) {
	sibling := rule.NewRule(dotKind, "sibling")
	own := rule.NewRule(dotKind, "own")
	cfg := manifestConfig()
	existing := mustLoadData(t, "BUILD.bazel", "a", "legacy_kind(\n    name = \"old\",\n)\n")

	for _, tc := range []struct {
		includeOtherGen bool
		want            []string
	}{
		{includeOtherGen: false, want: []string{"own"}},
		{includeOtherGen: true, want: []string{"sibling", "own"}},
	} {
		rec := &ManifestRecorder{IncludeOtherGen: tc.includeOtherGen}
		rec.Record(
			language.GenerateArgs{Rel: "a", Dir: "/repo/a", File: existing, Config: cfg, OtherGen: []*rule.Rule{nil, sibling, sibling}},
			language.GenerateResult{Gen: []*rule.Rule{own, own}},
		)
		if len(rec.Visited) != 1 {
			t.Fatalf("visited = %d, want 1", len(rec.Visited))
		}
		got := rec.Visited[0]
		if got.Rel != "a" || got.Dir != "/repo/a" || got.File != existing || got.Cfg != cfg {
			t.Fatalf("record = %+v, want the args echoed", got)
		}
		var names []string
		for _, g := range got.Gen {
			names = append(names, g.AttrString("name"))
		}
		if strings.Join(names, ",") != strings.Join(tc.want, ",") {
			t.Fatalf("gen names = %v, want %v", names, tc.want)
		}
		if len(got.GenKinds) != len(tc.want) {
			t.Fatalf("gen kinds = %v, want one per kept rule", got.GenKinds)
		}
		for _, kind := range got.GenKinds {
			if kind != dotKind {
				t.Fatalf("gen kinds = %v, want only %s", got.GenKinds, dotKind)
			}
		}
		if len(got.OldKinds) != 1 || got.OldKinds[0] != "legacy_kind" {
			t.Fatalf("old kinds = %v, want legacy_kind", got.OldKinds)
		}
	}
}

func TestScopeIndexPrefersTheLongestDir(t *testing.T) {
	rec := &ManifestRecorder{Scopes: []ScopeElement{
		{Element: "//a/b:target", Dirs: []string{"a/b"}},
		{Element: "//a/...", Dirs: []string{"a"}},
		{Element: "//...", Dirs: []string{""}},
	}}
	for rel, want := range map[string]int{
		"a/b":       0,
		"a/b/c":     0,
		"a":         1,
		"a/other":   1,
		"":          2,
		"unrelated": 2,
	} {
		if got := rec.ScopeIndex(rel); got != want {
			t.Errorf("ScopeIndex(%q) = %d, want %d", rel, got, want)
		}
	}
	narrow := &ManifestRecorder{Scopes: []ScopeElement{{Element: "//a/...", Dirs: []string{"a"}}}}
	if got := narrow.ScopeIndex("b"); got != -1 {
		t.Errorf("ScopeIndex outside narrow scope = %d, want -1", got)
	}
}

func TestSplitLinesRebuilds(t *testing.T) {
	for _, c := range []string{"", "a", "a\n", "a\nb\nc", "a\nb", "é\nx\n"} {
		var rebuilt strings.Builder
		for _, line := range splitLines([]byte(c)) {
			rebuilt.Write(line)
		}
		if rebuilt.String() != c {
			t.Errorf("splitLines(%q) rebuilds to %q", c, rebuilt.String())
		}
	}
	if splitLines(nil) != nil {
		t.Errorf("splitLines(nil) = %q, want nil", splitLines(nil))
	}
	if got := nonNilBytes(nil); got == nil || len(got) != 0 {
		t.Errorf("nonNilBytes(nil) = %q, want empty non-nil", got)
	}
}

func TestDiffLinesRoundtrips(t *testing.T) {
	cases := [][2]string{
		{"a\n", "a\n"},
		{"a\n", "b\n"},
		{"a\nb\n", "a\nx\nb\n"},
		{"x\na\nb\n", "a\nb\n"},
		{"", "load(\"x\")\n"},
		{"load(\"x\")\n", ""},
		{"", ""},
		{"", "new"},
		{"old", ""},
		{"one\ntwo", "one\nthree"},
		{"one\n", "one\ntwo\n"},
		{"no trailing\nnewline", "no trailing\nnewline\n"},
		{"café\nnaïve\n", "café\nnaive\n"},
		{"é\n", "ex\n"},
		{"line1\nline2\nline3\n", "line1\nline3\n"},
		{"a\n", "a\nb\nc\n"},
	}
	for _, c := range cases {
		edits := diffLines([]byte(c[0]), []byte(c[1]))
		if c[0] == c[1] {
			if len(edits) != 0 {
				t.Errorf("diffLines(%q, equal) = %+v, want none", c[0], edits)
			}
			continue
		}
		if len(edits) == 0 {
			t.Errorf("diffLines(%q, %q) yields no edits", c[0], c[1])
			continue
		}
		checkEditsValid(t, []byte(c[0]), edits)
		if got := string(applyEdits(t, []byte(c[0]), edits)); got != c[1] {
			t.Errorf("applyEdits(%q) = %q, want %q", c[0], got, c[1])
		}
		for _, edit := range edits {
			if edit.Replacement == nil {
				t.Fatalf("nil replacement serializes as null in %q -> %q", c[0], c[1])
			}
		}
	}
	if got := manifestPath("", "BUILD.bazel"); got != "BUILD.bazel" {
		t.Errorf("manifestPath at the root = %q, want BUILD.bazel", got)
	}
	if got := manifestPath("a/b", "BUILD.bazel"); got != "a/b/BUILD.bazel" {
		t.Errorf("manifestPath in a package = %q, want a/b/BUILD.bazel", got)
	}
}

func TestReplacementKindFollowsChainsAndRejectsCycles(t *testing.T) {
	kindMap := map[string]config.MappedKind{
		"old_kind": {FromKind: "old_kind", KindName: "new_kind", KindLoad: "@x//:defs.bzl"},
		"new_kind": {FromKind: "new_kind", KindName: "final_kind", KindLoad: "@x//:defs.bzl"},
		"same":     {FromKind: "same", KindName: "same", KindLoad: "@x//:defs.bzl"},
	}
	for _, prefix := range recorderPrefixes {
		rec := &ManifestRecorder{Prefix: prefix}
		if got, err := rec.replacementKind(kindMap, "plain"); err != nil || got != nil {
			t.Errorf("replacementKind(plain) = %+v, %v, want nil, nil", got, err)
		}
		if got, err := rec.replacementKind(kindMap, "old_kind"); err != nil || got == nil || got.KindName != "final_kind" {
			t.Errorf("replacementKind(old_kind) = %+v, %v, want transitive final_kind", got, err)
		}
		if got, err := rec.replacementKind(kindMap, "same"); err != nil || got == nil || got.KindName != "same" {
			t.Errorf("replacementKind(same) = %+v, %v, want same", got, err)
		}
		loop := map[string]config.MappedKind{
			"a": {FromKind: "a", KindName: "b", KindLoad: "@x//:defs.bzl"},
			"b": {FromKind: "b", KindName: "a", KindLoad: "@x//:defs.bzl"},
		}
		_, err := rec.replacementKind(loop, "a")
		if err == nil || err.Error() != prefix+`: kind map loop at "b"` {
			t.Errorf("replacementKind(loop) = %v, want a prefixed loop error", err)
		}
	}
}

func TestAppendOrMergeKindMapping(t *testing.T) {
	loads := []rule.LoadInfo{{Name: "@x//:defs.bzl", Symbols: []string{"old_kind"}}}
	merged := appendOrMergeKindMapping(loads, config.MappedKind{KindName: "new_kind", KindLoad: "@x//:defs.bzl"})
	if len(merged) != 1 || len(merged[0].Symbols) != 2 || merged[0].Symbols[1] != "new_kind" {
		t.Fatalf("merged loads = %+v, want symbols appended", merged)
	}
	grown := appendOrMergeKindMapping(merged, config.MappedKind{KindName: "other", KindLoad: "@y//:defs.bzl"})
	if len(grown) != 2 || grown[1].Name != "@y//:defs.bzl" {
		t.Fatalf("grown loads = %+v, want appended load", grown)
	}
}

func TestKnownLoadsAppliesKindMappings(t *testing.T) {
	rec := &ManifestRecorder{ApparentLoads: manifestLoads()}
	loads := rec.ApparentLoads(func(string) string { return "" })
	plain := PackageRecord{GenKinds: []string{dotKind}, Cfg: manifestConfig()}
	got, err := rec.KnownLoads(plain)
	if err != nil || len(got) != 1 {
		t.Fatalf("KnownLoads without a kind map = %+v, %v", got, err)
	}
	mappedCfg := manifestConfig()
	mappedCfg.KindMap = map[string]config.MappedKind{
		dotKind: {FromKind: dotKind, KindName: "custom_library", KindLoad: "@custom//:defs.bzl"},
	}
	got, err = rec.KnownLoads(PackageRecord{GenKinds: []string{dotKind}, Cfg: mappedCfg})
	if err != nil || len(got) != 2 || got[1].Name != "@custom//:defs.bzl" {
		t.Fatalf("KnownLoads with a kind map = %+v, %v", got, err)
	}
	loopCfg := manifestConfig()
	loopCfg.KindMap = map[string]config.MappedKind{
		"a": {FromKind: "a", KindName: "b", KindLoad: "@x//:defs.bzl"},
		"b": {FromKind: "b", KindName: "a", KindLoad: "@x//:defs.bzl"},
	}
	if _, err = rec.KnownLoads(PackageRecord{GenKinds: []string{"a"}, Cfg: loopCfg}); err == nil {
		t.Fatal("kind map loop accepted")
	}
	if _, err = rec.applyKindMappings(plain, loads); err != nil {
		t.Fatalf("applyKindMappings directly: %v", err)
	}
	nilLoads := &ManifestRecorder{}
	if got, err = nilLoads.KnownLoads(plain); err != nil || got != nil {
		t.Fatalf("KnownLoads without apparent loads = %+v, %v", got, err)
	}
}

func TestWitnessCreatesAWholeBuildFile(t *testing.T) {
	rec := &ManifestRecorder{IncludeOtherGen: true, ApparentLoads: manifestLoads()}
	dir := t.TempDir()
	gen := rule.NewRule(dotKind, "demo")
	gen.SetAttr("srcs", []string{"demo.dot"})
	other := rule.NewRule(dotKind, "native")
	other.SetAttr("srcs", []string{"native.dot"})
	rec.Record(
		language.GenerateArgs{Rel: "pkg", Dir: filepath.Join(dir, "pkg"), Config: manifestConfig(), OtherGen: []*rule.Rule{other}},
		language.GenerateResult{Gen: []*rule.Rule{gen}},
	)
	if len(rec.Visited) != 1 {
		t.Fatalf("visited = %d, want 1", len(rec.Visited))
	}
	file, changed, err := rec.Witness(rec.Visited[0])
	if err != nil || !changed {
		t.Fatalf("Witness new file changed = %v, err = %v", changed, err)
	}
	if file.Path != "pkg/BUILD.bazel" {
		t.Errorf("path = %q, want pkg/BUILD.bazel", file.Path)
	}
	if len(file.CreateContent) == 0 || len(file.Edits) != 0 || len(file.OriginalContent) != 0 {
		t.Errorf("new file record = %+v, want only create content", file)
	}
	content := string(file.CreateContent)
	if !strings.Contains(content, `name = "demo"`) || !strings.Contains(content, `name = "native"`) {
		t.Errorf("create content must carry both rules, got:\n%s", content)
	}
	if !strings.Contains(content, `load("@rules_dx//dot/rules:defs.bzl", "dot_library"`) {
		t.Errorf("create content must carry the apparent load, got:\n%s", content)
	}

	root, changed, err := rec.Witness(PackageRecord{Rel: "", Dir: dir, Cfg: manifestConfig(), Gen: []*rule.Rule{gen}})
	if err != nil || !changed || root.Path != "BUILD.bazel" {
		t.Fatalf("root new file = %+v, changed = %v, err = %v", root, changed, err)
	}
	empty, changed, err := rec.Witness(PackageRecord{Rel: "pkg", Dir: filepath.Join(dir, "pkg"), Cfg: manifestConfig()})
	if err != nil || changed {
		t.Fatalf("Witness empty new file changed = %v, err = %v", changed, err)
	}
	if len(empty.Path) != 0 {
		t.Fatalf("unchanged witness = %+v, want the zero file", empty)
	}
}

func TestWitnessEditsAnExistingBuildFile(t *testing.T) {
	for _, prefix := range recorderPrefixes {
		t.Run(prefix, func(t *testing.T) {
			rec := manifestRecorder(t, prefix)
			old := "load(\"@rules_dx//dot/rules:defs.bzl\", \"dot_library\")\n\ndot_library(\n    name = \"demo\",\n)\n"
			f := mustLoadData(t, "/repo/pkg/BUILD.bazel", "pkg", old)
			rule.NewRule(dotKind, "extra").Insert(f)
			rec.Record(
				language.GenerateArgs{Rel: "pkg", Dir: "/repo/pkg", File: f, Config: manifestConfig()},
				language.GenerateResult{},
			)
			file, changed, err := rec.Witness(rec.Visited[0])
			if err != nil || !changed {
				t.Fatalf("Witness changed = %v, err = %v", changed, err)
			}
			if file.Path != "pkg/BUILD.bazel" || string(file.OriginalContent) != old {
				t.Fatalf("file = %+v, want the input bytes for pkg/BUILD.bazel", file)
			}
			checkEditsValid(t, []byte(old), file.Edits)
			if got := string(applyEdits(t, []byte(old), file.Edits)); got != string(f.Format()) {
				t.Fatalf("edits apply to %q, want formatted %q", got, f.Format())
			}
			steady := stableContent(t, rec, "/repo/steady/BUILD.bazel", "steady", old)
			steadyFile := mustLoadData(t, "/repo/steady/BUILD.bazel", "steady", steady)
			rec.Record(
				language.GenerateArgs{Rel: "steady", Dir: "/repo/steady", File: steadyFile, Config: manifestConfig()},
				language.GenerateResult{},
			)
			if got, changed, err := rec.Witness(rec.Visited[1]); err != nil || changed {
				t.Fatalf("Witness on a stable file changed = %v, err = %v, file = %+v", changed, err, got)
			}
		})
	}
}

func TestWitnessFixLoadsConverges(t *testing.T) {
	f := mustLoadData(t, "/repo/pkg/BUILD.bazel", "pkg", "dot_library(\n    name = \"demo\",\n)\n")
	loads := manifestLoads()(func(string) string { return "" })
	merger.FixLoads(f, loads)
	first := string(f.Format())
	merger.FixLoads(f, loads)
	if got := string(f.Format()); got != first {
		t.Fatalf("second FixLoads changed bytes:\n%s\nvs\n%s", got, first)
	}
}

func TestWitnessReportsAKindMapLoop(t *testing.T) {
	for _, prefix := range recorderPrefixes {
		t.Run(prefix, func(t *testing.T) {
			cfg := manifestConfig()
			cfg.KindMap = map[string]config.MappedKind{
				"a": {FromKind: "a", KindName: "b", KindLoad: "@x//:defs.bzl"},
				"b": {FromKind: "b", KindName: "a", KindLoad: "@x//:defs.bzl"},
			}
			rec := manifestRecorder(t, prefix)
			gen := rule.NewRule("a", "demo")
			_, _, err := rec.Witness(PackageRecord{Rel: "pkg", Dir: "/repo/pkg", Cfg: cfg, Gen: []*rule.Rule{gen}, GenKinds: []string{"a"}})
			if err == nil || !strings.Contains(err.Error(), "kind map loop") {
				t.Fatalf("new file error = %v, want a kind map loop", err)
			}
			existing := mustLoadData(t, "/repo/pkg/BUILD.bazel", "pkg", "load(\"@x//:defs.bzl\", \"a\")\n\na(\n    name = \"demo\",\n)\n")
			_, _, err = rec.Witness(PackageRecord{Rel: "pkg", File: existing, Cfg: cfg, GenKinds: []string{"a"}})
			if err == nil || !strings.Contains(err.Error(), "kind map loop") {
				t.Fatalf("existing file error = %v, want a kind map loop", err)
			}
		})
	}
}

func TestEmitWritesOneManifest(t *testing.T) {
	for _, prefix := range recorderPrefixes {
		t.Run(prefix, func(t *testing.T) {
			out := filepath.Join(t.TempDir(), "intended.json")
			rec := manifestRecorder(t, prefix)
			rec.OutPath = out
			rec.Mode = "check"
			cfg := manifestConfig()
			old := "load(\"@rules_dx//dot/rules:defs.bzl\", \"dot_library\")\n\ndot_library(\n    name = \"demo\",\n)\n"
			changed := mustLoadData(t, "/repo/pkg/BUILD.bazel", "pkg", old)
			rule.NewRule(dotKind, "extra").Insert(changed)
			steady := mustLoadData(t, "/repo/steady/BUILD.bazel", "steady",
				stableContent(t, rec, "/repo/steady/BUILD.bazel", "steady", "load(\"@rules_dx//dot/rules:defs.bzl\", \"dot_library\")\n"))
			rec.Record(language.GenerateArgs{Rel: "pkg", Dir: "/repo/pkg", File: changed, Config: cfg}, language.GenerateResult{})
			rec.Record(language.GenerateArgs{Rel: "steady", Dir: "/repo/steady", File: steady, Config: cfg}, language.GenerateResult{})
			rec.Record(
				language.GenerateArgs{Rel: "new", Dir: "/repo/new", Config: cfg},
				language.GenerateResult{Gen: []*rule.Rule{rule.NewRule(dotKind, "fresh")}},
			)
			rec.Record(language.GenerateArgs{Rel: "empty", Dir: "/repo/empty", Config: cfg}, language.GenerateResult{})
			ignores := []IgnoredImport{
				{Path: "b", Language: prefix, Value: "zebra"},
				{Path: "b", Language: prefix, Value: "apple"},
				{Path: "a", Language: prefix, Value: "mango"},
				{Path: "a", Language: prefix, Value: "apple"},
				{Path: "a", Language: prefix, Value: "apple"},
				{Path: "a", Language: "zeta", Value: "apple"},
			}
			if err := rec.Emit(ignores); err != nil {
				t.Fatalf("Emit: %v", err)
			}
			manifest := readIntended(t, out)
			if manifest.SchemaMajor != IntendedManifestSchemaMajor || manifest.SchemaMinor != IntendedManifestSchemaMinor || manifest.Mode != "check" {
				t.Errorf("header = %+v", manifest)
			}
			if len(manifest.Scopes) != 1 || manifest.Scopes[0].Value != "//..." || !manifest.Scopes[0].ResultsComplete {
				t.Errorf("scopes = %+v", manifest.Scopes)
			}
			if len(manifest.Files) != 2 {
				t.Fatalf("files = %+v, want the changed and the new one only", manifest.Files)
			}
			if manifest.Files[0].Path != "pkg/BUILD.bazel" || manifest.Files[0].ScopeIndex != 0 || len(manifest.Files[0].Edits) == 0 {
				t.Errorf("first file = %+v", manifest.Files[0])
			}
			if string(manifest.Files[0].OriginalContent) != old {
				t.Errorf("original content = %q, want the input bytes", manifest.Files[0].OriginalContent)
			}
			checkEditsValid(t, manifest.Files[0].OriginalContent, manifest.Files[0].Edits)
			if got := string(applyEdits(t, manifest.Files[0].OriginalContent, manifest.Files[0].Edits)); got != string(changed.Format()) {
				t.Errorf("edits apply to %q, want %q", got, changed.Format())
			}
			if manifest.Files[1].Path != "new/BUILD.bazel" || !strings.Contains(string(manifest.Files[1].CreateContent), `name = "fresh"`) {
				t.Errorf("second file = %+v", manifest.Files[1])
			}
			want := [][3]string{
				{"a", prefix, "apple"},
				{"a", prefix, "mango"},
				{"a", "zeta", "apple"},
				{"b", prefix, "apple"},
				{"b", prefix, "zebra"},
			}
			if len(manifest.IgnoredImports) != len(want) {
				t.Fatalf("ignored imports = %+v, want %v", manifest.IgnoredImports, want)
			}
			for i, w := range want {
				got := manifest.IgnoredImports[i]
				if got.Path != w[0] || got.Language != w[1] || got.Import != w[2] || got.ScopeIndex != 0 {
					t.Errorf("ignored import %d = %+v, want %v", i, got, w)
				}
			}
		})
	}
}

func TestEmitFailsClosed(t *testing.T) {
	for _, prefix := range recorderPrefixes {
		t.Run(prefix, func(t *testing.T) {
			narrow := func() *ManifestRecorder {
				rec := manifestRecorder(t, prefix)
				rec.OutPath = filepath.Join(t.TempDir(), "intended.json")
				rec.Scopes = []ScopeElement{{Element: "//a/...", Dirs: []string{"a"}}}
				return rec
			}
			outside := narrow()
			outside.Record(language.GenerateArgs{Rel: "b", Dir: "/repo/b", Config: manifestConfig()}, language.GenerateResult{})
			err := outside.Emit(nil)
			if err == nil || err.Error() != prefix+`: package "b" matches no `+EnvGenerateScope+" scope element" {
				t.Errorf("file scope error = %v", err)
			}
			matched := narrow()
			err = matched.Emit([]IgnoredImport{{Path: "b", Language: prefix, Value: "x"}})
			if err == nil || err.Error() != prefix+`: ignored import "x" matches no `+EnvGenerateScope+" scope element" {
				t.Errorf("ignore scope error = %v", err)
			}
			missing := manifestRecorder(t, prefix)
			missing.OutPath = filepath.Join(t.TempDir(), "missing", "intended.json")
			err = missing.Emit(nil)
			if err == nil || !strings.HasPrefix(err.Error(), prefix+": cannot write intended manifest: ") {
				t.Errorf("write error = %v, want a prefixed write error", err)
			}
			loop := manifestRecorder(t, prefix)
			loopCfg := manifestConfig()
			loopCfg.KindMap = map[string]config.MappedKind{
				"a": {FromKind: "a", KindName: "b", KindLoad: "@x//:defs.bzl"},
				"b": {FromKind: "b", KindName: "a", KindLoad: "@x//:defs.bzl"},
			}
			loop.Record(
				language.GenerateArgs{Rel: "pkg", Dir: "/repo/pkg", Config: loopCfg},
				language.GenerateResult{Gen: []*rule.Rule{rule.NewRule("a", "demo")}},
			)
			if err = loop.Emit(nil); err == nil || !strings.Contains(err.Error(), prefix+": kind map loop") {
				t.Errorf("emit kind map loop = %v", err)
			}
		})
	}
}
