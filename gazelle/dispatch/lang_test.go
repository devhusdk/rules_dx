package dispatch

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/bazelbuild/bazel-gazelle/rule"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

func testConfig() *config.Config {
	return &config.Config{
		RepoRoot:             "/repo",
		ValidBuildFileNames:  []string{"BUILD.bazel"},
		ModuleToApparentName: func(string) string { return "" },
	}
}

func mustLoad(t *testing.T, path, pkg, content string) *rule.File {
	t.Helper()
	f, err := rule.LoadData(path, pkg, []byte(content))
	if err != nil {
		t.Fatalf("LoadData: %v", err)
	}
	return f
}

func TestNameIsDispatch(t *testing.T) {
	if got := NewLanguage().Name(); got != "dx_dispatch" {
		t.Fatalf("Name() = %q, want %q", got, "dx_dispatch")
	}
	for _, lang := range composedLanguages() {
		if lang.Name() == "dx_dispatch" {
			t.Fatalf("composed language %T collides with dispatch name %q", lang, "dx_dispatch")
		}
	}
}

func TestGazelleBinaryLanguageOrder(t *testing.T) {
	data, err := os.ReadFile("BUILD.bazel")
	if err != nil {
		t.Fatalf("read BUILD.bazel: %v", err)
	}
	f, err := rule.LoadData("BUILD.bazel", "gazelle/dispatch", data)
	if err != nil {
		t.Fatalf("LoadData: %v", err)
	}
	var bin *rule.Rule
	for _, r := range f.Rules {
		if r.Kind() == "gazelle_binary" && r.Name() == "gazelle" {
			if bin != nil {
				t.Fatal("BUILD.bazel has more than one gazelle_binary named gazelle")
			}
			bin = r
		}
	}
	if bin == nil {
		t.Fatal("BUILD.bazel has no gazelle_binary named gazelle")
	}
	got := bin.AttrStrings("languages")
	seen := map[string]bool{}
	for i, l := range got {
		if seen[l] {
			t.Errorf("languages[%d] = %q is listed twice", i, l)
		}
		seen[l] = true
		if l == ":dispatch" && i != len(got)-1 {
			t.Errorf("languages[%d] = \":dispatch\", but :dispatch must be the last entry", i)
		}
	}
	var want []string
	for _, lang := range composedLanguages() {
		want = append(want, "//gazelle/"+lang.Name())
	}
	want = append(want, ":dispatch")
	n := len(got)
	if len(want) > n {
		n = len(want)
	}
	for i := 0; i < n; i++ {
		var g, w string
		if i < len(got) {
			g = got[i]
		}
		if i < len(want) {
			w = want[i]
		}
		if g != w {
			t.Errorf("languages[%d] = %q, want %q: the list must match composedLanguages() in order with :dispatch last", i, g, w)
		}
	}
	if t.Failed() {
		t.Logf("got  %s", strings.Join(got, " "))
		t.Logf("want %s", strings.Join(want, " "))
	}
}

func TestGenerateRulesRecordsOtherGenUnion(t *testing.T) {
	l := &dispatchLang{}
	l.Before(context.Background())
	cfg := testConfig()
	other := rule.NewRule("python_library", "handlers")
	res := l.GenerateRules(language.GenerateArgs{
		Config:   cfg,
		Dir:      "/repo/a",
		Rel:      "a",
		OtherGen: []*rule.Rule{other},
	})
	if len(res.Gen) != 0 {
		t.Fatalf("dispatch generates no rules, got %v", res.Gen)
	}
	if l.manifest != nil {
		t.Fatal("manifest must stay nil without DX_GENERATE_INTENDED")
	}
	if len(l.configs) != 1 {
		t.Fatalf("configs = %d, want 1", len(l.configs))
	}

	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	t.Setenv(common.EnvGenerateMode, "default")
	l2 := &dispatchLang{}
	l2.Before(context.Background())
	if l2.manifest == nil {
		t.Fatal("manifest must load with DX_GENERATE_INTENDED set")
	}
	if l2.manifest.Prefix != "dispatch" || !l2.manifest.IncludeOtherGen {
		t.Fatalf("recorder = %+v, want the dispatch prefix and the sibling rules", l2.manifest)
	}
	l2.GenerateRules(language.GenerateArgs{
		Config:   cfg,
		Dir:      t.TempDir(),
		Rel:      "a",
		OtherGen: []*rule.Rule{other},
	})
	if len(l2.manifest.Visited) != 1 {
		t.Fatalf("visited = %d, want 1", len(l2.manifest.Visited))
	}
	if len(l2.manifest.Visited[0].Gen) != 1 {
		t.Fatalf("union gen = %d, want 1", len(l2.manifest.Visited[0].Gen))
	}
}

func TestUnionLoadsCoverAllExtensions(t *testing.T) {
	loads := unionApparentLoads(func(string) string { return "" })
	if len(loads) < 15 {
		t.Fatalf("union loads = %d entries, want >= 15 for 15 extensions", len(loads))
	}
	seen := map[string]bool{}
	for _, load := range loads {
		seen[load.Name] = true
	}
	for _, want := range []string{
		"//elixir/rules:defs.bzl",
		"//powershell/rules:defs.bzl",
		"//python/rules:defs.bzl",
		"//rust/rules:defs.bzl",
	} {
		found := false
		for name := range seen {
			if len(name) >= len(want) && name[len(name)-len(want):] == want {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("union loads miss %s (have %v)", want, seen)
		}
	}
}

func TestAfterResolvingDepsWritesManifest(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	t.Setenv(common.EnvGenerateMode, "default")
	l := &dispatchLang{}
	l.Before(context.Background())
	cfg := testConfig()
	gen := rule.NewRule("python_library", "handlers")
	gen.SetAttr("srcs", []string{"handlers.py"})
	l.GenerateRules(language.GenerateArgs{
		Config:   cfg,
		Dir:      t.TempDir(),
		Rel:      "a",
		OtherGen: []*rule.Rule{gen},
	})
	l.AfterResolvingDeps(context.Background())
	data, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("read manifest: %v", err)
	}
	var manifest common.IntendedManifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		t.Fatalf("decode manifest: %v", err)
	}
	if manifest.Mode != "default" {
		t.Errorf("mode = %q, want default", manifest.Mode)
	}
	if len(manifest.Files) != 1 {
		t.Fatalf("files = %d, want 1 new file", len(manifest.Files))
	}
	if len(manifest.Files[0].CreateContent) == 0 {
		t.Error("new file must carry complete content")
	}
}

func TestAfterResolvingDepsEmitsInheritedIgnoreInScope(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	t.Setenv(common.EnvGenerateScope, `[{"element":"//a/b/...","dirs":["a/b"]}]`)
	l := &dispatchLang{}
	l.Before(context.Background())
	cfg := testConfig()
	cfg.Exts = map[string]interface{}{"cc": &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{
		{Value: "foo", Path: "a", UsedBy: []string{"a/b"}},
	}}}
	l.GenerateRules(language.GenerateArgs{Config: cfg, Dir: t.TempDir(), Rel: "a/b"})
	oldExit := exitProcess
	code := -1
	exitProcess = func(c int) { code = c }
	defer func() { exitProcess = oldExit }()
	l.AfterResolvingDeps(context.Background())
	if code != -1 {
		t.Fatalf("exit code = %d, errors = %v, want no exit", code, l.errors)
	}
	data, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("read manifest: %v", err)
	}
	var manifest common.IntendedManifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		t.Fatalf("decode manifest: %v", err)
	}
	want := []common.IntendedIgnoredImport{{Path: "a", Language: "cc", Import: "foo", ScopeIndex: 0}}
	if !reflect.DeepEqual(manifest.IgnoredImports, want) {
		t.Fatalf("ignored imports = %+v, want %+v", manifest.IgnoredImports, want)
	}
}

func TestCollectUsedIgnoresMergesUsers(t *testing.T) {
	parent := &common.IgnoreEntry{Value: "v", Path: "a", UsedBy: []string{"a/c"}}
	twin := &common.IgnoreEntry{Value: "v", Path: "a", UsedBy: []string{"a/b"}}
	one, two := testConfig(), testConfig()
	one.Exts = map[string]interface{}{"cc": &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{parent}}}
	two.Exts = map[string]interface{}{"cc": &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{parent, twin}}}
	got := (&dispatchLang{configs: []*config.Config{one, two}}).collectUsedIgnores()
	want := []common.IgnoredImport{{Path: "a", Language: "cc", Value: "v", UsedBy: []string{"a/b", "a/c"}}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("ignores = %+v, want %+v", got, want)
	}
}

func TestAfterResolvingDepsFailsClosedOnBadScope(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	t.Setenv(common.EnvGenerateScope, `[{"element":"//a:one","dirs":["a"]}]`)
	l := &dispatchLang{}
	l.Before(context.Background())
	cfg := testConfig()
	gen := rule.NewRule("python_library", "handlers")
	gen.SetAttr("srcs", []string{"handlers.py"})
	l.GenerateRules(language.GenerateArgs{
		Config:   cfg,
		Dir:      t.TempDir(),
		Rel:      "elsewhere",
		OtherGen: []*rule.Rule{gen},
	})
	code, errOut := stubExit(t)
	l.AfterResolvingDeps(context.Background())
	if *code != 1 {
		t.Fatalf("exit code = %d, want 1", *code)
	}
	if !strings.HasPrefix(errOut.String(), "Generation failed:\n") || strings.Count(errOut.String(), "\n") < 2 {
		t.Fatalf("stderr = %q, want the scope mismatch under one header", errOut.String())
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("manifest stat = %v, want no manifest written", err)
	}
}

func TestCollectUsedIgnoresEmptyWithoutConfigs(t *testing.T) {
	l := &dispatchLang{}
	if got := l.collectUsedIgnores(); len(got) != 0 {
		t.Fatalf("ignores = %v, want empty", got)
	}
}

func TestCollectUsedIgnoresSortsAndFilters(t *testing.T) {
	cfg := testConfig()
	cfg.Exts = map[string]interface{}{}
	cfg.Exts["cc"] = &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{
		{Value: "zlib", Path: "pkg/z.cc", UsedBy: []string{"pkg/z.cc"}},
		{Value: "stale", Path: "pkg/s.cc"},
	}}
	cfg.Exts["vue"] = &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{
		{Value: "chart", Path: "pkg/a.vue", UsedBy: []string{"pkg/a.vue"}},
	}}
	cfg.Exts["gleam"] = &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{
		{Value: "gleam/io", Path: "pkg/app.gleam", UsedBy: []string{"pkg/app.gleam"}},
	}}
	l := &dispatchLang{configs: []*config.Config{cfg, nil, cfg}}
	want := []common.IgnoredImport{
		{Path: "pkg/a.vue", Language: "vue", Value: "chart", UsedBy: []string{"pkg/a.vue"}},
		{Path: "pkg/app.gleam", Language: "gleam", Value: "gleam/io", UsedBy: []string{"pkg/app.gleam"}},
		{Path: "pkg/z.cc", Language: "cc", Value: "zlib", UsedBy: []string{"pkg/z.cc"}},
	}
	got := l.collectUsedIgnores()
	if len(got) != len(want) {
		t.Fatalf("ignores = %+v, want %+v", got, want)
	}
	for i := range want {
		if !reflect.DeepEqual(got[i], want[i]) {
			t.Errorf("ignore %d = %+v, want %+v", i, got[i], want[i])
		}
	}
}

func TestCollectUsedIgnoresCoversEveryIgnoreLanguage(t *testing.T) {
	cfg := testConfig()
	cfg.Exts = map[string]interface{}{}
	names := ignoreLanguages()
	for _, name := range names {
		cfg.Exts[name] = &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{{Value: "v", Path: "pkg/" + name, UsedBy: []string{"pkg/" + name}}}}
	}
	got := (&dispatchLang{configs: []*config.Config{cfg}}).collectUsedIgnores()
	if len(got) != len(names) {
		t.Fatalf("ignores = %+v, want one per language in %v", got, names)
	}
	for i, name := range names {
		want := common.IgnoredImport{Path: "pkg/" + name, Language: name, Value: "v", UsedBy: []string{"pkg/" + name}}
		if !reflect.DeepEqual(got[i], want) {
			t.Errorf("ignore %d = %+v, want %+v", i, got[i], want)
		}
	}
}

type failingLanguage interface {
	Before(context.Context)
	Fail(string, ...interface{})
	AfterResolvingDeps(context.Context)
}

func stubExit(t *testing.T) (*int, *bytes.Buffer) {
	t.Helper()
	oldExit, oldStderr := exitProcess, stderr
	code := -1
	var out bytes.Buffer
	exitProcess = func(c int) { code = c }
	stderr = &out
	t.Cleanup(func() { exitProcess, stderr = oldExit, oldStderr })
	return &code, &out
}

func TestComposedLanguagesReportWithoutPanicking(t *testing.T) {
	checked := 0
	for _, composed := range composedLanguages() {
		lang, ok := composed.(failingLanguage)
		if !ok {
			continue
		}
		name := composed.Name()
		checked++
		func() {
			defer func() {
				if r := recover(); r != nil {
					t.Errorf("%s AfterResolvingDeps panicked: %v", name, r)
				}
			}()
			lang.Before(context.Background())
			lang.Fail("%s: boom", name)
			lang.AfterResolvingDeps(context.Background())
		}()
		got := common.Errors.Take()
		if len(got) != 1 || got[0].Language != name || got[0].Message != name+": boom" {
			t.Errorf("%s sink = %#v, want one entry for %s", name, got, name)
		}
	}
	if checked == 0 {
		t.Fatal("no composed language reports failures")
	}
}

func TestAfterResolvingDepsReportsEveryLanguage(t *testing.T) {
	t.Setenv(common.EnvIntendedManifest, "")
	l := &dispatchLang{}
	l.Before(context.Background())
	common.Errors.Add("go", "", "go: //b:b: unresolved import \"x\"")
	common.Errors.Add("cc", "", "cc: //a:a: unresolved import \"y\"")
	code, errOut := stubExit(t)
	l.AfterResolvingDeps(context.Background())
	if *code != 1 {
		t.Fatalf("exit code = %d, want 1", *code)
	}
	want := "Generation failed:\ncc: //a:a: unresolved import \"y\"\ngo: //b:b: unresolved import \"x\"\n"
	if errOut.String() != want {
		t.Fatalf("stderr = %q, want %q", errOut.String(), want)
	}
	if got := common.Errors.Take(); got != nil {
		t.Fatalf("sink after report = %#v, want empty", got)
	}
}

func TestLanguageInterfaceNoOps(t *testing.T) {
	l, ok := NewLanguage().(*dispatchLang)
	if !ok {
		t.Fatalf("NewLanguage() = %T, want *dispatchLang", NewLanguage())
	}
	l.DoneGeneratingRules()
	l.RegisterFlags(nil, "update", testConfig())
	if err := l.CheckFlags(nil, testConfig()); err != nil {
		t.Errorf("CheckFlags() = %v, want nil", err)
	}
	if got := l.KnownDirectives(); got != nil {
		t.Errorf("KnownDirectives() = %v, want nil", got)
	}
	l.Configure(testConfig(), "app", nil)
	if len(l.errors) != 0 {
		t.Errorf("Configure(nil file) errors = %v, want none", l.errors)
	}
	if got := l.Kinds(); got != nil {
		t.Errorf("Kinds() = %v, want nil", got)
	}
}

func TestBeforeFailsClosedOnBadGenerateMode(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	t.Setenv(common.EnvGenerateMode, "bogus")
	l := &dispatchLang{}
	l.Before(context.Background())
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "must be") {
		t.Fatalf("errors = %v, want one generate-mode rejection", l.errors)
	}
	if l.manifest != nil {
		t.Fatal("manifest must stay nil after a load failure")
	}
}

func TestCollectUsedIgnoresTieBreaksOnLanguageAndValue(t *testing.T) {
	cfg := testConfig()
	cfg.Exts = map[string]interface{}{}
	cfg.Exts["cc"] = &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{
		{Value: "b", Path: "pkg/same", UsedBy: []string{"pkg/same"}},
		{Value: "a", Path: "pkg/same", UsedBy: []string{"pkg/same"}},
	}}
	cfg.Exts["java"] = &common.IgnoreConfig{Ignores: []*common.IgnoreEntry{
		{Value: "c", Path: "pkg/same", UsedBy: []string{"pkg/same"}},
	}}
	l := &dispatchLang{configs: []*config.Config{cfg}}
	want := []common.IgnoredImport{
		{Path: "pkg/same", Language: "cc", Value: "a", UsedBy: []string{"pkg/same"}},
		{Path: "pkg/same", Language: "cc", Value: "b", UsedBy: []string{"pkg/same"}},
		{Path: "pkg/same", Language: "java", Value: "c", UsedBy: []string{"pkg/same"}},
	}
	got := l.collectUsedIgnores()
	if len(got) != len(want) {
		t.Fatalf("ignores = %+v, want %+v", got, want)
	}
	for i := range want {
		if !reflect.DeepEqual(got[i], want[i]) {
			t.Errorf("ignore %d = %+v, want %+v", i, got[i], want[i])
		}
	}
}

type plainLoadsLang struct {
	language.BaseLang
}

func (plainLoadsLang) Loads() []rule.LoadInfo {
	return []rule.LoadInfo{{Name: "//stub:defs.bzl"}}
}

func TestUnionApparentLoadsFallsBackToPlainLoads(t *testing.T) {
	loads := unionApparentLoadsFrom([]language.Language{&plainLoadsLang{}}, func(string) string { return "" })
	if len(loads) != 1 || loads[0].Name != "//stub:defs.bzl" {
		t.Fatalf("loads = %v, want the plain stub load", loads)
	}
	if _, ok := interface{}(&plainLoadsLang{}).(language.ModuleAwareLanguage); ok {
		t.Fatal("plainLoadsLang must stay outside ModuleAwareLanguage so the fallback is real")
	}
}

func sortedKeys(m map[string]string) []string {
	keys := make([]string, 0, len(m))
	for key := range m {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

func TestSingleLanguageUnresolvedImportHints(t *testing.T) {
	const oneSource = "add a local one-source library or an exact # gazelle:resolve mapping"
	const oneSourceRelative = oneSource + " (relative imports use the workspace path without extension and also search TypeScript and JavaScript targets)"
	const jvm = "add a local library or a # gazelle:resolve mapping on the qualified import or its package"
	const csharp = "add a local library that declares the namespace or a # gazelle:resolve mapping on the full namespace"
	hints := map[string]string{
		"astro":  oneSourceRelative,
		"cc":     "add a local one-header library or an exact # gazelle:resolve mapping",
		"csharp": csharp,
		"fsharp": oneSource,
		"java":   jvm,
		"kotlin": jvm,
		"mdx":    oneSourceRelative,
		"ruby":   oneSource,
		"scala":  jvm,
		"svelte": oneSourceRelative,
		"vue":    oneSourceRelative,
	}
	composed := map[string]language.Language{}
	for _, lang := range composedLanguages() {
		composed[lang.Name()] = lang
		if _, single := lang.(*common.SingleLang); single && hints[lang.Name()] == "" {
			t.Errorf("%s shares the single-language driver but pins no unresolved-import hint", lang.Name())
		}
	}
	emptyIndex := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil })
	emptyIndex.Finish()
	resolverCfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", resolverCfg)
	if err := resolver.CheckFlags(nil, resolverCfg); err != nil {
		t.Fatalf("resolver CheckFlags: %v", err)
	}
	resolver.Configure(resolverCfg, "app", nil)
	for _, name := range sortedKeys(hints) {
		lang, ok := composed[name].(interface {
			language.Language
			failingLanguage
		})
		if !ok {
			t.Errorf("%s is not composed into dispatch, or does not share the single-language driver", name)
			continue
		}
		kinds := lang.Kinds()
		if len(kinds) == 0 {
			t.Errorf("%s declares no kinds", name)
			continue
		}
		kind := ""
		for k := range kinds {
			if kind == "" || k < kind {
				kind = k
			}
		}
		lang.Before(context.Background())
		from := label.New("", "app", "app")
		lang.Resolve(resolverCfg, emptyIndex, nil, rule.NewRule(kind, "app"),
			common.WrapImportSet([]string{"Missing"}), from)
		want := name + ": " + from.String() + ": unresolved import \"Missing\"; " + hints[name]
		lang.AfterResolvingDeps(context.Background())
		if got := common.Errors.Take(); len(got) != 1 || got[0].Message != want {
			t.Errorf("%s unresolved import = %#v, want %q", name, got, want)
		}
	}
}

func ignoreFile(values ...string) *rule.File {
	f := rule.EmptyFile("pkg/BUILD.bazel", "pkg")
	for _, v := range values {
		f.Directives = append(f.Directives, rule.Directive{Key: "dx_ignore_import", Value: v})
	}
	return f
}

func TestConfigureRejectsUnknownIgnoreLanguage(t *testing.T) {
	cases := []struct {
		value string
		want  string
	}{
		{value: "pyton requests", want: `unknown language "pyton"`},
		{value: "ts react", want: `unknown language "ts"`},
		{value: "", want: "malformed"},
		{value: "python requests"},
		{value: "gleam x"},
		{value: "go a go b"},
		{value: "rust a b c"},
	}
	for _, tc := range cases {
		l := &dispatchLang{}
		l.Configure(testConfig(), "pkg", ignoreFile(tc.value))
		if tc.want == "" {
			if len(l.errors) != 0 {
				t.Errorf("%q: errors = %v, want none", tc.value, l.errors)
			}
			continue
		}
		if len(l.errors) != 1 || !strings.Contains(l.errors[0], tc.want) || !strings.Contains(l.errors[0], "//pkg") {
			t.Errorf("%q: errors = %v, want one containing %q", tc.value, l.errors, tc.want)
			continue
		}
		if strings.HasPrefix(tc.want, "unknown") && !strings.Contains(l.errors[0], "use one of: astro, cc,") {
			t.Errorf("%q: error %q must list the accepted languages", tc.value, l.errors[0])
		}
	}
	l := &dispatchLang{}
	f := rule.EmptyFile("pkg/BUILD.bazel", "pkg")
	f.Directives = []rule.Directive{{Key: "resolve", Value: "pyton requests //x"}}
	l.Configure(testConfig(), "pkg", f)
	if len(l.errors) != 0 {
		t.Errorf("non-ignore directive errors = %v, want none", l.errors)
	}
}

func TestIgnoreLanguageNamesCoverComposed(t *testing.T) {
	names := ignoreLanguages()
	if !sort.StringsAreSorted(names) {
		t.Errorf("names = %v, want sorted", names)
	}
	set := map[string]bool{}
	for _, name := range names {
		if set[name] {
			t.Errorf("duplicate name %q", name)
		}
		set[name] = true
	}
	if set[languageName] {
		t.Errorf("names = %v, must not contain %q", names, languageName)
	}
	for _, lang := range composedLanguages() {
		if !set[lang.Name()] {
			t.Errorf("composed language %q missing from %v", lang.Name(), names)
		}
	}
	langs := append(composedLanguages(), &plainLoadsLang{}, composedLanguages()[0])
	got := ignoreLanguageNames(langs)
	if strings.Join(got, ",") != strings.Join(names, ",") {
		t.Errorf("names with stub = %v, want %v", got, names)
	}
}

func TestAfterResolvingDepsFailsOnUnknownIgnoreLanguage(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	l := &dispatchLang{}
	l.Before(context.Background())
	l.Configure(testConfig(), "", ignoreFile("pyton requests"))
	code, errOut := stubExit(t)
	l.AfterResolvingDeps(context.Background())
	if *code != 1 {
		t.Fatalf("exit code = %d, want 1", *code)
	}
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], `unknown language "pyton"`) || !strings.Contains(errOut.String(), `unknown language "pyton"`) {
		t.Fatalf("errors = %v, stderr = %q, want the unknown language", l.errors, errOut.String())
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("manifest stat = %v, want no manifest written", err)
	}
}

func langFile(value string) *rule.File {
	f := rule.EmptyFile("pkg/BUILD.bazel", "pkg")
	f.Directives = append(f.Directives, rule.Directive{Key: "lang", Value: value})
	return f
}

func TestConfigureRejectsLangWithoutDispatch(t *testing.T) {
	cases := []struct {
		name    string
		langs   []string
		file    *rule.File
		wantErr bool
	}{
		{name: "go", langs: []string{"go"}, file: langFile("go"), wantErr: true},
		{name: "go,python", langs: []string{"go", "python"}, file: langFile("go,python"), wantErr: true},
		{name: "with dispatch", langs: []string{"go", languageName}, file: langFile("go," + languageName)},
		{name: "empty", langs: nil, file: langFile("")},
		{name: "inherited", langs: []string{"go"}, file: rule.EmptyFile("pkg/BUILD.bazel", "pkg")},
	}
	for _, tc := range cases {
		l := &dispatchLang{}
		c := testConfig()
		c.Langs = tc.langs
		l.Configure(c, "pkg", tc.file)
		if !tc.wantErr {
			if len(l.errors) != 0 {
				t.Errorf("%s: errors = %v, want none", tc.name, l.errors)
			}
			continue
		}
		if len(l.errors) != 1 || !strings.Contains(l.errors[0], "//pkg") || !strings.Contains(l.errors[0], languageName) {
			t.Errorf("%s: errors = %v, want one naming //pkg and %s", tc.name, l.errors, languageName)
		}
	}
	l := &dispatchLang{}
	l.Configure(nil, "pkg", langFile("go"))
	if len(l.errors) != 0 {
		t.Errorf("nil config errors = %v, want none", l.errors)
	}
}

func TestAfterResolvingDepsFailsOnLangWithoutDispatch(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	l := &dispatchLang{}
	l.Before(context.Background())
	c := testConfig()
	c.Langs = []string{"go"}
	l.Configure(c, "", langFile("go"))
	code, _ := stubExit(t)
	l.AfterResolvingDeps(context.Background())
	if *code != 1 {
		t.Fatalf("exit code = %d, want 1", *code)
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("manifest stat = %v, want no manifest written", err)
	}
}

func TestFallbackLanguagesAreComposed(t *testing.T) {
	names := make(map[string]bool)
	for _, lang := range composedLanguages() {
		names[lang.Name()] = true
	}
	checked := 0
	for _, lang := range composedLanguages() {
		fb, ok := lang.(interface{ Fallback() []string })
		if !ok {
			continue
		}
		for _, name := range fb.Fallback() {
			checked++
			if !names[name] {
				t.Errorf("%s: fallback language %q is not composed", lang.Name(), name)
			}
		}
	}
	if checked == 0 {
		t.Error("no language declares a fallback")
	}
}

func TestOnlyDispatchWritesTheManifest(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	t.Setenv(common.EnvGenerateMode, "default")
	for _, lang := range composedLanguages() {
		lm, ok := lang.(language.LifecycleManager)
		if !ok {
			continue
		}
		lm.Before(context.Background())
		lm.AfterResolvingDeps(context.Background())
		if _, err := os.Stat(out); !os.IsNotExist(err) {
			t.Fatalf("language %s wrote the intended manifest (stat err = %v)", lang.Name(), err)
		}
	}
	l := &dispatchLang{}
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if _, err := os.Stat(out); err != nil {
		t.Fatalf("dispatch did not write the manifest: %v", err)
	}
}
