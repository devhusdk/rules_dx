package cc

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ralvik/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func writeFixture(t *testing.T, root, name, content string) {
	t.Helper()
	file := filepath.Join(root, filepath.FromSlash(name))
	if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(file, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func generateFixture(t *testing.T, files map[string]string, regular []string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	for name, content := range files {
		writeFixture(t, root, name, content)
	}
	dir := filepath.Join(root, "pkg", "demo")
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: regular,
	})
}

func TestGeneratePackageLevelLibrary(t *testing.T) {
	regular := []string{"demo.cc", "helper.cc", "helper.h", "notes.txt", "demo_test.cc"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.cc":      "#include <string>\n#include \"stuff/thing.h\"\n",
		"pkg/demo/helper.cc":    "#include \"pkg/demo/helper.h\"\n",
		"pkg/demo/helper.h":     "#pragma once\n#include <string>\n",
		"pkg/demo/notes.txt":    "not a source\n",
		"pkg/demo/demo_test.cc": "#include \"stuff/only_by_test.h\"\nint main() { return 0; }\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s), want cc_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.cc,helper.cc" {
		t.Errorf("library srcs = %q, want demo.cc,helper.cc", got)
	}
	if got := strings.Join(lib.AttrStrings("hdrs"), ","); got != "helper.h" {
		t.Errorf("library hdrs = %q, want helper.h", got)
	}
	got := result.Imports[0].(common.ImportSet)
	if strings.Join(got.Names, ",") != "helper.h,thing.h" {
		t.Errorf("library imports = %+v, want [helper.h thing.h]", got.Names)
	}
}

func TestGenerateTestSourcesExcluded(t *testing.T) {
	regular := []string{"demo.cc", "demo_test.cc"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.cc":      "#include <string>\n",
		"pkg/demo/demo_test.cc": "#include \"stuff/only_by_test.h\"\nint main() { return 0; }\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "demo.cc" {
		t.Errorf("library srcs = %q, want demo.cc", got)
	}
	if got := result.Imports[0].(common.ImportSet); len(got.Names) != 0 {
		t.Errorf("library imports = %+v, want empty", got.Names)
	}
}

func TestGenerateHeaderOnly(t *testing.T) {
	regular := []string{"helper.h"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/helper.h": "#pragma once\n#include <string>\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	if attr := result.Gen[0].Attr("srcs"); attr != nil {
		t.Errorf("header-only srcs = %v, want absent", result.Gen[0].AttrStrings("srcs"))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("hdrs"), ","); got != "helper.h" {
		t.Errorf("header-only hdrs = %q, want helper.h", got)
	}
}

func TestGenerateNoSourcesStaleSweep(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "demo")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: []string{"notes.txt"},
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want 0", len(result.Gen))
	}
}

func TestGenerateMainFails(t *testing.T) {
	regular := []string{"demo.cc", "main.cc"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.cc": "#include <string>\n",
		"pkg/demo/main.cc": "#include <iostream>\nint main() { return 0; }\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a main mix, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateKindMismatchFails(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/demo.cc", "#include <string>\n")
	existing := &rule.File{}
	fg := rule.NewRule("filegroup", "demo")
	existing.Rules = append(existing.Rules, fg)
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.cc"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestImportsIndexNonTestHeaders(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"demo.cc", "helper.cc", "demo_test.cc", "notes.txt"})
	lib.SetAttr("hdrs", []string{"demo.h", "helper.h", "demo_test.h"})
	imports := lang.Imports(&config.Config{}, lib, nil)
	if len(imports) != 2 || imports[0].Lang != languageName || imports[0].Imp != "demo.h" || imports[1].Imp != "helper.h" {
		t.Errorf("imports = %+v, want demo.h+helper.h", imports)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule("filegroup", "demo"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(LibraryKind, "empty"), nil); got != nil {
		t.Errorf("headerless imports = %+v, want nil", got)
	}
}

func TestIgnoreDirectiveUsesThisLanguageName(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	cfg := config.New()
	for _, value := range []string{"cc Dropped", "cc cc Second", "other Foreign", ""} {
		lang.Configure(cfg, "app", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: value}}})
	}
	if lang.Failed() {
		t.Fatal("a foreign or empty ignore was rejected")
	}
	conf := cfg.Exts[languageName].(*common.IgnoreConfig)
	if len(conf.Ignores) != 2 || conf.Ignores[0].Value != "Dropped" || conf.Ignores[1].Value != "Second" {
		t.Fatalf("ignores = %+v, want Dropped and Second", conf.Ignores)
	}
	lang.Configure(cfg, "app", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "cc"}}})
	if !lang.Failed() {
		t.Fatal("a one-field ignore was accepted")
	}
	if got := CollectUsedIgnores(cfg); got != nil {
		t.Fatalf("unused ignores = %v, want none", got)
	}
}

func TestAfterResolvingDepsReportsFailures(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	lang.Fail("cc: boom")
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("AfterResolvingDeps did not panic with recorded errors")
		}
		if msg, ok := r.(string); !ok || !strings.Contains(msg, "CC generation failed") {
			t.Fatalf("panic = %v, want CC generation failure", r)
		}
	}()
	lang.AfterResolvingDeps(context.Background())
}

func TestLanguageMetadata(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
	if lang.Name() != languageName || len(lang.Kinds()) != 1 {
		t.Fatalf("metadata = %s / %v", lang.Name(), lang.Kinds())
	}
	loads := lang.ApparentLoads(func(name string) string {
		if name == "rules_dx" {
			return "renamed_dx"
		}
		return ""
	})
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//cc/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != LibraryKind {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := lang.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//cc/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
}
