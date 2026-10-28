package zig

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

func generate(t *testing.T, lang language.Language, files map[string]string) language.GenerateResult {
	t.Helper()
	return generateWithBuild(t, lang, files, "")
}

func generateWithBuild(t *testing.T, lang language.Language, files map[string]string, build string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "greet")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	var regular []string
	for name, content := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		regular = append(regular, name)
	}
	args := language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/greet",
		RegularFiles: regular,
	}
	if build != "" {
		f, err := rule.LoadData(filepath.Join(dir, "BUILD.bazel"), "pkg/greet", []byte(build))
		if err != nil {
			t.Fatal(err)
		}
		args.File = f
	}
	return lang.GenerateRules(args)
}

func failed(lang language.Language) bool { return lang.(*zigLang).Failed() }

func TestGenerateLibraryWithRootAndSources(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"greet.zig":      "const util = @import(\"util.zig\");\nconst fmt = @import(\"fmt_lib\");\n",
		"util.zig":       "const std = @import(\"std\");\n",
		"build.zig":      "const b = @import(\"never_parsed\");\n",
		"notes.txt":      "not a source\n",
	})
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules (failed=%v), want 1", len(result.Gen), failed(lang))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "greet" || lib.AttrString("main") != "greet.zig" {
		t.Fatalf("library = %s(%s, main=%s)", lib.Kind(), lib.Name(), lib.AttrString("main"))
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "util.zig" {
		t.Errorf("srcs = %q, want util.zig", got)
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "fmt_lib" {
		t.Errorf("imports = %v, want [fmt_lib]", got)
	}
	if lib.Attr("extra_srcs") != nil {
		t.Errorf("extra_srcs = %v, want none", lib.AttrStrings("extra_srcs"))
	}
}

func TestGenerateTracksZonAndEmbeddedFiles(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"greet.zig":    "const util = @import(\"util.zig\");\nconst data = @import(\"data.zon\");\nconst text = @embedFile(\"greeting.txt\");\n",
		"util.zig":     "const banner = @embedFile(\"greeting.txt\");\n",
		"data.zon":     ".{ .name = \"greet\" }\n",
		"greeting.txt": "Hello\n",
	})
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules (failed=%v), want 1", len(result.Gen), failed(lang))
	}
	lib := result.Gen[0]
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "util.zig" {
		t.Errorf("srcs = %q, want util.zig", got)
	}
	if got := strings.Join(lib.AttrStrings("extra_srcs"), ","); got != "data.zon,greeting.txt" {
		t.Errorf("extra_srcs = %q, want data.zon,greeting.txt", got)
	}
}

func TestGenerateFailsOnUntrackableFileRefs(t *testing.T) {
	cases := map[string]map[string]string{
		"parentImport": {"greet.zig": "const x = @import(\"../other/x.zig\");\n"},
		"subdirEmbed":  {"greet.zig": "const logo = @embedFile(\"assets/logo.png\");\n"},
		"windowsPath":  {"greet.zig": "const logo = @embedFile(\"assets\\\\logo.png\");\n"},
		"missingEmbed": {"greet.zig": "const m = @embedFile(\"missing.txt\");\n"},
		"missingZig":   {"greet.zig": "const m = @import(\"gone.zig\");\n"},
		"dotDotSubdir": {"greet.zig": "const logo = @embedFile(\"./assets/logo.png\");\n"},
		"testImport": {
			"greet.zig":      "const t = @import(\"greet_test.zig\");\n",
			"greet_test.zig": "const std = @import(\"std\");\n",
		},
		"buildImport": {
			"greet.zig": "const b = @import(\"build.zig\");\n",
			"build.zig": "const std = @import(\"std\");\n",
		},
	}
	for name, files := range cases {
		lang := NewLanguage()
		result := generate(t, lang, files)
		if !failed(lang) || len(result.Gen) != 0 {
			t.Errorf("%s: generated %d rules (failed=%v), want 0 with a recorded failure", name, len(result.Gen), failed(lang))
		}
	}
}

func TestGenerateResolvesDotSlashRefs(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"greet.zig": "const util = @import(\"./util.zig\");\nconst data = @embedFile(\"./data.txt\");\n",
		"util.zig":  "const std = @import(\"std\");\n",
		"data.txt":  "hi\n",
	})
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules (failed=%v), want 1", len(result.Gen), failed(lang))
	}
	lib := result.Gen[0]
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "util.zig" {
		t.Errorf("srcs = %q, want util.zig", got)
	}
	if got := strings.Join(lib.AttrStrings("extra_srcs"), ","); got != "data.txt" {
		t.Errorf("extra_srcs = %q, want data.txt", got)
	}
}

func TestGenerateAcceptsGenruleOutputs(t *testing.T) {
	lang := NewLanguage()
	result := generateWithBuild(t, lang, map[string]string{
		"greet.zig": "const s = @import(\"schema.zig\");\nconst c = @embedFile(\"catalog.txt\");\n",
	}, "genrule(\n    name = \"gen\",\n    outs = [\"schema.zig\", \"catalog.txt\"],\n    cmd = \"touch $@\",\n)\n")
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules (failed=%v), want 1", len(result.Gen), failed(lang))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("extra_srcs"), ","); got != "catalog.txt,schema.zig" {
		t.Errorf("extra_srcs = %q, want catalog.txt,schema.zig", got)
	}
}

func TestGenerateKeepsHandWrittenExtraSrcs(t *testing.T) {
	lang := NewLanguage()
	result := generateWithBuild(t, lang, map[string]string{
		"greet.zig": "const name = \"data.zon\";\nconst d = @embedFile(name);\n",
		"data.zon":  ".{ .name = \"greet\" }\n",
	}, "zig_library(\n    name = \"greet\",\n    main = \"greet.zig\",\n    extra_srcs = [\n        \"data.zon\",\n        \"gone.txt\",\n        \":schema_zig\",\n    ],\n)\n")
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules (failed=%v), want 1", len(result.Gen), failed(lang))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("extra_srcs"), ","); got != ":schema_zig,data.zon" {
		t.Errorf("extra_srcs = %q, want :schema_zig,data.zon", got)
	}
}

func TestGenerateSkipsTheCModule(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"greet.zig": "const c = @import(\"c\");\nconst std = @import(\"std\");\n",
	})
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules (failed=%v), want 1", len(result.Gen), failed(lang))
	}
	if got := result.Imports[0].(common.ImportSet).Names; len(got) != 0 {
		t.Errorf("imports = %v, want none", got)
	}
}

func TestGenerateMainFails(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"greet.zig": "pub fn main() void {}\n",
	})
	if len(result.Gen) != 0 || !failed(lang) {
		t.Fatalf("generated %d rules for a main file, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateWithoutRootFails(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"a.zig": "const x = 1;\n",
		"b.zig": "const y = 2;\n",
	})
	if len(result.Gen) != 0 || !failed(lang) {
		t.Fatalf("generated %d rules without a root, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestImportsUseImportNameOrTargetName(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "greet")
	if got := lang.Imports(nil, lib, nil); len(got) != 1 || got[0].Imp != "greet" || got[0].Lang != languageName {
		t.Errorf("imports = %+v, want greet", got)
	}
	lib.SetAttr("import_name", "hello")
	if got := lang.Imports(nil, lib, nil); len(got) != 1 || got[0].Imp != "hello" {
		t.Errorf("imports = %+v, want hello", got)
	}
	if got := lang.Imports(nil, rule.NewRule("filegroup", "greet"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
}

func resolveWith(t *testing.T, libs map[string]*rule.Rule, imports []string) (*rule.Rule, language.Language) {
	t.Helper()
	lang := NewLanguage()
	lang.(*zigLang).Before(context.Background())
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "", &rule.File{})
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	for pkg, lib := range libs {
		f := rule.EmptyFile(filepath.Join(pkg, "BUILD.bazel"), pkg)
		lib.Insert(f)
		index.AddRule(cfg, lib, f)
	}
	index.Finish()
	r := rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet(imports), label.New("", "app", "app"))
	return r, lang
}

func TestResolveFindsLibraryByImportName(t *testing.T) {
	renamed := rule.NewRule(LibraryKind, "greet_lib")
	renamed.SetAttr("main", "greet.zig")
	renamed.SetAttr("import_name", "greet")
	r, lang := resolveWith(t, map[string]*rule.Rule{"pkg/greet": renamed}, []string{"greet", "std"})
	if failed(lang) {
		t.Fatal("resolution failed")
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//pkg/greet:greet_lib" {
		t.Errorf("deps = %q, want //pkg/greet:greet_lib", got)
	}
}

func TestResolveReportsUnknownModules(t *testing.T) {
	_, lang := resolveWith(t, nil, []string{"missing"})
	if !failed(lang) {
		t.Fatal("an unknown module resolved")
	}
}

func TestUnresolvedImportFailsGeneration(t *testing.T) {
	_, lang := resolveWith(t, nil, []string{"missing"})
	lang.(*zigLang).AfterResolvingDeps(context.Background())
	msg := strings.Join(common.Messages(common.Errors.Take()), "\n")
	if !strings.Contains(msg, `unresolved import "missing"`) {
		t.Fatalf("errors = %q, want the unresolved import", msg)
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
	cases := []struct {
		repoName, want, main string
	}{
		{"demo-repo", "demo_repo", "demo_repo.zig"},
		{"", "lib", "root.zig"},
		{"demo-repo", "demo", "demo.zig"},
		{"demo-repo", "demo_repo", "demo_repo.zig"},
	}
	zons := []string{"", "", ".{ .name = .demo, }", ".{ .version = \"1\" }"}
	for i, tc := range cases {
		files := map[string]string{
			tc.main:    "const util = @import(\"util.zig\");\n",
			"util.zig": "const std = @import(\"std\");\n",
		}
		if zons[i] != "" {
			files[ManifestFile] = zons[i]
		}
		result := generateRoot(t, tc.repoName, files)
		if len(result.Gen) != 1 {
			t.Fatalf("repo %q: generated %d rules, want 1", tc.repoName, len(result.Gen))
		}
		lib := result.Gen[0]
		if lib.Kind() != LibraryKind || lib.Name() != tc.want || lib.AttrString("main") != tc.main {
			t.Fatalf("repo %q: library = %s(%s, main=%s), want %s(main=%s)", tc.repoName, lib.Kind(), lib.Name(), lib.AttrString("main"), tc.want, tc.main)
		}
		if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "util.zig" {
			t.Errorf("repo %q: srcs = %q, want util.zig", tc.repoName, got)
		}
	}
}

func TestGenerateTestSourcesGetTestRule(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"greet.zig":      "const std = @import(\"std\");\n",
		"greet_test.zig": "const std = @import(\"std\");\nconst greet = @import(\"greet.zig\");\n",
	})
	if failed(lang) || len(result.Gen) != 2 {
		t.Fatalf("generated %d rules (failed=%v), want 2", len(result.Gen), failed(lang))
	}
	if lib := result.Gen[0]; lib.Kind() != LibraryKind || lib.AttrString("main") != "greet.zig" || len(lib.AttrStrings("srcs")) != 0 {
		t.Errorf("library = %s(%s) main=%q srcs=%v, want main greet.zig only", lib.Kind(), lib.Name(), lib.AttrString("main"), lib.AttrStrings("srcs"))
	}
	test := result.Gen[1]
	if test.Kind() != TestKind || test.Name() != "greet_test" || test.AttrString("main") != "greet_test.zig" {
		t.Errorf("test = %s(%s) main=%q, want zig_test(greet_test) main=greet_test.zig", test.Kind(), test.Name(), test.AttrString("main"))
	}
}
