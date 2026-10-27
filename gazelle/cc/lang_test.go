package cc

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

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

func refusal(t *testing.T, lang language.Language) string {
	t.Helper()
	lm, ok := lang.(language.LifecycleManager)
	if !ok {
		t.Fatalf("%T does not implement language.LifecycleManager", lang)
	}
	lm.AfterResolvingDeps(context.Background())
	got := common.Errors.Take()
	if len(got) != 1 {
		t.Fatalf("refusals = %#v, want exactly one", got)
	}
	return got[0].Message
}

func refuseFixture(t *testing.T, files map[string]string, regular []string) string {
	t.Helper()
	root := t.TempDir()
	for name, content := range files {
		writeFixture(t, root, name, content)
	}
	lang := NewLanguage()
	result := lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: regular,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want 0 with a recorded failure", len(result.Gen))
	}
	return refusal(t, lang)
}

func wantRefusal(t *testing.T, got, want string) {
	t.Helper()
	if !strings.Contains(got, want) {
		t.Fatalf("refusal = %q, want it to contain %q", got, want)
	}
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
	got := refuseFixture(t, map[string]string{
		"pkg/demo/demo.cc": "#include <string>\n",
		"pkg/demo/main.cc": "#include <iostream>\nint main() { return 0; }\n",
	}, regular)
	wantRefusal(t, got, "cc: pkg/demo: main.cc defines main;")
}

func TestGenerateKindMismatchFails(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/demo.cc", "#include <string>\n")
	existing := &rule.File{}
	fg := rule.NewRule("filegroup", "demo")
	existing.Rules = append(existing.Rules, fg)
	lang := NewLanguage()
	result := lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.cc"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
	wantRefusal(t, refusal(t, lang), "cc: pkg/demo: target name \"demo\" is claimed by generated cc_library(pkg/demo) and existing filegroup")
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
}

func TestAfterResolvingDepsReportsFailures(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	lang.Fail("cc: boom")
	lang.AfterResolvingDeps(context.Background())
	got := common.Errors.Take()
	if len(got) != 1 || got[0].Language != languageName || got[0].Message != "cc: boom" {
		t.Fatalf("sink = %#v, want one cc failure", got)
	}
}

func TestLanguageMetadata(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
	if lang.Name() != languageName || len(lang.Kinds()) != 2 {
		t.Fatalf("metadata = %s / %v", lang.Name(), lang.Kinds())
	}
	loads := lang.ApparentLoads(func(name string) string {
		if name == "rules_dx" {
			return "renamed_dx"
		}
		return ""
	})
	if len(loads) != 2 || loads[0].Name != "@renamed_dx//cc/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != LibraryKind ||
		loads[1].Name != "@renamed_dx//cuda/rules:defs.bzl" || strings.Join(loads[1].Symbols, ",") != CudaKind {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := lang.Loads(); len(defaults) != 2 || defaults[0].Name != "@rules_dx//cc/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
}

func generateRoot(t *testing.T, repoName string, files map[string]string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	var regular []string
	for name, content := range files {
		if err := os.WriteFile(filepath.Join(root, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		regular = append(regular, name)
	}
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root, RepoName: repoName},
		Dir:          root,
		Rel:          "",
		RegularFiles: regular,
	})
}

func TestGenerateRootPackage(t *testing.T) {
	for _, tc := range []struct{ repoName, want string }{{"demo-repo", "demo_repo"}, {"", "lib"}} {
		result := generateRoot(t, tc.repoName, map[string]string{"lib.cc": "#include <string>\n"})
		if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind || result.Gen[0].Name() != tc.want {
			t.Fatalf("repo %q: generated %+v, want one cc_library(%s)", tc.repoName, result.Gen, tc.want)
		}
		if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "lib.cc" {
			t.Errorf("repo %q: srcs = %q, want lib.cc", tc.repoName, got)
		}
	}
}

func cudaFixture(t *testing.T, build string, archs []string, files map[string]string) (language.Language, language.GenerateResult) {
	t.Helper()
	root := t.TempDir()
	var regular []string
	for name, content := range files {
		writeFixture(t, root, "pkg/demo/"+name, content)
		regular = append(regular, name)
	}
	cfg := &config.Config{RepoRoot: root, Exts: map[string]interface{}{}}
	if archs != nil {
		cfg.Exts[archsDirective] = archs
	}
	var f *rule.File
	if build != "" {
		var err error
		f, err = rule.LoadData(filepath.Join(root, "pkg/demo/BUILD.bazel"), "pkg/demo", []byte(build))
		if err != nil {
			t.Fatal(err)
		}
	}
	lang := NewLanguage()
	res := lang.GenerateRules(language.GenerateArgs{Config: cfg, Dir: filepath.Join(root, "pkg", "demo"), Rel: "pkg/demo", File: f, RegularFiles: regular})
	return lang, res
}

func TestCudaGenerateOwnedByDirectory(t *testing.T) {
	_, res := cudaFixture(t, "", []string{"compute_89:compute_89,sm_89", "sm_120"}, map[string]string{
		"k.cu": "#include \"pkg/demo/k.cuh\"\n", "k.cuh": "#pragma once\n", "k_test.cu": "int main() {}\n",
	})
	if len(res.Gen) != 1 || res.Gen[0].Kind() != CudaKind || res.Gen[0].Name() != "demo_cuda" {
		t.Fatalf("gen = %+v", res.Gen)
	}
	r := res.Gen[0]
	if got := strings.Join(r.AttrStrings("srcs"), ","); got != "k.cu" {
		t.Errorf("srcs = %q", got)
	}
	if got := strings.Join(r.AttrStrings("hdrs"), ","); got != "k.cuh" {
		t.Errorf("hdrs = %q", got)
	}
	if got := strings.Join(r.AttrStrings("archs"), "|"); got != "compute_89:compute_89,sm_89|sm_120" {
		t.Errorf("archs = %q", got)
	}
	if len(res.Imports) != 1 {
		t.Errorf("imports = %v", res.Imports)
	}
}

func TestCudaMissingDirectiveRefuses(t *testing.T) {
	lang, res := cudaFixture(t, "", nil, map[string]string{"k.cu": "\n"})
	if len(res.Gen) != 0 {
		t.Fatalf("gen = %+v", res.Gen)
	}
	wantRefusal(t, refusal(t, lang), "cc: pkg/demo: .cu/.cuh files need # gazelle:dx_cuda_archs <arch>...")
}

func TestCudaMainRefuses(t *testing.T) {
	lang, _ := cudaFixture(t, "", []string{"sm_89"}, map[string]string{"k.cu": "int main() { return 0; }\n"})
	wantRefusal(t, refusal(t, lang), "k.cu defines main")
}

func TestCudaBinaryNameClashRefuses(t *testing.T) {
	lang, _ := cudaFixture(t, "cuda_binary(name = \"demo\", srcs = [\"main.cu\"], archs = [\"sm_89\"])\n", []string{"sm_89"}, map[string]string{"main.cu": "int main() {}\n", "k.cu": "\n"})
	wantRefusal(t, refusal(t, lang), "already creates demo_cuda")
}

func TestCudaOwnedFilesProduceNothing(t *testing.T) {
	_, res := cudaFixture(t, "cuda_library(name = \"kernel\", srcs = [\"k.cu\"], hdrs = [\"k.cuh\"], archs = [\"sm_89\"])\n", nil, map[string]string{"k.cu": "\n", "k.cuh": "\n"})
	if len(res.Gen) != 0 || len(res.Empty) != 0 {
		t.Fatalf("result = %+v", res)
	}
}

func TestCudaStaleOwnedTargetIsEmptied(t *testing.T) {
	_, res := cudaFixture(t, "cuda_library(name = \"demo_cuda\", srcs = [\"gone.cu\"], archs = [\"sm_89\"])\ncuda_library(name = \"hand\", srcs = [\"h.cu\"])\n", nil, map[string]string{"h.cu": "\n"})
	if len(res.Gen) != 0 || len(res.Empty) != 1 || res.Empty[0].Name() != "demo_cuda" {
		t.Fatalf("result = %+v", res)
	}
}

func TestImportsIndexCudaHeadersOnly(t *testing.T) {
	lang := NewLanguage()
	r := rule.NewRule(CudaKind, "demo_cuda")
	r.SetAttr("hdrs", []string{"k.cuh", "shared.h"})
	got := lang.Imports(&config.Config{}, r, nil)
	if len(got) != 1 || got[0].Imp != "k.cuh" {
		t.Fatalf("imports = %+v, want only k.cuh", got)
	}
}
