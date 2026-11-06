package javascript

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/merger"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

const jestManifest = "{\"devDependencies\": {\"jest-cli\": \"29.7.0\", \"jest-junit\": \"16.0.0\"}}\n"

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
		"package.json":             jestManifest,
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
	if got := test.AttrString("package_json"); got != rootPackageJSON {
		t.Errorf("test package_json = %q, want %q", got, rootPackageJSON)
	}
	testImports := common.RefRoots(importRefs(result.Imports[2]))
	if strings.Join(testImports, ",") != "pkg/demo/helper" {
		t.Errorf("test imports = %+v, want [pkg/demo/helper]", testImports)
	}
	libImports := common.RefRoots(importRefs(result.Imports[0]))
	if strings.Join(libImports, ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v", libImports)
	}
	helperImports := importRefs(result.Imports[1])
	if len(helperImports) != 0 {
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
	appImports := result.Imports[0]
	appRefs := importRefs(appImports)
	if strings.Join(common.RefRoots(appRefs), ",") != "pkg/demo/util" {
		t.Fatalf("app imports = %+v, want [util]", appRefs)
	}
	if !appRefs[0].Relative {
		t.Fatalf("app imports = %+v, want util marked relative", appRefs)
	}
	l := newLang()
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
	if len(l.Errors()) != 0 {
		t.Errorf("resolve errors = %v, want none", l.Errors())
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
		l := newLang()
		result := l.GenerateRules(language.GenerateArgs{
			Config:       &config.Config{RepoRoot: root},
			Dir:          filepath.Join(root, "pkg", "demo"),
			Rel:          "pkg/demo",
			RegularFiles: tc.regular,
		})
		if len(result.Gen) != 0 {
			t.Errorf("%s: generated %d rules, want none", tc.name, len(result.Gen))
		}
		if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], tc.want) {
			t.Errorf("%s: errors = %v, want %q", tc.name, l.Errors(), tc.want)
		}
	}
}

func TestGenerateFixturePathMarksTestonly(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/fixtures/demo/demo.js", "export const x = 1;\n")
	c := &config.Config{RepoRoot: root}
	common.SetFixtureTestonly(c, true)
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       c,
		Dir:          filepath.Join(root, "pkg", "fixtures", "demo"),
		Rel:          "pkg/fixtures/demo",
		RegularFiles: []string{"demo.js"},
	})
	if len(result.Gen) == 0 {
		t.Fatal("generated no rules under a fixture path")
	}
	for _, r := range result.Gen {
		if !r.AttrBool("testonly") {
			t.Errorf("%s(%s) missing testonly under a fixture path", r.Kind(), r.Name())
		}
	}
}

func TestGenerateSkipsTestClaimedByOtherPlainRunner(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/plain_test.js", "import { test } from \"node:test\";\ntest(\"ok\", () => {});\n")
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"plain_test.js"},
		OtherGen:     []*rule.Rule{nodeRunnerTest("plain_test")},
	})
	if len(l.Errors()) != 0 {
		t.Fatalf("errors = %v, want none", l.Errors())
	}
	for _, r := range result.Gen {
		if r.Kind() == testKind && r.Name() == "plain_test" {
			t.Fatalf("emitted %s(%s) beside other-pass %s", r.Kind(), r.Name(), testKind)
		}
	}
}

func TestCheckClaims(t *testing.T) {
	claimants := []Claimant{{Name: "demo", Source: "demo.js", Kind: libraryKind}}
	if err := common.CheckClaims(nil, nil, claimants, nil); err != nil {
		t.Errorf("unique claims = %v", err)
	}
	sameKind := rule.EmptyFile("BUILD.bazel", "pkg")
	sameKind.Rules = append(sameKind.Rules, rule.NewRule(libraryKind, "demo"))
	if err := common.CheckClaims(sameKind, nil, claimants, nil); err != nil {
		t.Errorf("same-kind handwritten claims = %v", err)
	}
	otherSame := []*rule.Rule{rule.NewRule(libraryKind, "demo")}
	if err := common.CheckClaims(nil, otherSame, claimants, nil); err != nil {
		t.Errorf("same-kind other claims = %v", err)
	}
	wrongKind := rule.EmptyFile("BUILD.bazel", "pkg")
	wrongKind.Rules = append(wrongKind.Rules, rule.NewRule("filegroup", "demo"))
	wrongKind.Rules = append(wrongKind.Rules, rule.NewRule("filegroup", "a_b"))
	if err := common.CheckClaims(wrongKind, nil, claimants, nil); err == nil || !strings.Contains(err.Error(), "existing filegroup") {
		t.Errorf("kind mismatch = %v", err)
	}
	otherWrong := []*rule.Rule{rule.NewRule(testKind, "demo")}
	if err := common.CheckClaims(nil, otherWrong, claimants, nil); err == nil || !strings.Contains(err.Error(), "existing javascript_test") {
		t.Errorf("other kind mismatch = %v", err)
	}
	dupes := []Claimant{{Name: "a_b", Source: "a-b.js", Kind: libraryKind}, {Name: "a_b", Source: "a_b.jsx", Kind: libraryKind}}
	if err := common.CheckClaims(wrongKind, nil, dupes, nil); err == nil || !strings.Contains(err.Error(), "handwritten") {
		t.Errorf("collision with handwritten = %v", err)
	}
}

func TestGeneratePlainTestKeepsHandwrittenRunner(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/plain_test.js", "import { test } from \"node:test\";\ntest(\"ok\", () => {});\n")
	file := rule.EmptyFile("BUILD.bazel", "pkg/demo")
	file.Rules = append(file.Rules, nodeRunnerTest("plain_test"))
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"plain_test.js"},
		File:         file,
	})
	if len(l.Errors()) != 0 {
		t.Fatalf("errors = %v, want none", l.Errors())
	}
	for _, r := range result.Gen {
		if r.Kind() == testKind && r.Name() == "plain_test" {
			t.Fatalf("emitted %s(%s) beside hand-written %s", r.Kind(), r.Name(), testKind)
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
	if len(imports) != 2 || imports[0].Lang != languageName || imports[0].Imp != "demo" || imports[1].Imp != "notes.txt" {
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
	l := newLang()
	l.Before(context.Background())
	l.Fail(common.InPackage("x"), "second")
	l.Fail(common.InPackage("x"), "first")
	l.AfterResolvingDeps(context.Background())
	if message := strings.Join(common.Messages(common.Errors.Take()), "\n"); message != "first\nsecond" {
		t.Errorf("errors were not deterministic: %s", message)
	}
}

func TestAfterResolvingDepsClean(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if len(l.Errors()) != 0 {
		t.Errorf("errors = %v", l.Errors())
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

func resolverIndex(lang *common.SingleLang, entries ...struct {
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
	l := newLang()
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
	l.Resolve(cfg, index, nil, r, refs("lib/b/b", "lib/a/a"), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a,//lib/b" {
		t.Errorf("resolved deps = %q", got)
	}

	self := rule.NewRule(libraryKind, "a")
	l.Resolve(cfg, index, nil, self, refs("lib/a/a"), label.New("", "lib/a", "a"))
	if self.Attr("deps") != nil {
		t.Errorf("self dependency was emitted: %v", self.AttrStrings("deps"))
	}

	std := rule.NewRule(libraryKind, "uses_std")
	l.Resolve(cfg, index, nil, std, refs("fs"), label.New("", "app", "uses_std"))
	if std.Attr("deps") != nil || len(l.Errors()) != 0 {
		t.Errorf("stdlib resolution = %v, errors = %v", std.AttrStrings("deps"), l.Errors())
	}

	hand := rule.NewRule(libraryKind, "hand")
	hand.SetAttr("deps", []string{":kept"})
	l.Resolve(cfg, index, nil, hand, refs("lib/a/a"), label.New("", "app", "hand"))
	if got := strings.Join(hand.AttrStrings("deps"), ","); got != "//lib/a,:kept" {
		t.Errorf("merged deps = %q", got)
	}

	l.Resolve(cfg, index, nil, rule.NewRule(libraryKind, "unknown"), refs("unknown"), label.New("", "app", "unknown"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "unresolved import") || !strings.Contains(l.Errors()[0], `or add "# gazelle:dx_ignore_import javascript unknown" to app/BUILD.bazel`) {
		t.Errorf("unresolved errors = %v", l.Errors())
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}

func TestResolveTestData(t *testing.T) {
	l := newLang()
	index := resolverIndex(l, struct {
		pkg, name string
		ext       string
	}{"lib/a", "a", ".js"})
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(testKind, "app_test")
	r.SetAttr("srcs", []string{"app_test.js"})
	r.SetAttr("node_modules", rootNodeModules)
	l.Resolve(cfg, index, nil, r, refs("lib/a/a"), label.New("", "app", "app_test"))
	if got := strings.Join(r.AttrStrings("data"), ","); got != "//lib/a:a" && got != "//lib/a" {
		t.Errorf("test data = %q, want //lib/a", got)
	}
	if r.Attr("deps") != nil {
		t.Errorf("test must not set deps: %v", r.AttrStrings("deps"))
	}
}

func TestResolveOverride(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "javascript javascript mapped //mapped:dep"}})
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, refs("mapped"), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
}

func TestResolveAmbiguous(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, nil)
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return l })
	for _, entry := range [][2]string{{"x", "same.js"}, {"y", "same.jsx"}} {
		r := rule.NewRule(libraryKind, entry[0])
		r.SetAttr("srcs", []string{entry[1]})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join("one", "BUILD.bazel"), "one"))
	}
	index.Finish()
	l.Resolve(cfg, index, nil, rule.NewRule(libraryKind, "app"), refs("one/same"), label.New("", "app", "app"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "ambiguous") {
		t.Errorf("ambiguity = %v", l.Errors())
	}
	if len(l.Errors()) == 1 && !strings.Contains(l.Errors()[0], `; add "# gazelle:resolve javascript one/same <label>" to app/BUILD.bazel`) {
		t.Errorf("ambiguous error = %q, want the resolve remedy", l.Errors()[0])
	}
}

func TestGenerateEntryThinBinary(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"package.json":            jestManifest,
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
	if raw := importRefs(result.Imports[libIdx]); strings.Join(common.RefRoots(raw), ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper]", raw)
	}
	if raw := importRefs(result.Imports[binIdx]); len(raw) != 0 {
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
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"main.js", "main_bin.js"},
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want none", len(result.Gen))
	}
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "main_bin") {
		t.Errorf("errors = %v, want main_bin collision", l.Errors())
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
	if len(info.NonEmptyAttrs) != 1 || !info.NonEmptyAttrs["data"] {
		t.Errorf("binary NonEmptyAttrs = %v, want data so stale binaries are removed", info.NonEmptyAttrs)
	}
	if !info.MergeableAttrs["data"] {
		t.Errorf("binary MergeableAttrs = %v, want data", info.MergeableAttrs)
	}
	if !info.ResolveAttrs["data"] {
		t.Errorf("binary ResolveAttrs = %v, want data", info.ResolveAttrs)
	}
}

func TestResolvePreservesBinaryData(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, nil)
	bin := rule.NewRule(binaryKind, "main_bin")
	bin.SetAttr("entry_point", "main.js")
	bin.SetAttr("data", []string{":main"})
	l.Resolve(cfg, resolverIndex(l), nil, bin, refs(), label.New("", "app", "main_bin"))
	if got := strings.Join(bin.AttrStrings("data"), ","); got != ":main" {
		t.Errorf("binary data = %q, want preserved :main", got)
	}
	if len(l.Errors()) != 0 {
		t.Errorf("binary resolve errors = %v", l.Errors())
	}
}

func regenerate(t *testing.T, files map[string]string, build string) *rule.File {
	t.Helper()
	root := t.TempDir()
	var regular []string
	for name, content := range files {
		writeFixture(t, root, "pkg/demo/"+name, content)
		regular = append(regular, name)
	}
	file, err := rule.LoadData("BUILD.bazel", "pkg/demo", []byte(build))
	if err != nil {
		t.Fatal(err)
	}
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: regular,
		File:         file,
	})
	if len(l.Errors()) != 0 {
		t.Fatalf("errors = %v", l.Errors())
	}
	if len(result.Gen) != len(result.Imports) {
		t.Fatalf("generated %d rules and %d import sets", len(result.Gen), len(result.Imports))
	}
	merger.MergeFile(file, result.Empty, result.Gen, merger.PreResolve, javascriptKinds, nil)
	return file
}

func binaries(file *rule.File) map[string]*rule.Rule {
	found := map[string]*rule.Rule{}
	for _, r := range file.Rules {
		if r.Kind() == binaryKind {
			found[r.Name()] = r
		}
	}
	return found
}

func TestMergeStaleCleansBinary(t *testing.T) {
	sources := map[string]string{"greet.js": "export const x = 1;\n", "main.js": "import { x } from \"./greet\";\n"}
	readme := binaries(regenerate(t, sources, `javascript_binary(
    name = "hello",
    data = [":greet"],
    entry_point = "main.js",
)
`))
	if readme["hello"] == nil || len(readme) != 1 {
		t.Fatalf("binaries = %v, want only hello", readme)
	}
	if got := strings.Join(readme["hello"].AttrStrings("data"), ","); got != ":greet" {
		t.Errorf("hello data = %q, want :greet", got)
	}

	kept := binaries(regenerate(t, sources, `javascript_binary(name = "main_bin", data = [":main"], entry_point = "main.js")
`))
	if kept["main_bin"] == nil || len(kept) != 1 {
		t.Fatalf("binaries = %v, want only main_bin", kept)
	}
	if got := strings.Join(kept["main_bin"].AttrStrings("data"), ","); got != ":main" {
		t.Errorf("main_bin data = %q, want :main", got)
	}

	stale := binaries(regenerate(t, map[string]string{"greet.js": "export const x = 1;\n"}, `javascript_binary(name = "main_bin", data = [":main"], entry_point = "main.js")
`))
	if len(stale) != 0 {
		t.Errorf("binaries = %v, want stale main_bin removed", stale)
	}
}

func TestIgnoreDirectiveInheritanceAndStaleCheck(t *testing.T) {
	l := newLang()
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
	ignore.MarkUsed("pkg")
	l.AfterResolvingDeps(context.Background())
}

func TestStaleIgnoreFails(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import javascript stale\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	l.GenerateRules(language.GenerateArgs{Rel: "pkg"})
	l.AfterResolvingDeps(context.Background())
	if got := strings.Join(common.Messages(common.Errors.Take()), "\n"); !strings.Contains(got, "stale") {
		t.Errorf("stale ignore result = %q", got)
	}
}

func TestResolveIgnore(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import javascript external\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, refs("external"), label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("ignored import produced deps = %v", r.AttrStrings("deps"))
	}
	if len(l.Errors()) != 0 {
		t.Errorf("ignored import errors = %v", l.Errors())
	}
	l.AfterResolvingDeps(context.Background())
}

func TestResolveMappingIgnoreConflict(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "javascript javascript mapped //mapped:dep"}})
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import javascript mapped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, refs("mapped"), label.New("", "app", "app"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "both an exact resolve mapping and ignore") {
		t.Errorf("conflict errors = %v", l.Errors())
	}
}

func TestConfigureSkipsOtherDirectives(t *testing.T) {
	l := newLang()
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
	if len(l.Errors()) != 0 {
		t.Errorf("other-directive errors = %v", l.Errors())
	}
}

func TestConfigureThreeFieldAndMalformed(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import javascript mydep\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	ignore := common.MatchingIgnore(cfg, languageName, "mydep")
	if ignore == nil || ignore.Value != "mydep" {
		t.Fatalf("three-field ignore = %+v, want mydep", ignore)
	}
	ignore.MarkUsed("pkg")

	malformed := newLang()
	malformed.Before(context.Background())
	badCfg := config.New()
	badFile, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import javascript\n"))
	if err != nil {
		t.Fatal(err)
	}
	malformed.Configure(badCfg, "pkg", badFile)
	if len(malformed.Errors()) != 1 || !strings.Contains(malformed.Errors()[0], "malformed") {
		t.Errorf("malformed errors = %v", malformed.Errors())
	}
	l.AfterResolvingDeps(context.Background())
}

func TestMatchingIgnoreMiss(t *testing.T) {
	l := newLang()
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

func TestRepeatedBasenamesKeepSourceIdentity(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/main.js": "import { a } from \"../a/util\";\nimport { b } from \"../b/util.js\";\n",
	}, []string{"main.js"})
	imports := result.Imports[0]
	if got := strings.Join(common.RefRoots(importRefs(imports)), ","); got != "pkg/a/util,pkg/b/util" {
		t.Fatalf("imports = %q, want pkg/a/util,pkg/b/util", got)
	}
	l := newLang()
	index := resolverIndex(l,
		struct {
			pkg, name string
			ext       string
		}{"pkg/a", "util", ".js"},
		struct {
			pkg, name string
			ext       string
		}{"pkg/b", "util", ".js"},
	)
	r := rule.NewRule(libraryKind, "main")
	l.Resolve(resolverConfig(t, nil), index, nil, r, imports, label.New("", "pkg/demo", "main"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//pkg/a:util,//pkg/b:util" {
		t.Errorf("deps = %q, want //pkg/a:util,//pkg/b:util", got)
	}
	if len(l.Errors()) != 0 {
		t.Errorf("errors = %v, want none", l.Errors())
	}

	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "javascript javascript pkg/a/util //mapped:a"}})
	r = rule.NewRule(libraryKind, "main")
	l.Resolve(cfg, index, nil, r, imports, label.New("", "pkg/demo", "main"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:a,//pkg/b:util" {
		t.Errorf("override deps = %q, want //mapped:a,//pkg/b:util", got)
	}
}

func TestResolveIndexDottedAndAssets(t *testing.T) {
	files := map[string]string{
		"pkg/demo/app.js": "import c from \"./components\";\nimport p from \"./config.prod\";\nimport d from \"./data.json\";\nimport s from \"./styles.module.css\";\nimport x from \"./x\";\n",
	}
	result := generateFixture(t, files, []string{"app.js"})
	if len(result.Imports) == 0 {
		t.Fatal("no imports generated")
	}
	imports := result.Imports[0]
	l := newLang()
	index := resolverIndex(l,
		struct{ pkg, name, ext string }{"pkg/demo/components", "index", ""},
		struct{ pkg, name, ext string }{"pkg/demo", "config.prod", ""},
		struct{ pkg, name, ext string }{"pkg/demo", "config", ""},
		struct{ pkg, name, ext string }{"pkg/demo", "x", ""},
		struct{ pkg, name, ext string }{"pkg/demo/x", "index", ""},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, index, nil, r, imports, label.New("", "pkg/demo", "app"))
	if len(l.Errors()) != 0 {
		t.Fatalf("errors = %v", l.Errors())
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//pkg/demo/components:index,:config.prod,:x" {
		t.Errorf("deps = %q", got)
	}

	owned := rule.NewRule(libraryKind, "data")
	owned.SetAttr("srcs", []string{"data.json"})
	specs := l.Imports(cfg, owned, rule.EmptyFile("pkg/demo/BUILD.bazel", "pkg/demo"))
	if len(specs) != 1 || specs[0].Imp != "pkg/demo/data.json" {
		t.Fatalf("asset imports = %v", specs)
	}
	handIndex := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return l })
	handIndex.AddRule(config.New(), owned, rule.EmptyFile("pkg/demo/BUILD.bazel", "pkg/demo"))
	handIndex.Finish()
	r = rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, handIndex, nil, r, common.WrapRefImportSet([]common.ImportRef{{Root: "pkg/demo/data.json", Relative: true, Ext: ".json"}}), label.New("", "pkg/demo", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != ":data" {
		t.Errorf("owned asset deps = %q", got)
	}

	missing := rule.NewRule(libraryKind, "missing")
	l.Resolve(cfg, index, nil, missing, common.WrapRefImportSet([]common.ImportRef{{Root: "pkg/demo/nope", Relative: true}}), label.New("", "pkg/demo", "missing"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "unresolved import") {
		t.Errorf("missing errors = %v", l.Errors())
	}
}

func TestGenerateTestPackageJSONFollowsRootManifest(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"package.json":            "{\"type\": \"module\", \"devDependencies\": {\"jest-cli\": \"1\", \"jest-junit\": \"1\"}}\n",
		"pkg/demo/helper.js":      "export const helper = 1;\n",
		"pkg/demo/helper_test.js": "import { helper } from \"./helper\";\n",
	}, []string{"helper.js", "helper_test.js"})
	var test *rule.Rule
	for _, r := range result.Gen {
		if r.Kind() == testKind {
			test = r
		}
	}
	if test == nil {
		t.Fatalf("missing %s in %v", testKind, result.Gen)
	}
	if got := test.AttrString("package_json"); got != rootPackageJSON {
		t.Errorf("test package_json = %q, want %q", got, rootPackageJSON)
	}
}

func TestMergeKeepsHandSetPackageJSON(t *testing.T) {
	info := testKindInfo()
	if info.MergeableAttrs["package_json"] {
		t.Fatal("package_json must not be mergeable, or Gazelle overwrites a hand-set nearest manifest")
	}
	gen := rule.NewRule(testKind, "helper_test")
	gen.SetAttr("package_json", rootPackageJSON)
	hand := rule.NewRule(testKind, "helper_test")
	hand.SetAttr("package_json", "//pkg:package_json")
	rule.MergeRules(gen, hand, info.MergeableAttrs, "BUILD.bazel")
	if got := hand.AttrString("package_json"); got != "//pkg:package_json" {
		t.Errorf("merged package_json = %q, want the hand-set //pkg:package_json", got)
	}
	missing := rule.NewRule(testKind, "helper_test")
	rule.MergeRules(gen, missing, info.MergeableAttrs, "BUILD.bazel")
	if got := missing.AttrString("package_json"); got != rootPackageJSON {
		t.Errorf("merged package_json = %q, want %q added to a rule without one", got, rootPackageJSON)
	}
}

func TestResolveTSConfigPaths(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"CONFIG":     `{"extends": "./CONFIGBASE", "compilerOptions": {"baseUrl": "."}}`,
		"CONFIGBASE": `{"compilerOptions": {"paths": {"@/*": ["./*"], "~utils": ["./lib/util.js"], "fs": ["./lib/util.js"]}}}`,
		"bad/CONFIG": `{`,
	} {
		name = strings.NewReplacer("CONFIGBASE", "jsconfig.base.json", "CONFIG", "jsconfig.json").Replace(name)
		p := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(strings.ReplaceAll(content, "CONFIGBASE", "jsconfig.base.json")), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	l := newLang()
	cfg := resolverConfig(t, nil)
	cfg.RepoRoot = root
	l.Configure(cfg, "", nil)
	app := cfg.Clone()
	l.Configure(app, "app", nil)
	if len(l.Errors()) != 0 {
		t.Fatalf("configure errors = %v", l.Errors())
	}
	type entry = struct {
		pkg, name string
		ext       string
	}
	index := resolverIndex(l, entry{"lib", "util", ""}, entry{"lib/components", "index", ""})
	for _, tc := range []struct{ name, want string }{
		{"@/lib/util", "//lib:util"},
		{"~utils", "//lib:util"},
		{"lib/util", "//lib:util"},
		{"@/components", ""},
		{"@/lib/components", "//lib/components:index"},
		{"fs", ""},
	} {
		r := rule.NewRule(libraryKind, "main")
		l.Resolve(app, index, nil, r, refs(tc.name), label.New("", "app", "main"))
		if got := strings.Join(r.AttrStrings("deps"), ","); got != tc.want {
			t.Errorf("%q deps = %q, want %q", tc.name, got, tc.want)
		}
	}
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], `unresolved import "@/components"`) {
		t.Errorf("errors = %v", l.Errors())
	}
	l.Before(context.Background())
	l.Resolve(app, index, nil, rule.NewRule(libraryKind, "main"), refs("react"), label.New("", "app", "main"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "tsconfig/jsconfig paths alias") {
		t.Errorf("react errors = %v", l.Errors())
	}
	l.Before(context.Background())
	l.Configure(app.Clone(), "bad", nil)
	if len(l.Errors()) != 1 || !strings.HasPrefix(l.Errors()[0], "tsconfig: //bad/jsconfig.json: ") {
		t.Errorf("malformed errors = %v", l.Errors())
	}
}

// refs builds a RefImportSet whose roots are bare imports, or relative when
// they start with "./".
func refs(roots ...string) any {
	out := make([]common.ImportRef, 0, len(roots))
	for _, root := range roots {
		out = append(out, common.ImportRef{Root: root})
	}
	return common.WrapRefImportSet(out)
}

func importRefs(raw any) []common.ImportRef {
	out, _ := common.UnwrapRefImportSet(raw)
	return out
}

func TestResolveUnownedAssetSkippedSilently(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapRefImportSet([]common.ImportRef{{Root: "pkg/app/data.json", Relative: true, Ext: ".json"}}), label.New("", "pkg/app", "app"))
	if len(l.Errors()) != 0 || r.Attr("deps") != nil {
		t.Errorf("unowned asset: errors = %v, deps = %v, want none", l.Errors(), r.AttrStrings("deps"))
	}
}

func TestResolveAmbiguousIncludingSelfIsSkipped(t *testing.T) {
	l := newLang()
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return l })
	for _, entry := range [][2]string{{"x", "same.js"}, {"y", "same.jsx"}} {
		r := rule.NewRule(libraryKind, entry[0])
		r.SetAttr("srcs", []string{entry[1]})
		index.AddRule(config.New(), r, rule.EmptyFile("one/BUILD.bazel", "one"))
	}
	index.Finish()
	r := rule.NewRule(libraryKind, "x")
	l.Resolve(resolverConfig(t, nil), index, nil, r, refs("one/same"), label.New("", "one", "x"))
	if len(l.Errors()) != 0 || r.Attr("deps") != nil {
		t.Errorf("self-including ambiguity: errors = %v, deps = %v, want silent skip", l.Errors(), r.AttrStrings("deps"))
	}
}

func nodeRunnerTest(name string) *rule.Rule {
	r := rule.NewRule(testKind, name)
	r.SetAttr("runner", "node")
	return r
}

func TestGenerateKeepsNodeRunnerTestWithoutSource(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/other_test.js", "export {};\n")
	file := rule.EmptyFile("BUILD.bazel", "pkg/demo")
	file.Rules = append(file.Rules, nodeRunnerTest("gone_test"))
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"other_test.js"},
		File:         file,
	})
	for _, r := range result.Empty {
		if r.Name() == "gone_test" {
			t.Fatalf("empty = %v, want gone_test kept", result.Empty)
		}
	}
}

func TestGenerateLinksNpmPackagesAtLockDirectory(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n")
	lang := NewLanguage()
	result := lang.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != "npm_link_all_packages" || result.Gen[0].Name() != "node_modules" {
		t.Fatalf("generated %v, want npm_link_all_packages(node_modules)", result.Gen)
	}
	if len(result.Imports) != 1 {
		t.Errorf("imports = %d, want 1", len(result.Imports))
	}
	loads := lang.(*common.SingleLang).Loads()
	found := false
	for _, l := range loads {
		if l.Name == "@npm//:defs.bzl" && len(l.Symbols) == 1 && l.Symbols[0] == "npm_link_all_packages" {
			found = true
		}
	}
	if !found {
		t.Errorf("loads = %+v, want @npm//:defs.bzl", loads)
	}
	other := lang.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, "x"), Rel: "x"})
	if len(other.Gen) != 0 {
		t.Errorf("package without a lock generated %v", other.Gen)
	}
}

func TestGenerateLinksNpmPackagesInWorkspaceMembers(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n")
	writeFixture(t, root, "pnpm-workspace.yaml", "packages:\n  - 'apps/*'\n  - '!apps/skip'\n")
	for _, d := range []string{"apps/web", "apps/skip", "libs/x"} {
		writeFixture(t, root, d+"/package.json", "{}\n")
	}
	lang := NewLanguage()
	for rel, want := range map[string]int{"apps/web": 2, "apps/skip": 0, "libs/x": 0} {
		result := lang.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, rel), Rel: rel})
		if len(result.Gen) != want {
			t.Errorf("%s generated %v, want %d rules", rel, result.Gen, want)
		}
	}
}

func TestGenerateJestNeedsRootDevDependencies(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "package.json", "{}\n")
	writeFixture(t, root, "pkg/demo/helper_test.js", "export {};\n")
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, "pkg", "demo"), Rel: "pkg/demo", RegularFiles: []string{"helper_test.js"}})
	if len(result.Gen) != 0 {
		t.Errorf("generated %v despite the missing jest packages", result.Gen)
	}
	if errs := l.Errors(); len(errs) != 1 || !strings.Contains(errs[0], `add "jest-cli" and "jest-junit" to devDependencies in package.json`) {
		t.Errorf("errors = %q, want the jest devDependencies remedy", errs)
	}
}

func TestGenerateRootPackageJSONLibrary(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "package.json", "{}\n")
	result := newLang().GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, Rel: ""})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != jsLibraryKind || result.Gen[0].Name() != packageJSONName {
		t.Fatalf("generated %v, want js_library(package_json)", result.Gen)
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "package.json" {
		t.Errorf("srcs = %q", got)
	}
	nested := newLang().GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, "x"), Rel: "x"})
	if len(nested.Gen) != 0 {
		t.Errorf("nested package generated %v", nested.Gen)
	}
}

func TestGenerateWorkspacePackage(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n")
	writeFixture(t, root, "pnpm-workspace.yaml", "packages:\n  - 'libs/*'\n")
	writeFixture(t, root, "libs/a/package.json", "{}\n")
	writeFixture(t, root, "libs/a/index.js", "export const a = 1;\n")
	writeFixture(t, root, "libs/b/package.json", "{}\n")
	writeFixture(t, root, "libs/b/index.ts", "export const b = 1;\n")
	pkgOf := func(rel string, regular ...string) *rule.Rule {
		result := newLang().GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, rel), Rel: rel, RegularFiles: regular})
		for _, r := range result.Gen {
			if r.Kind() == packageKind {
				return r
			}
		}
		return nil
	}
	a := pkgOf("libs/a", "package.json", "index.js")
	if a == nil || a.Name() != packageName || strings.Join(a.AttrStrings("srcs"), ",") != "package.json,:index" {
		t.Errorf("libs/a package = %v", a)
	}
	if b := pkgOf("libs/b", "package.json", "index.ts"); b != nil {
		t.Errorf("libs/b has TypeScript sources, so TypeScript owns its package; got %v", b)
	}
}

func TestMissingPackageHintNamesOwningManifest(t *testing.T) {
	spec := newLang().ResolveSpecFor(&config.Config{RepoRoot: "/nonexistent"}, rule.NewRule(libraryKind, "x"), label.New("", "pkg", "x"))
	if spec.MissingExternalHint == nil {
		t.Fatal("javascript must name the package.json to add an undeclared import to")
	}
}
