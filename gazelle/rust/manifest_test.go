package rust

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"

	"github.com/ralvik/rules_dx/gazelle/common"
)

func manifestConfig() *config.Config {
	return &config.Config{
		RepoRoot:             "/repo",
		ValidBuildFileNames:  []string{"BUILD.bazel"},
		ModuleToApparentName: func(string) string { return "" },
	}
}

func readIntended(t *testing.T, path string) common.IntendedManifest {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	var manifest common.IntendedManifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		t.Fatalf("Unmarshal: %v", err)
	}
	return manifest
}

func TestBeforeWiresTheRecorder(t *testing.T) {
	l := &rustLang{}
	l.Before(context.Background())
	if l.manifest != nil {
		t.Fatal("Before without env sets recorder")
	}
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	l.Before(context.Background())
	if l.manifest == nil {
		t.Fatal("Before with env leaves recorder nil")
	}
	if l.manifest.Prefix != "rust" {
		t.Errorf("recorder prefix = %q, want rust", l.manifest.Prefix)
	}
	if l.manifest.ApparentLoads == nil {
		t.Error("Before with env leaves the apparent loads unset")
	}
	if l.manifest.IncludeOtherGen {
		t.Error("rust witnesses only its own rules")
	}
	l.GenerateRules(language.GenerateArgs{Config: manifestConfig(), Dir: "/missing", Rel: "gone"})
	if len(l.manifest.Visited) != 1 || l.manifest.Visited[0].Rel != "gone" {
		t.Fatalf("visited = %+v, want one recorded visit", l.manifest.Visited)
	}
}

func TestUsedImportsDropsStaleEntries(t *testing.T) {
	got := common.UsedImports(languageName, []*common.IgnoreEntry{
		{Value: "used_crate", Path: "pkg", Used: true},
		{Value: "stale_crate", Path: "pkg"},
		{Value: "used_crate", Path: "other", Used: true},
	})
	want := []common.IgnoredImport{
		{Path: "pkg", Language: languageName, Value: "used_crate"},
		{Path: "other", Language: languageName, Value: "used_crate"},
	}
	if len(got) != len(want) {
		t.Fatalf("used ignores = %+v, want %+v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Errorf("used ignore %d = %+v, want %+v", i, got[i], want[i])
		}
	}
}

func TestCollectUsedIgnoresReadsTheRustConfigExtension(t *testing.T) {
	cfg := config.New()
	if got := CollectUsedIgnores(cfg); got != nil {
		t.Fatalf("absent: %v", got)
	}
	cfg.Exts[languageName] = &rustConfig{}
	if got := CollectUsedIgnores(cfg); got != nil {
		t.Fatalf("empty: %v", got)
	}
	cfg.Exts[languageName] = &rustConfig{
		IgnoreConfig: common.IgnoreConfig{Ignores: []*common.IgnoreEntry{
			{Value: "used_crate", Path: "pkg", Used: true},
			{Value: "stale_crate", Path: "pkg"},
		}},
		tools: []string{"cargo"},
	}
	want := [][2]string{{"pkg", "used_crate"}}
	got := CollectUsedIgnores(cfg)
	if len(got) != len(want) || got[0] != want[0] {
		t.Fatalf("used ignores = %v, want %v", got, want)
	}
	if tools := selectedNativeTools(cfg); len(tools) != 1 || tools[0] != "cargo" {
		t.Fatalf("native tools = %v, want [cargo]", tools)
	}
}

func TestAfterResolvingDepsEmits(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	l := &rustLang{}
	l.manifest = &common.ManifestRecorder{
		Prefix:        "rust",
		OutPath:       out,
		Mode:          "default",
		Scopes:        []common.ScopeElement{{Element: "//...", Dirs: []string{""}}},
		ApparentLoads: l.ApparentLoads,
	}
	l.AfterResolvingDeps(context.Background())
	manifest := readIntended(t, out)
	if manifest.Mode != "default" || len(manifest.Files) != 0 || len(manifest.IgnoredImports) != 0 {
		t.Errorf("manifest = %+v", manifest)
	}
	if manifest.SchemaMajor != common.IntendedManifestSchemaMajor || manifest.SchemaMinor != common.IntendedManifestSchemaMinor {
		t.Errorf("manifest schema = %d.%d, want %d.%d", manifest.SchemaMajor, manifest.SchemaMinor,
			common.IntendedManifestSchemaMajor, common.IntendedManifestSchemaMinor)
	}
}
