package javascript

import (
	"context"
	"flag"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/ralvik/rules_dx/gazelle/common"
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

func TestGenerateSourceOnlyPackage(t *testing.T) {
	regular := []string{"demo.js", "helper.jsx", "helper_test.mjs", "notes.txt", "widget.d.ts"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.js":         "import helper from \"./helper.jsx\";\nimport fs from \"fs\";\n",
		"pkg/demo/helper.jsx":      "export const suffix = (t) => t;\n",
		"pkg/demo/helper_test.mjs": "import helper from \"./helper.jsx\";\n",
		"pkg/demo/notes.txt":       "not a source\n",
		"pkg/demo/widget.d.ts":     "export declare const x: number;\n",
	}, regular)
	if len(result.Gen) != 3 || len(result.Imports) != 3 {
		t.Fatalf("generated %d rules and %d import sets, want 3 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != libraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.js" {
		t.Errorf("library srcs = %q", got)
	}
	helper := result.Gen[1]
	if helper.Kind() != libraryKind || helper.Name() != "helper" {
		t.Fatalf("helper = %s(%s)", helper.Kind(), helper.Name())
	}
	if got := strings.Join(helper.AttrStrings("srcs"), ","); got != "helper.jsx" {
		t.Errorf("helper srcs = %q", got)
	}
	test := result.Gen[2]
	if test.Kind() != testKind || test.Name() != "helper_test" {
		t.Fatalf("test = %s(%s)", test.Kind(), test.Name())
	}
	if got := strings.Join(test.AttrStrings("srcs"), ","); got != "helper_test.mjs" {
		t.Errorf("test srcs = %q", got)
	}
	if got := test.AttrString("node_modules"); got != rootNodeModules {
		t.Errorf("test node_modules = %q, want %q", got, rootNodeModules)
	}
	testImports := result.Imports[2].(targetImports)
	if strings.Join(testImports.imports, ",") != "helper" {
		t.Errorf("test imports = %+v, want [helper]", testImports)
	}
	libImports := result.Imports[0].(targetImports)
	if strings.Join(libImports.imports, ",") != "helper" {
		t.Errorf("library imports = %+v", libImports)
	}
	helperImports := result.Imports[1].(targetImports)
	if len(helperImports.imports) != 0 {
		t.Errorf("helper imports = %+v, want empty", helperImports)
	}
}

func TestGenerateRelativeStdlibCollision(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/util.js": "export function fmt(name) { return `hello ${name}`; }\n",
		"pkg/demo/app.js":  "import { fmt } from \"./util.js\";\nimport fs from \"fs\";\nexport const greeting = fmt(\"ada\");\n",
	}, []string{"app.js", "util.js"})
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	appImports := result.Imports[0].(targetImports)
	if strings.Join(appImports.imports, ",") != "util" {
		t.Fatalf("app imports = %+v, want [util]", appImports)
	}
	if !appImports.local["util"] {
		t.Fatalf("app imports = %+v, want util marked relative", appImports)
	}
	l := &javascriptLang{}
	index := resolverIndex(l,
		struct {
			pkg, name string
			ext       string
		}{"pkg/demo", "util", ".js"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, index, nil, r, appImports, label.New("", "pkg/demo", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != ":util" {
		t.Errorf("resolved deps = %q, want %q", got, ":util")
	}
	if len(l.errors) != 0 {
		t.Errorf("resolve errors = %v, want none", l.errors)
	}
}

func TestGenerateEmptySweepsStale(t *testing.T) {
	result := generateFixture(t, nil, []string{"notes.txt"})
	if len(result.Gen) != 0 || len(result.Empty) != 0 {
		t.Fatalf("empty generation = %+v", result)
	}
	f := rule.EmptyFile("BUILD.bazel", "pkg/demo")
	f.Rules = append(f.Rules,
		rule.NewRule(libraryKind, "old"),
		rule.NewRule("filegroup", "keep"),
		rule.NewRule(libraryKind, "demo"),
	)
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/demo.js", "export const x = 1;\n")
	dir := filepath.Join(root, "pkg", "demo")
	result = NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.js"},
		File:         f,
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if len(result.Empty) != 1 || result.Empty[0].Name() != "old" {
		t.Fatalf("empty rules = %v, want [old]", result.Empty)
	}
}

func TestGenerateFailures(t *testing.T) {
	cases := []struct {
		name    string
		files   map[string]string
		regular []string
		want    string
	}{
		{"readError", map[string]string{}, []string{"missing.js"}, "read missing.js"},
		{"emptyName", map[string]string{"pkg/demo/---.js": "export const x = 1;\n"}, []string{"---.js"}, "empty target name"},
		{"collision", map[string]string{
			"pkg/demo/a-b.js":  "export const x = 1;\n",
			"pkg/demo/a_b.jsx": "export const y = 2;\n",
		}, []string{"a-b.js", "a_b.jsx"}, "claimed by a-b.js, a_b.jsx"},
	}
	for _, tc := range cases {
		root := t.TempDir()
		for name, content := range tc.files {
			writeFixture(t, root, name, content)
		}
		l := &javascriptLang{}
		result := l.GenerateRules(language.GenerateArgs{
			Config:       &config.Config{RepoRoot: root},
			Dir:          filepath.Join(root, "pkg", "demo"),
			Rel:          "pkg/demo",
			RegularFiles: tc.regular,
		})
		if len(result.Gen) != 0 {
			t.Errorf("%s: generated %d rules, want none", tc.name, len(result.Gen))
		}
		if len(l.errors) != 1 || !strings.Contains(l.errors[0], tc.want) {
			t.Errorf("%s: errors = %v, want %q", tc.name, l.errors, tc.want)
		}
	}
}

func TestCheckClaims(t *testing.T) {
	claimants := []Claimant{{Name: "demo", Source: "demo.js"}}
	if err := checkClaims(nil, nil, claimants); err != nil {
		t.Errorf("unique claims = %v", err)
	}
	sameKind := rule.EmptyFile("BUILD.bazel", "pkg")
	sameKind.Rules = append(sameKind.Rules, rule.NewRule(libraryKind, "demo"))
	if err := checkClaims(sameKind, nil, claimants); err != nil {
		t.Errorf("same-kind handwritten claims = %v", err)
	}
	otherSame := []*rule.Rule{rule.NewRule(libraryKind, "demo")}
	if err := checkClaims(nil, otherSame, claimants); err != nil {
		t.Errorf("same-kind other claims = %v", err)
	}
	wrongKind := rule.EmptyFile("BUILD.bazel", "pkg")
	wrongKind.Rules = append(wrongKind.Rules, rule.NewRule("filegroup", "demo"))
	wrongKind.Rules = append(wrongKind.Rules, rule.NewRule("filegroup", "a_b"))
	if err := checkClaims(wrongKind, nil, claimants); err == nil || !strings.Contains(err.Error(), "existing filegroup") {
		t.Errorf("kind mismatch = %v", err)
	}
	otherWrong := []*rule.Rule{rule.NewRule(testKind, "demo")}
	if err := checkClaims(nil, otherWrong, claimants); err == nil || !strings.Contains(err.Error(), "existing javascript_test") {
		t.Errorf("other kind mismatch = %v", err)
	}
	plainClaimants := []Claimant{{Name: "demo", Source: "demo_test.js", Kind: testKind}}
	plainFile := rule.EmptyFile("BUILD.bazel", "pkg")
	plainFile.Rules = append(plainFile.Rules, rule.NewRule(plainTestKind, "demo"))
	if err := checkClaims(plainFile, nil, plainClaimants); err != nil {
		t.Errorf("plain-runner handwritten claims = %v", err)
	}
	plainOther := []*rule.Rule{rule.NewRule(plainTestKind, "demo")}
	if err := checkClaims(nil, plainOther, plainClaimants); err != nil {
		t.Errorf("plain-runner other claims = %v", err)
	}
	dupes := []Claimant{{Name: "a_b", Source: "a-b.js"}, {Name: "a_b", Source: "a_b.jsx"}}
	if err := checkClaims(wrongKind, nil, dupes); err == nil || !strings.Contains(err.Error(), "handwritten") {
		t.Errorf("collision with handwritten = %v", err)
	}
}

func TestGeneratePlainTestKeepsHandwrittenRunner(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/plain_test.js", "import { test } from \"node:test\";\ntest(\"ok\", () => {});\n")
	file := rule.EmptyFile("BUILD.bazel", "pkg/demo")
	file.Rules = append(file.Rules, rule.NewRule(plainTestKind, "plain_test"))
	l := &javascriptLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"plain_test.js"},
		File:         file,
	})
	if len(l.errors) != 0 {
		t.Fatalf("errors = %v, want none", l.errors)
	}
	for _, r := range result.Gen {
		if r.Kind() == testKind && r.Name() == "plain_test" {
			t.Fatalf("emitted %s(%s) beside hand-written %s", r.Kind(), r.Name(), plainTestKind)
		}
	}
	if len(result.Gen) != len(result.Imports) {
		t.Fatalf("generated %d rules and %d import sets, want aligned", len(result.Gen), len(result.Imports))
	}
}

func TestMergeStale(t *testing.T) {
	if got := mergeStale(nil, language.GenerateResult{}); len(got.Empty) != 0 {
		t.Errorf("nil file stale = %v", got.Empty)
	}
	f := rule.EmptyFile("BUILD.bazel", "pkg")
	f.Rules = append(f.Rules,
		rule.NewRule(libraryKind, "old"),
		rule.NewRule("filegroup", "keep"),
	)
	kept := rule.NewRule(libraryKind, "demo")
	result := mergeStale(f, language.GenerateResult{Gen: []*rule.Rule{kept}})
	if len(result.Empty) != 1 || result.Empty[0].Name() != "old" {
		t.Fatalf("stale = %v", result.Empty)
	}
}

func TestImportsIndexesLibraries(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(libraryKind, "demo")
	lib.SetAttr("srcs", []string{"demo.js", "notes.txt"})
	imports := lang.Imports(&config.Config{}, lib, nil)
	if len(imports) != 1 || imports[0].Lang != languageName || imports[0].Imp != "demo" {
		t.Errorf("imports = %+v", imports)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(testKind, "demo_test"), nil); got != nil {
		t.Errorf("test imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(libraryKind, "empty"), nil); got != nil {
		t.Errorf("srcless imports = %+v, want nil", got)
	}
}

func TestErrorsAbortBeforeEmission(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	l.fail("second")
	l.fail("first")
	defer func() {
		got := recover()
		if got == nil {
			t.Fatal("AfterResolvingDeps did not abort")
		}
		if message := got.(string); !strings.Contains(message, "first\nsecond") {
			t.Errorf("errors were not deterministic: %s", message)
		}
	}()
	l.AfterResolvingDeps(context.Background())
}

func TestAfterResolvingDepsClean(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("errors = %v", l.errors)
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := &javascriptLang{}
	if l.Name() != "javascript" || len(l.Kinds()) != 3 || l.CheckFlags(flag.NewFlagSet("test", flag.ContinueOnError), config.New()) != nil {
		t.Fatal("invalid language metadata")
	}
	l.RegisterFlags(flag.NewFlagSet("test", flag.ContinueOnError), "update", config.New())
	l.Configure(config.New(), "pkg", nil)
	l.DoneGeneratingRules()
	if got := l.KnownDirectives(); len(got) != 1 || got[0] != "dx_ignore_import" {
		t.Fatalf("directives = %v, want [dx_ignore_import]", got)
	}
	if l.Embeds(nil, label.NoLabel) != nil {
		t.Fatal("invalid directives or embeds")
	}
	loads := l.ApparentLoads(func(name string) string {
		if name == "rules_dx" {
			return "renamed_dx"
		}
		return ""
	})
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//javascript/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "javascript_library,javascript_test,javascript_binary" {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := l.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//javascript/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
	defaultApparent := l.ApparentLoads(func(string) string { return "" })
	if defaultApparent[0].Name != "@rules_dx//javascript/rules:defs.bzl" {
		t.Errorf("default apparent loads = %+v", defaultApparent)
	}
}

func resolverConfig(t *testing.T, directives []rule.Directive) *config.Config {
	t.Helper()
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "app", &rule.File{Directives: directives})
	return cfg
}

func resolverIndex(lang *javascriptLang, entries ...struct {
	pkg  string
	name string
	ext  string
}) *resolve.RuleIndex {
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := javascriptKinds[r.Kind()]; ok {
			return lang
		}
		return nil
	})
	for _, entry := range entries {
		ext := entry.ext
		if ext == "" {
			ext = ".js"
		}
		r := rule.NewRule(libraryKind, entry.name)
		r.SetAttr("srcs", []string{entry.name + ext})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry.pkg, "BUILD.bazel"), entry.pkg))
	}
	index.Finish()
	return index
}

func TestResolveBranches(t *testing.T) {
	l := &javascriptLang{}
	index := resolverIndex(l,
		struct {
			pkg, name string
			ext       string
		}{"lib/b", "b", ".js"},
		struct {
			pkg, name string
			ext       string
		}{"lib/a", "a", ".jsx"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, index, nil, r, targetImports{imports: []string{"b", "a"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a,//lib/b" {
		t.Errorf("resolved deps = %q", got)
	}

	self := rule.NewRule(libraryKind, "a")
	l.Resolve(cfg, index, nil, self, targetImports{imports: []string{"a"}}, label.New("", "lib/a", "a"))
	if self.Attr("deps") != nil {
		t.Errorf("self dependency was emitted: %v", self.AttrStrings("deps"))
	}

	std := rule.NewRule(libraryKind, "uses_std")
	l.Resolve(cfg, index, nil, std, targetImports{imports: []string{"fs"}}, label.New("", "app", "uses_std"))
	if std.Attr("deps") != nil || len(l.errors) != 0 {
		t.Errorf("stdlib resolution = %v, errors = %v", std.AttrStrings("deps"), l.errors)
	}

	hand := rule.NewRule(libraryKind, "hand")
	hand.SetAttr("deps", []string{":kept"})
	l.Resolve(cfg, index, nil, hand, targetImports{imports: []string{"a"}}, label.New("", "app", "hand"))
	if got := strings.Join(hand.AttrStrings("deps"), ","); got != "//lib/a,:kept" {
		t.Errorf("merged deps = %q", got)
	}

	l.Resolve(cfg, index, nil, rule.NewRule(libraryKind, "unknown"), targetImports{imports: []string{"unknown"}}, label.New("", "app", "unknown"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("unresolved errors = %v", l.errors)
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}

func TestResolveTestData(t *testing.T) {
	l := &javascriptLang{}
	index := resolverIndex(l, struct {
		pkg, name string
		ext       string
	}{"lib/a", "a", ".js"})
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(testKind, "app_test")
	r.SetAttr("srcs", []string{"app_test.js"})
	r.SetAttr("node_modules", rootNodeModules)
	l.Resolve(cfg, index, nil, r, targetImports{imports: []string{"a"}}, label.New("", "app", "app_test"))
	if got := strings.Join(r.AttrStrings("data"), ","); got != "//lib/a:a" && got != "//lib/a" {
		t.Errorf("test data = %q, want //lib/a", got)
	}
	if r.Attr("deps") != nil {
		t.Errorf("test must not set deps: %v", r.AttrStrings("deps"))
	}
}

func TestResolveOverride(t *testing.T) {
	l := &javascriptLang{}
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "javascript javascript mapped //mapped:dep"}})
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"mapped"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
}

func TestResolveAmbiguous(t *testing.T) {
	l := &javascriptLang{}
	cfg := resolverConfig(t, nil)
	index := resolverIndex(l,
		struct {
			pkg, name string
			ext       string
		}{"one", "same", ".js"},
		struct {
			pkg, name string
			ext       string
		}{"two", "same", ".jsx"},
	)
	l.Resolve(cfg, index, nil, rule.NewRule(libraryKind, "app"), targetImports{imports: []string{"same"}}, label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "ambiguous") || !strings.Contains(l.errors[0], "//one:same") || !strings.Contains(l.errors[0], "//two:same") {
		t.Errorf("ambiguity = %v", l.errors)
	}
}

func TestUnionStrings(t *testing.T) {
	if got := unionStrings([]string{"b", "a", "a"}, []string{"c", "a"}); strings.Join(got, ",") != "a,b,c" {
		t.Errorf("union = %q", got)
	}
	if got := unionStrings(nil, nil); len(got) != 0 {
		t.Errorf("empty union = %q", got)
	}
}

func TestGenerateEntryThinBinary(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/main.js":        "import helper from \"./helper.js\";\nconsole.log(helper);\n",
		"pkg/demo/helper.js":      "export const helper = 1;\n",
		"pkg/demo/helper_test.js": "import helper from \"./helper.js\";\n",
	}, []string{"main.js", "helper.js", "helper_test.js"})
	if len(result.Gen) != 4 || len(result.Imports) != 4 {
		t.Fatalf("generated %d rules and %d import sets, want 4 each", len(result.Gen), len(result.Imports))
	}
	byRule := make(map[string]int, len(result.Gen))
	for i, r := range result.Gen {
		byRule[r.Kind()+"\x00"+r.Name()] = i
	}
	libIdx, ok := byRule[libraryKind+"\x00main"]
	if !ok {
		t.Fatalf("missing javascript_library(main) in %v", result.Gen)
	}
	binIdx, ok := byRule[binaryKind+"\x00main_bin"]
	if !ok {
		t.Fatalf("missing javascript_binary(main_bin) in %v", result.Gen)
	}
	lib := result.Gen[libIdx]
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "main.js" {
		t.Errorf("library srcs = %q, want main.js", got)
	}
	bin := result.Gen[binIdx]
	if got := bin.AttrString("entry_point"); got != "main.js" {
		t.Errorf("binary entry_point = %q, want main.js", got)
	}
	if got := strings.Join(bin.AttrStrings("data"), ","); got != ":main" {
		t.Errorf("binary data = %q, want :main", got)
	}
	if bin.Attr("srcs") != nil {
		t.Errorf("thin binary must own no srcs, got %v", bin.AttrStrings("srcs"))
	}
	if raw := result.Imports[libIdx].(targetImports); strings.Join(raw.imports, ",") != "helper" {
		t.Errorf("library imports = %+v, want [helper]", raw)
	}
	if raw := result.Imports[binIdx].(targetImports); len(raw.imports) != 0 {
		t.Errorf("binary imports = %+v, want empty", raw)
	}
}

func TestGenerateEntryCollision(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/main.js":     "export const x = 1;\n",
		"pkg/demo/main_bin.js": "export const y = 2;\n",
	} {
		writeFixture(t, root, name, content)
	}
	l := &javascriptLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"main.js", "main_bin.js"},
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want none", len(result.Gen))
	}
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "main_bin") {
		t.Errorf("errors = %v, want main_bin collision", l.errors)
	}
}

func TestBinaryKindInfo(t *testing.T) {
	info := binaryKindInfo()
	for _, attr := range info.MatchAttrs {
		if attr == "srcs" {
			t.Errorf("binary MatchAttrs must not contain srcs: %v", info.MatchAttrs)
		}
	}
	if len(info.MatchAttrs) != 1 || info.MatchAttrs[0] != "entry_point" {
		t.Errorf("binary MatchAttrs = %v, want [entry_point]", info.MatchAttrs)
	}
	if info.NonEmptyAttrs["srcs"] {
		t.Errorf("binary NonEmptyAttrs must not require srcs: %v", info.NonEmptyAttrs)
	}
	if !info.MergeableAttrs["data"] {
		t.Errorf("binary MergeableAttrs = %v, want data", info.MergeableAttrs)
	}
	if !info.ResolveAttrs["data"] {
		t.Errorf("binary ResolveAttrs = %v, want data", info.ResolveAttrs)
	}
}

func TestResolvePreservesBinaryData(t *testing.T) {
	l := &javascriptLang{}
	cfg := resolverConfig(t, nil)
	bin := rule.NewRule(binaryKind, "main_bin")
	bin.SetAttr("entry_point", "main.js")
	bin.SetAttr("data", []string{":main"})
	l.Resolve(cfg, resolverIndex(l), nil, bin, targetImports{}, label.New("", "app", "main_bin"))
	if got := strings.Join(bin.AttrStrings("data"), ","); got != ":main" {
		t.Errorf("binary data = %q, want preserved :main", got)
	}
	if len(l.errors) != 0 {
		t.Errorf("binary resolve errors = %v", l.errors)
	}
}

func TestMergeStaleCleansBinary(t *testing.T) {
	f := rule.EmptyFile("BUILD.bazel", "pkg")
	f.Rules = append(f.Rules, rule.NewRule(binaryKind, "old_bin"))
	keptLib := rule.NewRule(libraryKind, "main")
	keptBin := rule.NewRule(binaryKind, "main_bin")
	result := mergeStale(f, language.GenerateResult{Gen: []*rule.Rule{keptLib, keptBin}})
	if len(result.Empty) != 1 || result.Empty[0].Name() != "old_bin" {
		t.Fatalf("stale = %v, want [old_bin]", result.Empty)
	}
}

func TestIgnoreDirectiveInheritanceAndStaleCheck(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	root := config.New()
	file, err := rule.LoadData("BUILD.bazel", "", []byte("# gazelle:dx_ignore_import javascript missing_dep\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(root, "", file)
	child := root.Clone()
	l.Configure(child, "child", nil)
	ignore := common.MatchingIgnore(child, languageName, "missing_dep")
	if ignore == nil || ignore.Path != "" {
		t.Fatalf("inherited ignore = %+v", ignore)
	}
	ignore.Used = true
	l.AfterResolvingDeps(context.Background())
}

func TestStaleIgnoreFails(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import javascript stale\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	defer func() {
		if got := recover(); got == nil || !strings.Contains(got.(string), "stale") {
			t.Errorf("stale ignore result = %v", got)
		}
	}()
	l.AfterResolvingDeps(context.Background())
}

func TestResolveIgnore(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import javascript external\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"external"}}, label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("ignored import produced deps = %v", r.AttrStrings("deps"))
	}
	if len(l.errors) != 0 {
		t.Errorf("ignored import errors = %v", l.errors)
	}
	l.AfterResolvingDeps(context.Background())
}

func TestResolveMappingIgnoreConflict(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "javascript javascript mapped //mapped:dep"}})
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import javascript mapped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"mapped"}}, label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "both an exact resolve mapping and ignore") {
		t.Errorf("conflict errors = %v", l.errors)
	}
}

func TestConfigureSkipsOtherDirectives(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:resolve javascript javascript foo //foo:bar\n# gazelle:dx_ignore_import python foo\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	if got := common.MatchingIgnore(cfg, languageName, "foo"); got != nil {
		t.Errorf("other-directive ignore = %+v, want nil", got)
	}
	if len(l.errors) != 0 {
		t.Errorf("other-directive errors = %v", l.errors)
	}
}

func TestConfigureThreeFieldAndMalformed(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import javascript javascript mydep\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	ignore := common.MatchingIgnore(cfg, languageName, "mydep")
	if ignore == nil || ignore.Value != "mydep" {
		t.Fatalf("three-field ignore = %+v, want mydep", ignore)
	}
	ignore.Used = true

	malformed := &javascriptLang{}
	malformed.Before(context.Background())
	badCfg := config.New()
	badFile, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import javascript\n"))
	if err != nil {
		t.Fatal(err)
	}
	malformed.Configure(badCfg, "pkg", badFile)
	if len(malformed.errors) != 1 || !strings.Contains(malformed.errors[0], "malformed") {
		t.Errorf("malformed errors = %v", malformed.errors)
	}
	l.AfterResolvingDeps(context.Background())
}

func TestClaimKind(t *testing.T) {
	if got := claimKind(Claimant{Name: "x", Source: "x.js", Kind: binaryKind}); got != binaryKind {
		t.Errorf("explicit kind = %q, want %q", got, binaryKind)
	}
	if got := claimKind(Claimant{Name: "a_test", Source: "a_test.js"}); got != testKind {
		t.Errorf("test kind = %q, want %q", got, testKind)
	}
	if got := claimKind(Claimant{Name: "a", Source: "a.js"}); got != libraryKind {
		t.Errorf("library kind = %q, want %q", got, libraryKind)
	}
}

func TestMatchingIgnoreMiss(t *testing.T) {
	l := &javascriptLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import javascript other\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	if got := common.MatchingIgnore(cfg, languageName, "missing"); got != nil {
		t.Errorf("ignore miss = %+v, want nil", got)
	}
	if got := common.MatchingIgnore(config.New(), languageName, "missing"); got != nil {
		t.Errorf("no-exts ignore = %+v, want nil", got)
	}
}
