package python

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
	regular := []string{"demo.py", "demo.pyi", "helper.py", "helper_test.py", "helper_test.pyi", "orphan.pyi"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.py":         "import helper\nfrom helper import suffix\nimport os\n",
		"pkg/demo/demo.pyi":        "def greet(name: str) -> str: ...\n",
		"pkg/demo/helper.py":       "def suffix(tag):\n    return tag\n",
		"pkg/demo/helper_test.py":  "import helper\n",
		"pkg/demo/helper_test.pyi": "import stub_only_dep\ndef test_suffix() -> None: ...\n",
		"pkg/demo/orphan.pyi":      "def unused() -> None: ...\n",
	}, regular)
	if len(result.Gen) != 3 || len(result.Imports) != 3 {
		t.Fatalf("generated %d rules and %d import sets, want 3 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != libraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.py,demo.pyi" {
		t.Errorf("library srcs = %q", got)
	}
	if got := strings.Join(lib.AttrStrings("imports"), ","); got != "." {
		t.Errorf("library imports = %q", got)
	}
	helper := result.Gen[1]
	if helper.Kind() != libraryKind || helper.Name() != "helper" {
		t.Fatalf("helper = %s(%s)", helper.Kind(), helper.Name())
	}
	if got := strings.Join(helper.AttrStrings("srcs"), ","); got != "helper.py" {
		t.Errorf("helper srcs = %q", got)
	}
	test := result.Gen[2]
	if test.Kind() != testKind || test.Name() != "helper_test" {
		t.Fatalf("test = %s(%s)", test.Kind(), test.Name())
	}
	if got := strings.Join(test.AttrStrings("srcs"), ","); got != "helper_test.py,helper_test.pyi" {
		t.Errorf("test srcs = %q", got)
	}
	testImports := result.Imports[2].(targetImports)
	if strings.Join(testImports.imports, ",") != "helper" {
		t.Errorf("test imports = %+v, want [helper] (stub_only_dep ignored)", testImports)
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

func TestGenerateEmptySweepsStale(t *testing.T) {
	result := generateFixture(t, nil, []string{"orphan.pyi"})
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
	writeFixture(t, root, "pkg/demo/demo.py", "x = 1\n")
	dir := filepath.Join(root, "pkg", "demo")
	result = NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.py"},
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
		{"readError", map[string]string{}, []string{"missing.py"}, "read missing.py"},
		{"emptyName", map[string]string{"pkg/demo/---.py": "x = 1\n"}, []string{"---.py"}, "empty target name"},
		{"collision", map[string]string{
			"pkg/demo/a-b.py": "x = 1\n",
			"pkg/demo/a_b.py": "y = 2\n",
		}, []string{"a-b.py", "a_b.py"}, "claimed by a-b.py, a_b.py"},
	}
	for _, tc := range cases {
		root := t.TempDir()
		for name, content := range tc.files {
			writeFixture(t, root, name, content)
		}
		l := &pythonLang{}
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
	claimants := []Claimant{{Name: "demo", Source: "demo.py"}}
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
	if err := checkClaims(nil, otherWrong, claimants); err == nil || !strings.Contains(err.Error(), "existing python_test") {
		t.Errorf("other kind mismatch = %v", err)
	}
	dupes := []Claimant{{Name: "a_b", Source: "a-b.py"}, {Name: "a_b", Source: "a_b.py"}}
	if err := checkClaims(wrongKind, nil, dupes); err == nil || !strings.Contains(err.Error(), "handwritten") {
		t.Errorf("collision with handwritten = %v", err)
	}
	testDupes := []Claimant{{Name: "a_test", Source: "a-test_test.py"}, {Name: "a_test", Source: "a_test_test.py"}}
	if err := checkClaims(nil, nil, testDupes); err == nil || !strings.Contains(err.Error(), "a_test") {
		t.Errorf("test collision = %v", err)
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
	lib.SetAttr("srcs", []string{"demo.py", "demo.pyi", "notes.txt"})
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
	l := &pythonLang{}
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
	l := &pythonLang{}
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("errors = %v", l.errors)
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := &pythonLang{}
	if l.Name() != "python" || len(l.Kinds()) != 3 || l.CheckFlags(flag.NewFlagSet("test", flag.ContinueOnError), config.New()) != nil {
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
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//python/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "python_library,python_test,python_binary" {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := l.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//python/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
	defaultApparent := l.ApparentLoads(func(string) string { return "" })
	if defaultApparent[0].Name != "@rules_dx//python/rules:defs.bzl" {
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

func resolverIndex(lang *pythonLang, entries ...struct {
	pkg  string
	name string
}) *resolve.RuleIndex {
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := pythonKinds[r.Kind()]; ok {
			return lang
		}
		return nil
	})
	for _, entry := range entries {
		r := rule.NewRule(libraryKind, entry.name)
		r.SetAttr("srcs", []string{entry.name + ".py"})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry.pkg, "BUILD.bazel"), entry.pkg))
	}
	index.Finish()
	return index
}

func TestResolveBranches(t *testing.T) {
	l := &pythonLang{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/b", "b"},
		struct{ pkg, name string }{"lib/a", "a"},
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
	l.Resolve(cfg, index, nil, std, targetImports{imports: []string{"os"}}, label.New("", "app", "uses_std"))
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

func TestResolveOverride(t *testing.T) {
	l := &pythonLang{}
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "python python mapped //mapped:dep"}})
	r := rule.NewRule(libraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"mapped"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
}

func TestResolveAmbiguous(t *testing.T) {
	l := &pythonLang{}
	cfg := resolverConfig(t, nil)
	index := resolverIndex(l,
		struct{ pkg, name string }{"one", "same"},
		struct{ pkg, name string }{"two", "same"},
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
		"pkg/demo/main.py":        "import helper\n",
		"pkg/demo/helper.py":      "def suffix(tag):\n    return tag\n",
		"pkg/demo/helper_test.py": "import helper\n",
	}, []string{"main.py", "helper.py", "helper_test.py"})
	if len(result.Gen) != 4 || len(result.Imports) != 4 {
		t.Fatalf("generated %d rules and %d import sets, want 4 each", len(result.Gen), len(result.Imports))
	}
	byRule := make(map[string]int, len(result.Gen))
	for i, r := range result.Gen {
		byRule[r.Kind()+"\x00"+r.Name()] = i
	}
	libIdx, ok := byRule[libraryKind+"\x00main"]
	if !ok {
		t.Fatalf("missing python_library(main) in %v", result.Gen)
	}
	binIdx, ok := byRule[binaryKind+"\x00main_bin"]
	if !ok {
		t.Fatalf("missing python_binary(main_bin) in %v", result.Gen)
	}
	lib := result.Gen[libIdx]
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "main.py" {
		t.Errorf("library srcs = %q, want main.py", got)
	}
	bin := result.Gen[binIdx]
	if got := bin.AttrString("main"); got != "main.py" {
		t.Errorf("binary main = %q, want main.py", got)
	}
	if got := strings.Join(bin.AttrStrings("deps"), ","); got != ":main" {
		t.Errorf("binary deps = %q, want :main", got)
	}
	if got := strings.Join(bin.AttrStrings("imports"), ","); got != "." {
		t.Errorf("binary imports = %q, want .", got)
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
	if _, ok := byRule[libraryKind+"\x00helper"]; !ok {
		t.Errorf("missing python_library(helper)")
	}
	if _, ok := byRule[testKind+"\x00helper_test"]; !ok {
		t.Errorf("missing python_test(helper_test)")
	}
}

func TestGenerateEntryCollision(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/main.py":     "x = 1\n",
		"pkg/demo/main_bin.py": "y = 2\n",
	} {
		writeFixture(t, root, name, content)
	}
	l := &pythonLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"main.py", "main_bin.py"},
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want none", len(result.Gen))
	}
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "main_bin") {
		t.Errorf("errors = %v, want main_bin collision", l.errors)
	}
}

func TestGenerateEntryHandwrittenMismatch(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/main.py", "x = 1\n")
	f := rule.EmptyFile("BUILD.bazel", "pkg/demo")
	f.Rules = append(f.Rules, rule.NewRule("filegroup", "main_bin"))
	l := &pythonLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"main.py"},
		File:         f,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want none", len(result.Gen))
	}
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "main_bin") {
		t.Errorf("errors = %v, want main_bin kind mismatch", l.errors)
	}
}

func TestClaimKind(t *testing.T) {
	if got := claimKind(Claimant{Name: "main_bin", Source: "main.py", Kind: binaryKind}); got != binaryKind {
		t.Errorf("explicit kind = %q, want %q", got, binaryKind)
	}
	if got := claimKind(Claimant{Name: "a_test", Source: "a_test.py"}); got != testKind {
		t.Errorf("inferred test kind = %q", got)
	}
	if got := claimKind(Claimant{Name: "demo", Source: "demo.py"}); got != libraryKind {
		t.Errorf("inferred library kind = %q", got)
	}
}

func TestCheckClaimsBinary(t *testing.T) {
	bin := rule.NewRule(binaryKind, "main_bin")
	same := rule.EmptyFile("BUILD.bazel", "pkg")
	same.Rules = append(same.Rules, bin)
	claim := []Claimant{{Name: "main", Source: "main.py", Kind: libraryKind}, {Name: "main_bin", Source: "main.py", Kind: binaryKind}}
	if err := checkClaims(same, nil, claim); err != nil {
		t.Errorf("same-kind binary claims = %v", err)
	}
	wrong := rule.EmptyFile("BUILD.bazel", "pkg")
	wrong.Rules = append(wrong.Rules, rule.NewRule(libraryKind, "main_bin"))
	if err := checkClaims(wrong, nil, claim); err == nil || !strings.Contains(err.Error(), "main_bin") {
		t.Errorf("binary kind mismatch = %v", err)
	}
}

func TestBinaryKindInfo(t *testing.T) {
	info := binaryKindInfo()
	for _, attr := range info.MatchAttrs {
		if attr == "srcs" {
			t.Errorf("binary MatchAttrs must not contain srcs: %v", info.MatchAttrs)
		}
	}
	if len(info.MatchAttrs) != 1 || info.MatchAttrs[0] != "main" {
		t.Errorf("binary MatchAttrs = %v, want [main]", info.MatchAttrs)
	}
	if info.NonEmptyAttrs["srcs"] {
		t.Errorf("binary NonEmptyAttrs must not require srcs: %v", info.NonEmptyAttrs)
	}
	if !info.MergeableAttrs["deps"] || !info.MergeableAttrs["imports"] {
		t.Errorf("binary MergeableAttrs = %v, want deps+imports", info.MergeableAttrs)
	}
	if !info.ResolveAttrs["deps"] {
		t.Errorf("binary ResolveAttrs = %v, want deps", info.ResolveAttrs)
	}
}

func TestResolvePreservesBinaryDeps(t *testing.T) {
	l := &pythonLang{}
	cfg := resolverConfig(t, nil)
	bin := rule.NewRule(binaryKind, "main_bin")
	bin.SetAttr("main", "main.py")
	bin.SetAttr("deps", []string{":main"})
	l.Resolve(cfg, resolverIndex(l), nil, bin, targetImports{}, label.New("", "app", "main_bin"))
	if got := strings.Join(bin.AttrStrings("deps"), ","); got != ":main" {
		t.Errorf("binary deps = %q, want preserved :main", got)
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
	l := &pythonLang{}
	l.Before(context.Background())
	root := config.New()
	file, err := rule.LoadData("BUILD.bazel", "", []byte("# gazelle:dx_ignore_import python missing_dep\n"))
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
	l := &pythonLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import python stale\n"))
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

func TestMalformedIgnoreFails(t *testing.T) {
	l := &pythonLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import python\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	defer func() {
		if got := recover(); got == nil || !strings.Contains(got.(string), "malformed") {
			t.Errorf("malformed ignore result = %v", got)
		}
	}()
	l.AfterResolvingDeps(context.Background())
}

func TestConfigureDirectiveForms(t *testing.T) {
	l := &pythonLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:other ignored\n# gazelle:dx_ignore_import python python external\n# gazelle:dx_ignore_import rust foreign\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	if len(l.ignores) != 1 || l.ignores[0].Value != "external" {
		t.Errorf("ignores = %+v", l.ignores)
	}
	l.ignores[0].Used = true
	l.AfterResolvingDeps(context.Background())
	if common.MatchingIgnore(config.New(), languageName, "none") != nil || common.MatchingIgnore(cfg, languageName, "none") != nil {
		t.Error("nonmatching ignore found")
	}
}

func TestResolveIgnore(t *testing.T) {
	l := &pythonLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import python external\n"))
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
	l := &pythonLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "python python mapped //mapped:dep"}})
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import python mapped\n"))
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
