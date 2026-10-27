package cc

import (
	"context"
	"os"
	"path"
	"path/filepath"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	bzl "github.com/bazelbuild/buildtools/build"
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
	if strings.Join(got.Names, ",") != "pkg/demo/helper.h,stuff/thing.h" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper.h stuff/thing.h]", got.Names)
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
	if got := importNames(lang.Imports(&config.Config{}, lib, rule.EmptyFile("pkg/BUILD.bazel", "pkg"))); got != "pkg/demo.h,demo.h,pkg/helper.h,helper.h" {
		t.Errorf("package imports = %s, want path and basename ids", got)
	}
	if got := importNames(lang.Imports(&config.Config{}, lib, rule.EmptyFile("BUILD.bazel", ""))); got != "demo.h,helper.h" {
		t.Errorf("root imports = %s, want one id per header", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule("filegroup", "demo"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(LibraryKind, "empty"), nil); got != nil {
		t.Errorf("headerless imports = %+v, want nil", got)
	}
}

func importNames(specs []resolve.ImportSpec) string {
	names := make([]string, 0, len(specs))
	for _, s := range specs {
		names = append(names, s.Imp)
	}
	return strings.Join(names, ",")
}

func TestResolveSameNamedHeadersByPath(t *testing.T) {
	l := NewLanguage().(*common.SingleLang)
	l.Before(context.Background())
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return l })
	for _, lib := range []struct{ pkg, hdr string }{{"a", "config.h"}, {"b", "config.h"}, {"a/sub", "x.h"}} {
		r := rule.NewRule(LibraryKind, path.Base(lib.pkg))
		r.SetAttr("hdrs", []string{lib.hdr})
		index.AddRule(config.New(), r, rule.EmptyFile(path.Join(lib.pkg, "BUILD.bazel"), lib.pkg))
	}
	index.Finish()
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolveDeps := func(pkg string, includes ...string) string {
		r := rule.NewRule(LibraryKind, path.Base(pkg))
		l.Resolve(cfg, index, nil, r, common.WrapImportSet(includes), label.New("", pkg, path.Base(pkg)))
		return strings.Join(r.AttrStrings("deps"), ",")
	}
	if got := resolveDeps("app", "a/config.h", "b/config.h"); got != "//a,//b" {
		t.Errorf("app deps = %q, want //a,//b", got)
	}
	if got := resolveDeps("a", "config.h", "b/config.h"); got != "//b" {
		t.Errorf("a deps = %q, want //b", got)
	}
	if got := resolveDeps("a", "sub/x.h"); got != "//a/sub" {
		t.Errorf("a sub deps = %q, want //a/sub", got)
	}
	if l.Failed() {
		t.Fatal("path includes recorded a failure")
	}
	resolveDeps("app", "config.h")
	if !l.Failed() {
		t.Error("bare config.h with two candidates resolved, want ambiguous")
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

func androidLib(t *testing.T, files map[string]string) *rule.Rule {
	t.Helper()
	full := make(map[string]string, len(files))
	regular := make([]string, 0, len(files))
	for name, content := range files {
		full["pkg/demo/"+name] = content
		regular = append(regular, name)
	}
	result := generateFixture(t, full, regular)
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	return result.Gen[0]
}

func TestGenerateAndroidUnguarded(t *testing.T) {
	lib := androidLib(t, map[string]string{
		"activity.c": "#include <android/native_activity.h>\n#include <android/log.h>\n",
		"looper.c":   "#include <android/looper.h>\n#include <android/log.h>\n",
	})
	if got := strings.Join(lib.AttrStrings("target_compatible_with"), ","); got != "@platforms//os:android" {
		t.Errorf("target_compatible_with = %q", got)
	}
	if got := strings.Join(lib.AttrStrings("linkopts"), ","); got != "-landroid,-llog" {
		t.Errorf("linkopts = %q, want -landroid,-llog", got)
	}
	if lib.Attr("alwayslink") != nil {
		t.Errorf("alwayslink set without an entry point")
	}
}

func TestGenerateAndroidGuardedSelectsLinkopts(t *testing.T) {
	lib := androidLib(t, map[string]string{
		"activity.c": "#include <stddef.h>\n#ifdef __ANDROID__\n#include <android/log.h>\n#include <android/native_activity.h>\n#else\ntypedef struct ANativeActivity ANativeActivity;\n#endif\n",
	})
	if lib.Attr("target_compatible_with") != nil {
		t.Errorf("target_compatible_with set for guarded includes")
	}
	got := bzl.FormatString(lib.Attr("linkopts"))
	want := "select({\n    \"@platforms//os:android\": [\n        \"-landroid\",\n        \"-llog\",\n    ],\n    \"//conditions:default\": [],\n})"
	if got != want {
		t.Errorf("linkopts = %s, want %s", got, want)
	}
}

func TestGenerateJNIOnlyAlwayslink(t *testing.T) {
	lib := androidLib(t, map[string]string{
		"jni_bridge.cc": "#include <jni.h>\n\nextern \"C\" JNIEXPORT jstring JNICALL\nJava_org_example_adoptandroid_NativeBridge_greeting(JNIEnv* env, jclass) {\n  return env->NewStringUTF(\"hi\");\n}\n",
	})
	if lib.Attr("target_compatible_with") != nil || lib.Attr("linkopts") != nil {
		t.Errorf("jni.h alone must stay host-neutral")
	}
	if v, ok := lib.Attr("alwayslink").(*bzl.Ident); !ok || v.Name != "True" {
		t.Errorf("alwayslink = %v, want True", lib.Attr("alwayslink"))
	}
}

func TestGenerateUnknownAndroidHeaderAddsNoLib(t *testing.T) {
	lib := androidLib(t, map[string]string{"api.c": "#include <android/api-level.h>\n"})
	if got := strings.Join(lib.AttrStrings("target_compatible_with"), ","); got != "@platforms//os:android" {
		t.Errorf("target_compatible_with = %q", got)
	}
	if lib.Attr("linkopts") != nil {
		t.Errorf("linkopts set for a header with no library")
	}
}

func TestGenerateHostSourceUnchanged(t *testing.T) {
	lib := androidLib(t, map[string]string{"host.cc": "#include <string>\n#include <vector>\nint Count() { return 1; }\n"})
	for _, attr := range []string{"target_compatible_with", "linkopts", "alwayslink"} {
		if lib.Attr(attr) != nil {
			t.Errorf("%s set on a host source", attr)
		}
	}
}

func TestGenerateAndroidIncludeInComment(t *testing.T) {
	lib := androidLib(t, map[string]string{"host.cc": "// #include <android/log.h>\n/* #include <android/looper.h> */\nint Count() { return 1; }\n"})
	for _, attr := range []string{"target_compatible_with", "linkopts"} {
		if lib.Attr(attr) != nil {
			t.Errorf("%s set from a commented include", attr)
		}
	}
}

func TestCcKindsLeaveAndroidAttrsUnmergeable(t *testing.T) {
	for kind, info := range ccKinds {
		for _, attr := range []string{"target_compatible_with", "linkopts", "alwayslink"} {
			if info.MergeableAttrs[attr] {
				t.Errorf("%s: %s must stay unmergeable", kind, attr)
			}
		}
	}
}
