package typescript

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
	"github.com/devhusdk/rules_dx/gazelle/common"
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
	regular := []string{"demo.ts", "helper.tsx", "helper_test.mts", "notes.txt", "widget.d.ts", "orphan.d.mts"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.ts":         "import helper from \"./helper.tsx\";\nimport fs from \"fs\";\n",
		"pkg/demo/helper.tsx":      "export const suffix = (t: string) => t;\n",
		"pkg/demo/helper_test.mts": "import helper from \"./helper.tsx\";\n",
		"pkg/demo/notes.txt":       "not a source\n",
		"pkg/demo/widget.d.ts":     "export declare const x: number;\n",
		"pkg/demo/orphan.d.mts":    "export declare const y: number;\n",
	}, regular)
	if len(result.Gen) != 5 || len(result.Imports) != 5 {
		t.Fatalf("generated %d rules and %d import sets, want 5 each", len(result.Gen), len(result.Imports))
	}
	for i, want := range []string{"orphan", "widget"} {
		decl := result.Gen[3+i]
		if decl.Kind() != projectKind || decl.Name() != want || len(decl.AttrStrings("srcs")) != 0 {
			t.Errorf("declaration rule %d = %s(%s) srcs %v", i, decl.Kind(), decl.Name(), decl.AttrStrings("srcs"))
		}
	}
	if got := strings.Join(result.Gen[3].AttrStrings("declaration_srcs"), ","); got != "orphan.d.mts" {
		t.Errorf("orphan declaration_srcs = %q", got)
	}
	if got := strings.Join(result.Gen[4].AttrStrings("declaration_srcs"), ","); got != "widget.d.ts" {
		t.Errorf("widget declaration_srcs = %q", got)
	}
	demo := result.Gen[0]
	if demo.Kind() != projectKind || demo.Name() != "demo" {
		t.Fatalf("demo = %s(%s)", demo.Kind(), demo.Name())
	}
	if got := strings.Join(demo.AttrStrings("srcs"), ","); got != "demo.ts" {
		t.Errorf("demo srcs = %q", got)
	}
	helper := result.Gen[1]
	if helper.Kind() != projectKind || helper.Name() != "helper" {
		t.Fatalf("helper = %s(%s)", helper.Kind(), helper.Name())
	}
	test := result.Gen[2]
	if test.Kind() != testKind || test.Name() != "helper_test" {
		t.Fatalf("test = %s(%s)", test.Kind(), test.Name())
	}
	if got := strings.Join(test.AttrStrings("srcs"), ","); got != "helper_test.mts" {
		t.Errorf("test srcs = %q", got)
	}
	if got := test.AttrString("node_modules"); got != rootNodeModules {
		t.Errorf("test node_modules = %q, want %q", got, rootNodeModules)
	}
	testImports := result.Imports[2].(targetImports)
	if strings.Join(testImports.imports, ",") != "pkg/demo/helper" {
		t.Errorf("test imports = %+v, want [pkg/demo/helper]", testImports)
	}
	demoImports := result.Imports[0].(targetImports)
	if strings.Join(demoImports.imports, ",") != "pkg/demo/helper" {
		t.Errorf("demo imports = %+v", demoImports)
	}
	helperImports := result.Imports[1].(targetImports)
	if len(helperImports.imports) != 0 {
		t.Errorf("helper imports = %+v, want empty", helperImports)
	}
}

func TestGenerateRelativeStdlibCollision(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/util.ts": "export const add = (a: number, b: number) => a + b;\n",
		"pkg/demo/app.ts":  "import { add } from \"./util.js\";\nimport fs from \"fs\";\nexport const total = add(1, 2);\n",
	}, []string{"app.ts", "util.ts"})
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	appImports := result.Imports[0].(targetImports)
	if strings.Join(appImports.imports, ",") != "pkg/demo/util" {
		t.Fatalf("app imports = %+v, want [util]", appImports)
	}
	if !appImports.local["pkg/demo/util"] {
		t.Fatalf("app imports = %+v, want util marked relative", appImports)
	}
	l := &typescriptLang{}
	index := resolverIndex(l,
		struct {
			pkg, name string
			ext       string
		}{"pkg/demo", "util", ".ts"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(projectKind, "app")
	l.Resolve(cfg, index, nil, r, appImports, label.New("", "pkg/demo", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != ":util" {
		t.Errorf("resolved deps = %q, want %q", got, ":util")
	}
	if len(l.errors) != 0 {
		t.Errorf("resolve errors = %v, want none", l.errors)
	}
}

func TestGenerateEmptySweepsStale(t *testing.T) {
	result := generateFixture(t, nil, []string{"notes.txt", "orphan.d.ts"})
	if len(result.Gen) != 0 || len(result.Empty) != 0 {
		t.Fatalf("empty generation = %+v", result)
	}
	f := rule.EmptyFile("BUILD.bazel", "pkg/demo")
	f.Rules = append(f.Rules,
		rule.NewRule(projectKind, "old"),
		rule.NewRule("filegroup", "keep"),
		rule.NewRule(projectKind, "demo"),
	)
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/demo.ts", "export const x = 1;\n")
	dir := filepath.Join(root, "pkg", "demo")
	result = NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.ts"},
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
		{"readError", map[string]string{}, []string{"missing.ts"}, "read missing.ts"},
		{"emptyName", map[string]string{"pkg/demo/---.ts": "export const x = 1;\n"}, []string{"---.ts"}, "empty target name"},
		{"collision", map[string]string{
			"pkg/demo/a-b.ts":  "export const x = 1;\n",
			"pkg/demo/a_b.tsx": "export const y = 2;\n",
		}, []string{"a-b.ts", "a_b.tsx"}, "claimed by a-b.ts, a_b.tsx"},
	}
	for _, tc := range cases {
		root := t.TempDir()
		for name, content := range tc.files {
			writeFixture(t, root, name, content)
		}
		l := &typescriptLang{}
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

func TestGenerateEntryThinBinary(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/main.ts":        "import helper from \"./helper\";\nconsole.log(helper(\"x\"));\n",
		"pkg/demo/helper.ts":      "export function helper(name: string): string { return name; }\n",
		"pkg/demo/helper_test.ts": "import helper from \"./helper\";\n",
	}, []string{"main.ts", "helper.ts", "helper_test.ts"})
	if len(result.Gen) != 4 || len(result.Imports) != 4 {
		t.Fatalf("generated %d rules and %d import sets, want 4 each", len(result.Gen), len(result.Imports))
	}
	byRule := make(map[string]int, len(result.Gen))
	for i, r := range result.Gen {
		byRule[r.Kind()+"\x00"+r.Name()] = i
	}
	libIdx, ok := byRule[projectKind+"\x00main"]
	if !ok {
		t.Fatalf("missing typescript_project(main) in %v", result.Gen)
	}
	binIdx, ok := byRule[binaryKind+"\x00main_bin"]
	if !ok {
		t.Fatalf("missing javascript_binary(main_bin) in %v", result.Gen)
	}
	testIdx, ok := byRule[testKind+"\x00helper_test"]
	if !ok {
		t.Fatalf("missing typescript_test(helper_test) in %v", result.Gen)
	}
	if got := result.Gen[testIdx].AttrString("node_modules"); got != rootNodeModules {
		t.Errorf("test node_modules = %q, want %q", got, rootNodeModules)
	}
	if got := result.Gen[testIdx].Attr("package_json"); got != nil {
		t.Errorf("test package_json = %v, want unset without a root package.json", got)
	}
	lib := result.Gen[libIdx]
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "main.ts" {
		t.Errorf("library srcs = %q, want main.ts", got)
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
	if raw := result.Imports[libIdx].(targetImports); strings.Join(raw.imports, ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper]", raw)
	}
	if raw := result.Imports[binIdx].(targetImports); len(raw.imports) != 0 {
		t.Errorf("binary imports = %+v, want empty", raw)
	}
}

func TestGenerateEntryCompiledExtensions(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/main.mts": "export const x = 1;\n",
	}, []string{"main.mts"})
	if len(result.Gen) != 2 {
		t.Fatalf("generated %d rules, want 2", len(result.Gen))
	}
	byRule := make(map[string]int, len(result.Gen))
	for i, r := range result.Gen {
		byRule[r.Kind()+"\x00"+r.Name()] = i
	}
	binIdx, ok := byRule[binaryKind+"\x00main_bin"]
	if !ok {
		t.Fatalf("missing javascript_binary(main_bin) in %v", result.Gen)
	}
	if got := result.Gen[binIdx].AttrString("entry_point"); got != "main.mjs" {
		t.Errorf("binary entry_point = %q, want main.mjs", got)
	}
}

func TestGenerateEntryCollision(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/main.ts":     "export const x = 1;\n",
		"pkg/demo/main_bin.ts": "export const y = 2;\n",
	} {
		writeFixture(t, root, name, content)
	}
	l := &typescriptLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"main.ts", "main_bin.ts"},
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
	l := &typescriptLang{}
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
	keptLib := rule.NewRule(projectKind, "main")
	keptBin := rule.NewRule(binaryKind, "main_bin")
	result := mergeStale(f, language.GenerateResult{Gen: []*rule.Rule{keptLib, keptBin}})
	if len(result.Empty) != 1 || result.Empty[0].Name() != "old_bin" {
		t.Fatalf("stale = %v, want [old_bin]", result.Empty)
	}
}

func TestImportsIndexesNonTestOnly(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(projectKind, "demo")
	lib.SetAttr("srcs", []string{"demo.ts", "notes.txt"})
	imports := lang.Imports(&config.Config{}, lib, nil)
	if len(imports) != 2 || imports[0].Lang != languageName || imports[0].Imp != "demo" || imports[1].Imp != "notes.txt" {
		t.Errorf("imports = %+v", imports)
	}
	testRule := rule.NewRule(projectKind, "demo_test")
	testRule.SetAttr("srcs", []string{"demo_test.ts"})
	if got := lang.Imports(&config.Config{}, testRule, nil); got != nil {
		t.Errorf("test imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(testKind, "demo_test"), nil); got != nil {
		t.Errorf("test-kind imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(projectKind, "empty"), nil); got != nil {
		t.Errorf("srcless imports = %+v, want nil", got)
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := &typescriptLang{}
	if l.Name() != "typescript" || len(l.Kinds()) != 3 || l.CheckFlags(flag.NewFlagSet("test", flag.ContinueOnError), config.New()) != nil {
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
	if len(loads) != 2 || loads[0].Name != "@renamed_dx//typescript/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "typescript_project,typescript_test" {
		t.Errorf("apparent loads = %+v", loads)
	}
	if loads[1].Name != "@renamed_dx//javascript/rules:defs.bzl" || strings.Join(loads[1].Symbols, ",") != "javascript_binary" {
		t.Errorf("apparent binary loads = %+v", loads)
	}
	if defaults := l.Loads(); len(defaults) != 2 || defaults[0].Name != "@rules_dx//typescript/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
	if defaults := l.Loads(); defaults[1].Name != "@rules_dx//javascript/rules:defs.bzl" || strings.Join(defaults[1].Symbols, ",") != "javascript_binary" {
		t.Errorf("default binary loads = %+v", defaults)
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

func resolverIndex(lang *typescriptLang, entries ...struct {
	pkg  string
	name string
	ext  string
}) *resolve.RuleIndex {
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := typescriptKinds[r.Kind()]; ok {
			return lang
		}
		return nil
	})
	for _, entry := range entries {
		ext := entry.ext
		if ext == "" {
			ext = ".ts"
		}
		r := rule.NewRule(projectKind, entry.name)
		r.SetAttr("srcs", []string{entry.name + ext})
		if IsTestFile(entry.name + ext) {
			continue
		}
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry.pkg, "BUILD.bazel"), entry.pkg))
	}
	index.Finish()
	return index
}

func TestResolveBranches(t *testing.T) {
	l := &typescriptLang{}
	index := resolverIndex(l,
		struct {
			pkg, name string
			ext       string
		}{"lib/b", "b", ".ts"},
		struct {
			pkg, name string
			ext       string
		}{"lib/a", "a", ".tsx"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(projectKind, "app")
	l.Resolve(cfg, index, nil, r, targetImports{imports: []string{"lib/b/b", "lib/a/a"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a,//lib/b" {
		t.Errorf("resolved deps = %q", got)
	}

	self := rule.NewRule(projectKind, "a")
	self.SetAttr("srcs", []string{"a.ts"})
	l.Resolve(cfg, index, nil, self, targetImports{imports: []string{"lib/a/a", "lib/a/a"}}, label.New("", "lib/a", "a"))
	_ = self

	std := rule.NewRule(projectKind, "uses_std")
	l.Resolve(cfg, index, nil, std, targetImports{imports: []string{"fs"}}, label.New("", "app", "uses_std"))
	if std.Attr("deps") != nil || len(l.errors) != 0 {
		t.Errorf("stdlib resolution = %v, errors = %v", std.AttrStrings("deps"), l.errors)
	}

	hand := rule.NewRule(projectKind, "hand")
	hand.SetAttr("deps", []string{":kept"})
	l.Resolve(cfg, index, nil, hand, targetImports{imports: []string{"lib/a/a"}}, label.New("", "app", "hand"))
	if got := strings.Join(hand.AttrStrings("deps"), ","); got != "//lib/a,:kept" {
		t.Errorf("merged deps = %q", got)
	}

	l.Resolve(cfg, index, nil, rule.NewRule(projectKind, "unknown"), targetImports{imports: []string{"unknown"}}, label.New("", "app", "unknown"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("unresolved errors = %v", l.errors)
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}

func TestResolveOverride(t *testing.T) {
	l := &typescriptLang{}
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "typescript typescript mapped //mapped:dep"}})
	r := rule.NewRule(projectKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"mapped"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
}

func TestResolveAmbiguous(t *testing.T) {
	l := &typescriptLang{}
	cfg := resolverConfig(t, nil)
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return l })
	for _, entry := range [][2]string{{"x", "same.ts"}, {"y", "same.tsx"}} {
		r := rule.NewRule(projectKind, entry[0])
		r.SetAttr("srcs", []string{entry[1]})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join("one", "BUILD.bazel"), "one"))
	}
	index.Finish()
	l.Resolve(cfg, index, nil, rule.NewRule(projectKind, "app"), targetImports{imports: []string{"one/same"}}, label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "ambiguous") {
		t.Errorf("ambiguity = %v", l.errors)
	}
}

func TestUnionStrings(t *testing.T) {
	if got := unionStrings([]string{"b", "a", "a"}, []string{"c", "a"}); strings.Join(got, ",") != "a,b,c" {
		t.Errorf("union = %q", got)
	}
}

func TestErrorsAbortBeforeEmission(t *testing.T) {
	l := &typescriptLang{}
	l.Before(context.Background())
	l.fail("second")
	l.fail("first")
	l.AfterResolvingDeps(context.Background())
	if message := strings.Join(common.Messages(common.Errors.Take()), "\n"); message != "first\nsecond" {
		t.Errorf("errors were not deterministic: %s", message)
	}
}

func TestResolveIgnore(t *testing.T) {
	l := &typescriptLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import typescript external\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(projectKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"external"}}, label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("ignored import produced deps = %v", r.AttrStrings("deps"))
	}
	l.AfterResolvingDeps(context.Background())
}

func TestStaleIgnoreFails(t *testing.T) {
	l := &typescriptLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import typescript stale\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	l.generated.Add("pkg")
	l.AfterResolvingDeps(context.Background())
	if got := strings.Join(common.Messages(common.Errors.Take()), "\n"); !strings.Contains(got, "stale") {
		t.Errorf("stale ignore result = %q", got)
	}
}

func TestConfigureInheritanceAndOtherDirectives(t *testing.T) {
	l := &typescriptLang{}
	l.Before(context.Background())
	root := config.New()
	file, err := rule.LoadData("BUILD.bazel", "", []byte("# gazelle:dx_ignore_import typescript missing_dep\n# gazelle:resolve typescript typescript foo //foo:bar\n# gazelle:dx_ignore_import python foo\n"))
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
	if got := common.MatchingIgnore(child, languageName, "absent"); got != nil {
		t.Errorf("ignore miss = %+v, want nil", got)
	}
	if got := common.MatchingIgnore(config.New(), languageName, "absent"); got != nil {
		t.Errorf("no-exts ignore = %+v, want nil", got)
	}
	l.AfterResolvingDeps(context.Background())
}

func TestConfigureThreeFieldAndMalformed(t *testing.T) {
	l := &typescriptLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import typescript typescript mydep\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	ignore := common.MatchingIgnore(cfg, languageName, "mydep")
	if ignore == nil || ignore.Value != "mydep" {
		t.Fatalf("three-field ignore = %+v, want mydep", ignore)
	}
	ignore.MarkUsed("pkg")
	l.AfterResolvingDeps(context.Background())

	malformed := &typescriptLang{}
	malformed.Before(context.Background())
	badCfg := config.New()
	badFile, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import typescript\n"))
	if err != nil {
		t.Fatal(err)
	}
	malformed.Configure(badCfg, "pkg", badFile)
	if len(malformed.errors) != 1 || !strings.Contains(malformed.errors[0], "malformed") {
		t.Errorf("malformed errors = %v", malformed.errors)
	}
}

func TestApparentLoadsDefault(t *testing.T) {
	l := &typescriptLang{}
	got := l.ApparentLoads(func(string) string { return "" })
	if len(got) != 2 || got[0].Name != "@rules_dx//typescript/rules:defs.bzl" {
		t.Errorf("default apparent loads = %+v", got)
	}
	if got[1].Name != "@rules_dx//javascript/rules:defs.bzl" || strings.Join(got[1].Symbols, ",") != "javascript_binary" {
		t.Errorf("default apparent binary loads = %+v", got)
	}
}

func TestImportsNonProjectKind(t *testing.T) {
	lang := NewLanguage()
	if got := lang.Imports(&config.Config{}, rule.NewRule("filegroup", "x"), nil); got != nil {
		t.Errorf("non-project imports = %+v, want nil", got)
	}
}

func TestClaimKindFallback(t *testing.T) {
	if got := claimKind(Claimant{Name: "x", Source: "x.ts"}); got != projectKind {
		t.Errorf("fallback kind = %q, want %q", got, projectKind)
	}
	if got := claimKind(Claimant{Name: "a_test", Source: "a_test.ts"}); got != testKind {
		t.Errorf("test kind = %q, want %q", got, testKind)
	}
	if got := claimKind(Claimant{Name: "x", Source: "x.ts", Kind: projectKind}); got != projectKind {
		t.Errorf("explicit kind = %q, want %q", got, projectKind)
	}
	if got := claimKind(Claimant{Name: "x_bin", Source: "x.ts", Kind: binaryKind}); got != binaryKind {
		t.Errorf("binary kind = %q, want %q", got, binaryKind)
	}
}

func TestCheckClaimsOtherAndMismatch(t *testing.T) {
	claimants := []Claimant{{Name: "demo", Source: "demo.ts", Kind: projectKind}}
	otherSame := []*rule.Rule{rule.NewRule(projectKind, "demo")}
	if err := checkClaims(nil, otherSame, claimants); err != nil {
		t.Errorf("same-kind other claims = %v", err)
	}
	otherWrong := []*rule.Rule{rule.NewRule("filegroup", "demo")}
	if err := checkClaims(nil, otherWrong, claimants); err == nil || !strings.Contains(err.Error(), "existing filegroup") {
		t.Errorf("other kind mismatch = %v", err)
	}
	wrongKind := rule.EmptyFile("BUILD.bazel", "pkg")
	wrongKind.Rules = append(wrongKind.Rules, rule.NewRule("filegroup", "demo"))
	if err := checkClaims(wrongKind, nil, claimants); err == nil || !strings.Contains(err.Error(), "existing filegroup") {
		t.Errorf("kind mismatch = %v", err)
	}
	dupes := []Claimant{{Name: "a_b", Source: "a-b.ts", Kind: projectKind}, {Name: "a_b", Source: "a_b.tsx", Kind: projectKind}}
	existing := rule.EmptyFile("BUILD.bazel", "pkg")
	existing.Rules = append(existing.Rules, rule.NewRule("filegroup", "a_b"))
	if err := checkClaims(existing, nil, dupes); err == nil || !strings.Contains(err.Error(), "handwritten") {
		t.Errorf("collision with handwritten = %v", err)
	}
	plainClaimants := []Claimant{{Name: "demo", Source: "demo_test.ts", Kind: testKind}}
	plainFile := rule.EmptyFile("BUILD.bazel", "pkg")
	plainFile.Rules = append(plainFile.Rules, rule.NewRule(plainTestKind, "demo"))
	if err := checkClaims(plainFile, nil, plainClaimants); err != nil {
		t.Errorf("plain-runner handwritten claims = %v", err)
	}
	plainOther := []*rule.Rule{rule.NewRule(plainTestKind, "demo")}
	if err := checkClaims(nil, plainOther, plainClaimants); err != nil {
		t.Errorf("plain-runner other claims = %v", err)
	}
}

func TestGeneratePlainTestKeepsHandwrittenRunner(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/plain_test.ts", "import { test } from \"node:test\";\ntest(\"ok\", () => {});\n")
	file := rule.EmptyFile("BUILD.bazel", "pkg/demo")
	file.Rules = append(file.Rules, rule.NewRule(plainTestKind, "plain_test"))
	l := &typescriptLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"plain_test.ts"},
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

func TestUnionStringsEmpty(t *testing.T) {
	if got := unionStrings(nil, nil); len(got) != 0 {
		t.Errorf("empty union = %q", got)
	}
}

func TestResolveMappingIgnoreConflict(t *testing.T) {
	l := &typescriptLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "typescript typescript mapped //mapped:dep"}})
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import typescript mapped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(projectKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"mapped"}}, label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "both an exact resolve mapping and ignore") {
		t.Errorf("conflict errors = %v", l.errors)
	}
}

func TestRepeatedBasenamesKeepSourceIdentity(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/main.ts": "import { a } from \"../a/util\";\nimport { b } from \"../b/util.ts\";\n",
	}, []string{"main.ts"})
	imports := result.Imports[0].(targetImports)
	if got := strings.Join(imports.imports, ","); got != "pkg/a/util,pkg/b/util" {
		t.Fatalf("imports = %q, want pkg/a/util,pkg/b/util", got)
	}
	l := &typescriptLang{}
	index := resolverIndex(l,
		struct {
			pkg, name string
			ext       string
		}{"pkg/a", "util", ".ts"},
		struct {
			pkg, name string
			ext       string
		}{"pkg/b", "util", ".ts"},
	)
	r := rule.NewRule(projectKind, "main")
	l.Resolve(resolverConfig(t, nil), index, nil, r, imports, label.New("", "pkg/demo", "main"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//pkg/a:util,//pkg/b:util" {
		t.Errorf("deps = %q, want //pkg/a:util,//pkg/b:util", got)
	}
	if len(l.errors) != 0 {
		t.Errorf("errors = %v, want none", l.errors)
	}

	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "typescript typescript pkg/a/util //mapped:a"}})
	r = rule.NewRule(projectKind, "main")
	l.Resolve(cfg, index, nil, r, imports, label.New("", "pkg/demo", "main"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:a,//pkg/b:util" {
		t.Errorf("override deps = %q, want //mapped:a,//pkg/b:util", got)
	}
}

func TestResolveIndexDottedAndAssets(t *testing.T) {
	files := map[string]string{
		"pkg/demo/app.ts": "import c from \"./components\";\nimport p from \"./config.prod\";\nimport d from \"./data.json\";\nimport s from \"./styles.module.css\";\nimport x from \"./x\";\n",
	}
	result := generateFixture(t, files, []string{"app.ts"})
	if len(result.Imports) == 0 {
		t.Fatal("no imports generated")
	}
	imports, ok := result.Imports[0].(targetImports)
	if !ok {
		t.Fatalf("imports = %T", result.Imports[0])
	}
	l := &typescriptLang{}
	index := resolverIndex(l,
		struct{ pkg, name, ext string }{"pkg/demo/components", "index", ""},
		struct{ pkg, name, ext string }{"pkg/demo", "config.prod", ""},
		struct{ pkg, name, ext string }{"pkg/demo", "config", ""},
		struct{ pkg, name, ext string }{"pkg/demo", "x", ""},
		struct{ pkg, name, ext string }{"pkg/demo/x", "index", ""},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(projectKind, "app")
	l.Resolve(cfg, index, nil, r, imports, label.New("", "pkg/demo", "app"))
	if len(l.errors) != 0 {
		t.Fatalf("errors = %v", l.errors)
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//pkg/demo/components:index,:config.prod,:x" {
		t.Errorf("deps = %q", got)
	}

	owned := rule.NewRule(projectKind, "data")
	owned.SetAttr("srcs", []string{"data.json", "data.d.ts"})
	specs := l.Imports(cfg, owned, rule.EmptyFile("pkg/demo/BUILD.bazel", "pkg/demo"))
	if len(specs) != 1 || specs[0].Imp != "pkg/demo/data.json" {
		t.Fatalf("asset imports = %v", specs)
	}
	handIndex := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return l })
	handIndex.AddRule(config.New(), owned, rule.EmptyFile("pkg/demo/BUILD.bazel", "pkg/demo"))
	handIndex.Finish()
	r = rule.NewRule(projectKind, "app")
	l.Resolve(cfg, handIndex, nil, r, targetImports{imports: []string{"pkg/demo/data.json"}, local: map[string]bool{"pkg/demo/data.json": true}, asset: map[string]bool{"pkg/demo/data.json": true}}, label.New("", "pkg/demo", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != ":data" {
		t.Errorf("owned asset deps = %q", got)
	}

	missing := rule.NewRule(projectKind, "missing")
	l.Resolve(cfg, index, nil, missing, targetImports{imports: []string{"pkg/demo/nope"}, local: map[string]bool{"pkg/demo/nope": true}}, label.New("", "pkg/demo", "missing"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("missing errors = %v", l.errors)
	}
}

func declNames(result language.GenerateResult) string {
	var out []string
	for _, r := range result.Gen {
		out = append(out, r.Kind()+":"+r.Name())
	}
	return strings.Join(out, ",")
}

func generateWithFile(t *testing.T, files map[string]string, regular []string, f *rule.File) (language.GenerateResult, *typescriptLang) {
	t.Helper()
	root := t.TempDir()
	for name, content := range files {
		writeFixture(t, root, name, content)
	}
	l := &typescriptLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: regular,
		File:         f,
	})
	return result, l
}

func TestGenerateDeclarations(t *testing.T) {
	const decl = "export declare const x: number;\n"
	const src = "export const y = 1;\n"
	t.Run("mts and cts", func(t *testing.T) {
		result, _ := generateWithFile(t, map[string]string{
			"pkg/demo/a.ts": src, "pkg/demo/b.d.mts": decl, "pkg/demo/c.d.cts": decl,
		}, []string{"a.ts", "b.d.mts", "c.d.cts"}, nil)
		if got := declNames(result); got != "typescript_project:a,typescript_project:b,typescript_project:c" {
			t.Fatalf("rules = %s", got)
		}
		if got := result.Gen[2].AttrStrings("declaration_srcs"); strings.Join(got, ",") != "c.d.cts" {
			t.Errorf("c declaration_srcs = %v", got)
		}
	})
	t.Run("source with same stem owns it", func(t *testing.T) {
		result, _ := generateWithFile(t, map[string]string{
			"pkg/demo/foo.ts": src, "pkg/demo/foo.d.ts": decl,
		}, []string{"foo.ts", "foo.d.ts"}, nil)
		if got := declNames(result); got != "typescript_project:foo" {
			t.Fatalf("rules = %s", got)
		}
		if len(result.Gen[0].AttrStrings("declaration_srcs")) != 0 {
			t.Errorf("source rule gained declaration_srcs")
		}
	})
	t.Run("stem collision", func(t *testing.T) {
		result, l := generateWithFile(t, map[string]string{
			"pkg/demo/a.ts": src, "pkg/demo/foo.d.ts": decl, "pkg/demo/foo.d.mts": decl,
		}, []string{"a.ts", "foo.d.ts", "foo.d.mts"}, nil)
		if len(result.Gen) != 0 || len(l.errors) != 1 || !strings.Contains(l.errors[0], "claimed by") {
			t.Fatalf("gen %d errors %v", len(result.Gen), l.errors)
		}
	})
	t.Run("hand rule owns it", func(t *testing.T) {
		f := rule.EmptyFile("BUILD.bazel", "pkg/demo")
		types := rule.NewRule(projectKind, "types")
		types.SetAttr("declaration_srcs", []string{"widget.d.ts"})
		f.Rules = append(f.Rules, types)
		result, _ := generateWithFile(t, map[string]string{
			"pkg/demo/a.ts": src, "pkg/demo/widget.d.ts": decl,
		}, []string{"a.ts", "widget.d.ts"}, f)
		if got := declNames(result); got != "typescript_project:a" {
			t.Fatalf("rules = %s", got)
		}
	})
	t.Run("own earlier output is regenerated", func(t *testing.T) {
		f := rule.EmptyFile("BUILD.bazel", "pkg/demo")
		w := rule.NewRule(projectKind, "widget")
		w.SetAttr("declaration_srcs", []string{"widget.d.ts"})
		f.Rules = append(f.Rules, w)
		result, _ := generateWithFile(t, map[string]string{
			"pkg/demo/a.ts": src, "pkg/demo/widget.d.ts": decl,
		}, []string{"a.ts", "widget.d.ts"}, f)
		if got := declNames(result); got != "typescript_project:a,typescript_project:widget" || len(result.Empty) != 0 {
			t.Fatalf("rules = %s, empty = %v", got, result.Empty)
		}
	})
	t.Run("main declaration gets no binary", func(t *testing.T) {
		result, _ := generateWithFile(t, map[string]string{
			"pkg/demo/a.ts": src, "pkg/demo/main.d.ts": decl,
		}, []string{"a.ts", "main.d.ts"}, nil)
		if got := declNames(result); got != "typescript_project:a,typescript_project:main" {
			t.Fatalf("rules = %s", got)
		}
	})
	t.Run("source rule keeps declaration_srcs", func(t *testing.T) {
		f := rule.EmptyFile("BUILD.bazel", "pkg/demo")
		a := rule.NewRule(projectKind, "a")
		a.SetAttr("srcs", []string{"a.ts"})
		a.SetAttr("declaration_srcs", []string{"shared.d.ts"})
		f.Rules = append(f.Rules, a)
		result, _ := generateWithFile(t, map[string]string{
			"pkg/demo/a.ts": src, "pkg/demo/shared.d.ts": decl,
		}, []string{"a.ts", "shared.d.ts"}, f)
		if got := declNames(result); got != "typescript_project:a" {
			t.Fatalf("rules = %s", got)
		}
		if got := strings.Join(result.Gen[0].AttrStrings("declaration_srcs"), ","); got != "shared.d.ts" {
			t.Errorf("declaration_srcs = %q", got)
		}
	})
}

func TestImportsIndexesDeclarations(t *testing.T) {
	lang := NewLanguage()
	r := rule.NewRule(projectKind, "types")
	r.SetAttr("srcs", []string{"widget.ts"})
	r.SetAttr("declaration_srcs", []string{"widget.d.ts", ":bindings", "sub/x.d.ts", "other.d.mts"})
	f := rule.EmptyFile("pkg/BUILD.bazel", "pkg")
	var got []string
	for _, spec := range lang.Imports(&config.Config{}, r, f) {
		got = append(got, spec.Imp)
	}
	if strings.Join(got, ",") != "pkg/widget,pkg/other" {
		t.Errorf("imports = %v", got)
	}
}

func TestGenerateTestPackageJSONFollowsRootManifest(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"package.json":            "{\"type\": \"module\"}\n",
		"pkg/demo/helper.ts":      "export const helper = 1;\n",
		"pkg/demo/helper_test.ts": "import { helper } from \"./helper\";\n",
	}, []string{"helper.ts", "helper_test.ts"})
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
		"CONFIGBASE": `{"compilerOptions": {"paths": {"@/*": ["./*"], "~utils": ["./lib/util.ts"], "fs": ["./lib/util.ts"]}}}`,
		"bad/CONFIG": `{`,
	} {
		name = strings.NewReplacer("CONFIGBASE", "tsconfig.base.json", "CONFIG", "tsconfig.json").Replace(name)
		p := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(strings.ReplaceAll(content, "CONFIGBASE", "tsconfig.base.json")), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	l := &typescriptLang{}
	cfg := resolverConfig(t, nil)
	cfg.RepoRoot = root
	l.Configure(cfg, "", nil)
	app := cfg.Clone()
	l.Configure(app, "app", nil)
	if len(l.errors) != 0 {
		t.Fatalf("configure errors = %v", l.errors)
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
		r := rule.NewRule(projectKind, "main")
		l.Resolve(app, index, nil, r, targetImports{imports: []string{tc.name}}, label.New("", "app", "main"))
		if got := strings.Join(r.AttrStrings("deps"), ","); got != tc.want {
			t.Errorf("%q deps = %q, want %q", tc.name, got, tc.want)
		}
	}
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], `unresolved import "@/components"`) {
		t.Errorf("errors = %v", l.errors)
	}
	l.errors = nil
	l.Resolve(app, index, nil, rule.NewRule(projectKind, "main"), targetImports{imports: []string{"react"}}, label.New("", "app", "main"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "tsconfig/jsconfig paths alias") {
		t.Errorf("react errors = %v", l.errors)
	}
	l.errors = nil
	l.Configure(app.Clone(), "bad", nil)
	if len(l.errors) != 1 || !strings.HasPrefix(l.errors[0], "tsconfig: //bad/tsconfig.json: ") {
		t.Errorf("malformed errors = %v", l.errors)
	}
}
