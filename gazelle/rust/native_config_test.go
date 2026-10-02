package rust

import (
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

func TestCorpusSplitPreservesSiblingsAndFixtureOwnership(t *testing.T) {
	root := t.TempDir()
	for _, name := range []string{"BUILD.bazel", "README.md", "sibling.md", "fixture.md", "config.toml", ".gitleaks.toml", ".hidden.md", "AGENTS.local.md", "new.json", "keep.json", "fixture.json", "sub/guide.md", "sub/config.toml", "nested/BUILD", "nested/hidden.md", "other/BUILD.bazel", "other/hidden.md", ".state/hidden.md", "node_modules/hidden.md", "bazel-out/hidden.md", "data.lock"} {
		writeFixture(t, root, name, "")
	}
	writeFixture(t, root, ".buildifier.json", "{}")
	writeFixture(t, root, "biome.json", "{}")
	file := rule.EmptyFile(filepath.Join(root, "BUILD.bazel"), "")
	legacy := rule.NewRule(corpusKind, "corpus")
	legacy.SetAttr("markdown_siblings", []string{"sibling.md", "//other:doc", ":local", "@repo//:doc", "*.md", ""})
	legacy.SetAttr("json_srcs", []string{"keep.json", "fixture.json", "@repo//:json", "//other:json", ":json", "*.json", ""})
	legacy.Insert(file)
	fixture := rule.NewRule(corpusKind, "fixture")
	fixture.SetAttr("markdown_srcs", []string{"fixture.md", "@repo//:doc", "//other:doc", ":local", "*.md", ""})
	fixture.SetAttr("json_srcs", []string{"fixture.json"})
	fixture.Insert(file)
	cfg := config.New()
	cfg.RepoRoot = root
	plan := planCorpus(language.GenerateArgs{Config: cfg, Dir: root, File: file}, false)
	byName := map[string]*rule.Rule{}
	for _, r := range plan.gen {
		byName[r.Name()] = r
	}
	for name, want := range map[string]string{"corpus_markdown": "README.md,sub/guide.md", "corpus_toml": ".gitleaks.toml,config.toml,sub/config.toml", "corpus_json": "keep.json", "corpus_starlark": "BUILD.bazel"} {
		attr := strings.Replace(name, "corpus_", "", 1) + "_srcs"
		if byName[name] == nil || strings.Join(byName[name].AttrStrings(attr), ",") != want {
			t.Fatalf("%s: %v, want %s", name, byName[name], want)
		}
	}
	if len(byName["corpus_markdown"].AttrStrings("markdown_siblings")) != 6 {
		t.Fatal("siblings lost")
	}
	if got := byName["corpus_starlark"].AttrStrings("aspect_hints"); len(got) != 1 || got[0] != corpusBuildifierHint {
		t.Fatalf("buildifier hints: %v", got)
	}
	if got := byName["corpus_json"].AttrStrings("aspect_hints"); len(got) != 1 || got[0] != corpusBiomeHint {
		t.Fatalf("biome hints: %v", got)
	}
	if len(plan.empty) != 1 || plan.empty[0].Name() != "corpus" {
		t.Fatalf("legacy deletion: %v", plan.empty)
	}
}

func TestCorpusJsonBiomeHintNeedsRootConfig(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "BUILD.bazel", "")
	writeFixture(t, root, ".buildifier.json", "{}")
	file := rule.EmptyFile(filepath.Join(root, "BUILD.bazel"), "")
	legacy := rule.NewRule(corpusKind, "corpus")
	legacy.SetAttr("json_srcs", []string{"keep.json"})
	legacy.Insert(file)
	cfg := config.New()
	cfg.RepoRoot = root
	plan := planCorpus(language.GenerateArgs{Config: cfg, Dir: root, File: file}, false)
	for _, r := range plan.gen {
		if r.Name() == "corpus_json" {
			if got := r.AttrStrings("aspect_hints"); len(got) != 0 {
				t.Fatalf("biome hints without a root biome.json: %v", got)
			}
			return
		}
	}
	t.Fatal("missing corpus_json")
}

func TestCorpusSkipsToolTreesAndKeepsSiblingOnlyTargets(t *testing.T) {
	cfg := config.New()
	root := t.TempDir()
	for _, rel := range []string{".opencode", ".opencode/skills", "node_modules", "node_modules/pkg", "pkg/node_modules/pkg", "tools/depcheck/testdata", "tools/depcheck/testdata/fixture"} {
		plan := planCorpus(language.GenerateArgs{Config: cfg, Dir: root, Rel: rel}, true)
		if len(plan.gen) != 0 {
			t.Fatalf("tool tree generated: %s", rel)
		}
	}
	file := rule.EmptyFile(filepath.Join(root, "BUILD.bazel"), "")
	legacy := rule.NewRule(corpusKind, "corpus")
	legacy.SetAttr("markdown_siblings", []string{"//other:doc"})
	legacy.Insert(file)
	plan := planCorpus(language.GenerateArgs{Config: cfg, Dir: root, File: file}, false)
	for _, r := range plan.gen {
		if r.Name() == "corpus_markdown" {
			if len(r.AttrStrings("markdown_srcs")) != 0 || len(r.AttrStrings("aspect_hints")) != 0 || len(r.AttrStrings("markdown_siblings")) != 1 {
				t.Fatalf("sibling target: %v", r)
			}
			return
		}
	}
	t.Fatal("missing sibling-only target")
}

func runNativeGenerate(t *testing.T, rel string, files map[string]string, build string) (*rustLang, language.GenerateResult) {
	t.Helper()
	root := t.TempDir()
	for name, content := range files {
		writeFixture(t, root, name, content)
	}
	dir := root
	if rel != "" {
		dir = filepath.Join(root, filepath.FromSlash(rel))
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	var regular []string
	for name := range files {
		parent := path.Dir(name)
		if parent == "." {
			parent = ""
		}
		if parent == rel {
			regular = append(regular, path.Base(name))
		}
	}
	sort.Strings(regular)
	cfg := config.New()
	cfg.RepoRoot = root
	l := &rustLang{}
	l.Configure(cfg, "", nil)
	var file *rule.File
	if build != "" {
		var err error
		file, err = rule.LoadData("BUILD.bazel", rel, []byte(build))
		if err != nil {
			t.Fatal(err)
		}
		if rel != "" {
			l.Configure(cfg, rel, file)
		} else {
			l.Configure(cfg, "", file)
		}
	} else if rel != "" {
		l.Configure(cfg, rel, nil)
	}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       cfg,
		Dir:          dir,
		Rel:          rel,
		RegularFiles: regular,
		File:         file,
	})
	return l, result
}

func findGenerated(result language.GenerateResult, kind, name string) *rule.Rule {
	for _, r := range result.Gen {
		if r.Kind() == kind && r.Name() == name {
			return r
		}
	}
	return nil
}

func findEmpty(result language.GenerateResult, kind, name string) bool {
	for _, r := range result.Empty {
		if r.Kind() == kind && r.Name() == name {
			return true
		}
	}
	return false
}

func TestNativeConfigRecognition(t *testing.T) {
	_, result := runNativeGenerate(t, "site", map[string]string{
		"site/src/lib.rs":           "pub fn current() {}\n",
		"site/rustfmt.toml":         "edition = \"2021\"\n",
		"site/clippy.toml":          "[lints]\n",
		"site/.vale.ini":            "StylesPath = styles\n",
		"site/styles/org/Rules.yml": "extends: existence\n",
		"site/taplo.toml":           "[formatting]\n",
		"site/.buildifier.json":     "{}\n",
		"site/custom.toml":          "[custom]\n",
		"site/vale_test.ini":        "StylesPath = styles\n",
	}, "")
	want := map[string]string{
		"rustfmt_config":    "rustfmt.toml",
		"vale_config":       ".vale.ini",
		"taplo_config":      "taplo.toml",
		"buildifier_config": ".buildifier.json",
	}
	if len(result.Gen) != len(want)+3 {
		names := []string{}
		for _, r := range result.Gen {
			names = append(names, r.Kind()+":"+r.Name())
		}
		t.Fatalf("generated %d rules %v, want lib plus %d configs plus corpus splits", len(result.Gen), names, len(want))
	}
	lib := findGenerated(result, libraryKind, "site")
	if lib == nil {
		t.Fatal("missing rust_library(site)")
	}
	if got := strings.Join(lib.AttrStrings("aspect_hints"), ","); got != ":rustfmt_config" {
		t.Errorf("library aspect_hints = %q, want managed rust additions in canonical order", got)
	}
	for name, src := range want {
		kind := strings.TrimSuffix(name, "_config") + "_config"
		r := findGenerated(result, kind, name)
		if r == nil {
			t.Errorf("missing %s(%s)", kind, name)
			continue
		}
		if r.AttrString("src") != src {
			t.Errorf("%s src = %q, want %q", name, r.AttrString("src"), src)
		}
		if got := strings.Join(r.AttrStrings("visibility"), ","); got != "//site:__subpackages__" {
			t.Errorf("%s visibility = %q, want package-scoped", name, got)
		}
	}
	if findGenerated(result, "clippy_config", "clippy_config") != nil {
		t.Error("unmanaged clippy.toml produced a config target")
	}
	vale := findGenerated(result, "vale_config", "vale_config")
	if got := strings.Join(vale.AttrStrings("data"), ","); got != "site/styles/org/Rules.yml" {
		t.Errorf("vale data = %q, want styles closure", got)
	}
	if rustfmt := findGenerated(result, "rustfmt_config", "rustfmt_config"); len(rustfmt.AttrStrings("data")) != 0 {
		t.Errorf("rustfmt data = %v, want no closure", rustfmt.AttrStrings("data"))
	}
}

func TestNativeConfigSelection(t *testing.T) {
	files := map[string]string{
		"site/src/lib.rs":   "pub fn current() {}\n",
		"site/rustfmt.toml": "edition = \"2021\"\n",
		"site/taplo.toml":   "[formatting]\n",
	}
	_, result := runNativeGenerate(t, "site", files, "# gazelle:dx_native_tools rustfmt\n")
	if findGenerated(result, "rustfmt_config", "rustfmt_config") == nil {
		t.Error("selected rustfmt produced no target")
	}
	if findGenerated(result, "taplo_config", "taplo_config") != nil {
		t.Error("skipped taplo produced a target")
	}
	lib := findGenerated(result, libraryKind, "site")
	if got := strings.Join(lib.AttrStrings("aspect_hints"), ","); got != ":rustfmt_config" {
		t.Errorf("library aspect_hints = %q, want only the selected tool", got)
	}

	l, _ := runNativeGenerate(t, "site", files, "# gazelle:dx_native_tools rustfmt bogus\n")
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], `unknown native tool "bogus"`) {
		t.Errorf("unknown tool errors = %v", l.errors)
	}

	clippy, _ := runNativeGenerate(t, "site", files, "# gazelle:dx_native_tools clippy\n")
	if len(clippy.errors) != 1 || !strings.Contains(clippy.errors[0], `unknown native tool "clippy"`) {
		t.Errorf("removed clippy tool errors = %v", clippy.errors)
	}

	bare, _ := runNativeGenerate(t, "site", files, "# gazelle:dx_native_tools\n")
	if len(bare.errors) != 1 || !strings.Contains(bare.errors[0], "malformed") {
		t.Errorf("bare directive errors = %v", bare.errors)
	}
}

func TestNativeConfigDirectiveInheritance(t *testing.T) {
	cfg := config.New()
	l := &rustLang{}
	l.Configure(cfg, "", nil)
	parent, err := rule.LoadData("BUILD.bazel", "", []byte("# gazelle:dx_native_tools taplo\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "", parent)
	if got := strings.Join(selectedNativeTools(cfg), ","); got != "taplo" {
		t.Fatalf("root tools = %q, want taplo", got)
	}
	child, err := rule.LoadData("BUILD.bazel", "sub", []byte("# build comment, no directive\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "sub", child)
	if got := strings.Join(selectedNativeTools(cfg), ","); got != "taplo" {
		t.Errorf("inherited tools = %q, want taplo", got)
	}
	override, err := rule.LoadData("BUILD.bazel", "other", []byte("# gazelle:dx_native_tools rustfmt\n# gazelle:dx_native_tools taplo\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "other", override)
	if got := strings.Join(selectedNativeTools(cfg), ","); got != "taplo" {
		t.Errorf("overridden tools = %q, want nearest directive to win", got)
	}
}

func TestNativeHintsMerge(t *testing.T) {
	build := `load("@rules_dx//rust/rules:defs.bzl", "rust_library")

rust_library(
    name = "site",
    srcs = ["src/lib.rs"],
    crate_name = "site",
    crate_root = "src/lib.rs",
    aspect_hints = [
        ":hand_cfg",
        ":rustfmt_config",
        "//site:other_cfg",
    ],
)
`
	_, result := runNativeGenerate(t, "site", map[string]string{
		"site/src/lib.rs": "pub fn current() {}\n",
	}, build)
	lib := findGenerated(result, libraryKind, "site")
	if lib == nil {
		t.Fatal("missing rust_library(site)")
	}
	if got := strings.Join(lib.AttrStrings("aspect_hints"), ","); got != ":hand_cfg,//site:other_cfg" {
		t.Errorf("merged aspect_hints = %q", got)
	}
	if findGenerated(result, "rustfmt_config", "rustfmt_config") != nil {
		t.Error("absent rustfmt.toml produced a target")
	}
}

func TestNativeExistingGeneratedConfigRemainsCurrent(t *testing.T) {
	build := `load("//quality:native_config.bzl", "rustfmt_config")

rustfmt_config(
    name = "rustfmt_config",
    src = "rustfmt.toml",
)
`
	_, result := runNativeGenerate(t, "site", map[string]string{
		"site/rustfmt.toml": "edition = \"2021\"\n",
	}, build)
	if findEmpty(result, "rustfmt_config", "rustfmt_config") {
		t.Error("current generated-shaped config was stubbed")
	}
}

func TestNativeHintsIgnoreNonRustAndConfigRules(t *testing.T) {
	plan := &nativePlan{resolved: map[string]string{"rustfmt": "rustfmt_config"}}
	foreign := rule.NewRule("filegroup", "assets")
	applyNativeHints("site", foreign, plan)
	if foreign.Attr("aspect_hints") != nil {
		t.Errorf("foreign rule received hints: %v", foreign.AttrStrings("aspect_hints"))
	}
	configRule := rule.NewRule("rustfmt_config", "rustfmt_config")
	applyNativeHints("site", configRule, plan)
	if configRule.Attr("aspect_hints") != nil {
		t.Errorf("native config rule received Rust hints: %v", configRule.AttrStrings("aspect_hints"))
	}
}

func TestNativeRootCanonicalHintSurvives(t *testing.T) {
	existing := rule.NewRule(libraryKind, "root")
	existing.SetAttr("aspect_hints", []string{"//:rustfmt_config"})
	lib := rule.NewRule(libraryKind, "root")
	plan := &nativePlan{
		resolved: map[string]string{"rustfmt": "rustfmt_config"},
		existing: map[string]*rule.Rule{"root": existing, "rustfmt_config": rule.NewRule("rustfmt_config", "rustfmt_config")},
	}
	applyNativeHints("", lib, plan)
	if got := strings.Join(lib.AttrStrings("aspect_hints"), ","); got != "//:rustfmt_config" {
		t.Errorf("root hints = %q, want canonical existing hint", got)
	}
}

func TestNativeHandConfigWins(t *testing.T) {
	build := `load("//quality:native_config.bzl", "rustfmt_config")

rustfmt_config(
    name = "rustfmt_cfg",
    src = "custom.toml",
)
`
	_, result := runNativeGenerate(t, "site", map[string]string{
		"site/src/lib.rs":   "pub fn current() {}\n",
		"site/rustfmt.toml": "edition = \"2021\"\n",
		"site/custom.toml":  "edition = \"2015\"\n",
	}, build)
	if findGenerated(result, "rustfmt_config", "rustfmt_config") != nil {
		t.Error("hand rustfmt_cfg did not suppress generation")
	}
	lib := findGenerated(result, libraryKind, "site")
	if got := strings.Join(lib.AttrStrings("aspect_hints"), ","); got != ":rustfmt_cfg" {
		t.Errorf("library aspect_hints = %q, want the hand target", got)
	}
}

func TestNativeRemoval(t *testing.T) {
	build := `load("//quality:native_config.bzl", "rustfmt_config")
load("@rules_dx//rust/rules:defs.bzl", "rust_library")

rustfmt_config(
    name = "rustfmt_config",
    src = "rustfmt.toml",
)

rust_library(
    name = "site",
    srcs = ["src/lib.rs"],
    crate_name = "site",
    crate_root = "src/lib.rs",
    aspect_hints = [":rustfmt_config"],
)
`
	_, result := runNativeGenerate(t, "site", map[string]string{
		"site/src/lib.rs": "pub fn current() {}\n",
	}, build)
	if !findEmpty(result, "rustfmt_config", "rustfmt_config") {
		t.Error("removed rustfmt.toml did not stub the generated target")
	}
	lib := findGenerated(result, libraryKind, "site")
	if lib.Attr("aspect_hints") != nil {
		t.Errorf("library aspect_hints = %v, want the attribute gone", lib.AttrStrings("aspect_hints"))
	}
}

func TestNativeHandSurvivesRemoval(t *testing.T) {
	build := `load("//quality:native_config.bzl", "rustfmt_config")
load("@rules_dx//rust/rules:defs.bzl", "rust_library")

rustfmt_config(
    name = "rustfmt_config",
    src = "custom.toml",
)

rust_library(
    name = "site",
    srcs = ["src/lib.rs"],
    crate_name = "site",
    crate_root = "src/lib.rs",
    aspect_hints = [":rustfmt_config"],
)
`
	_, result := runNativeGenerate(t, "site", map[string]string{
		"site/src/lib.rs":  "pub fn current() {}\n",
		"site/custom.toml": "[custom]\n",
	}, build)
	if findEmpty(result, "rustfmt_config", "rustfmt_config") {
		t.Error("hand-customized target was stubbed")
	}
	lib := findGenerated(result, libraryKind, "site")
	if got := strings.Join(lib.AttrStrings("aspect_hints"), ","); got != ":rustfmt_config" {
		t.Errorf("library aspect_hints = %q, want the surviving hand target", got)
	}
}

func TestNativeDeselectedOmitsGenerated(t *testing.T) {
	build := `# gazelle:dx_native_tools taplo
load("//quality:native_config.bzl", "rustfmt_config", "taplo_config")

rustfmt_config(
    name = "rustfmt_config",
    src = "rustfmt.toml",
)

taplo_config(
    name = "taplo_cfg",
    src = "custom.toml",
)
`
	_, result := runNativeGenerate(t, "site", map[string]string{
		"site/rustfmt.toml": "edition = \"2021\"\n",
		"site/custom.toml":  "[custom]\n",
	}, build)
	if !findEmpty(result, "rustfmt_config", "rustfmt_config") {
		t.Error("deselected rustfmt did not omit its generated target")
	}
	if findEmpty(result, "taplo_config", "taplo_cfg") {
		t.Error("hand taplo_cfg was stubbed")
	}
	if findGenerated(result, "taplo_config", "taplo_config") != nil {
		t.Error("selected taplo with no taplo.toml still produced a target")
	}
}

func TestNativeAmbiguousFails(t *testing.T) {
	build := `load("//quality:native_config.bzl", "rustfmt_config")

rustfmt_config(
    name = "rustfmt_cfg",
    src = "custom.toml",
)

rustfmt_config(
    name = "rustfmt_extra",
    src = "extra.toml",
)
`
	l, result := runNativeGenerate(t, "site", map[string]string{
		"site/rustfmt.toml": "edition = \"2021\"\n",
	}, build)
	if len(result.Gen) != 0 || len(result.Empty) != 0 {
		t.Error("ambiguous configs produced a partial result")
	}
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "2 config targets") {
		t.Errorf("ambiguous errors = %v", l.errors)
	}
}

func TestNativeConfigOnlyDir(t *testing.T) {
	_, result := runNativeGenerate(t, "site", map[string]string{
		"site/taplo.toml": "[formatting]\n",
	}, "")
	if len(result.Gen) != 3 {
		t.Fatalf("generated %d rules, want taplo_config plus corpus splits", len(result.Gen))
	}
	taplo := findGenerated(result, "taplo_config", "taplo_config")
	if taplo == nil || taplo.AttrString("src") != "taplo.toml" {
		t.Errorf("taplo target = %v", taplo)
	}
	if findGenerated(result, corpusKind, "corpus_toml") == nil {
		t.Error("missing corpus_toml split for taplo.toml")
	}
	if findGenerated(result, corpusKind, "corpus_starlark") == nil {
		t.Error("missing corpus_starlark split for BUILD.bazel")
	}
}

func TestNativeRootVisibility(t *testing.T) {
	_, result := runNativeGenerate(t, "", map[string]string{
		"rustfmt.toml": "edition = \"2021\"\n",
	}, "")
	r := findGenerated(result, "rustfmt_config", "rustfmt_config")
	if r == nil {
		t.Fatal("missing root rustfmt_config")
	}
	if got := strings.Join(r.AttrStrings("visibility"), ","); got != "//:__subpackages__" {
		t.Errorf("root visibility = %q", got)
	}
}

func TestNativeValeStylesVariants(t *testing.T) {
	_, custom := runNativeGenerate(t, "site", map[string]string{
		"site/.vale.ini":             "StylesPath = config/styles\n",
		"site/config/styles/a.yml":   "extends: existence\n",
		"site/config/styles/b/c.yml": "extends: occurrence\n",
	}, "")
	vale := findGenerated(custom, "vale_config", "vale_config")
	if got := strings.Join(vale.AttrStrings("data"), ","); got != "site/config/styles/a.yml,site/config/styles/b/c.yml" {
		t.Errorf("custom styles data = %q", got)
	}
	_, bare := runNativeGenerate(t, "site", map[string]string{
		"site/.vale.ini": "MinAlertLevel = suggestion\n",
	}, "")
	plain := findGenerated(bare, "vale_config", "vale_config")
	if plain == nil {
		t.Fatal("missing self-contained vale_config")
	}
	if plain.Attr("data") != nil {
		t.Errorf("self-contained vale data = %v, want absent", plain.AttrStrings("data"))
	}
}

func TestNativeValeStylesInvalidPaths(t *testing.T) {
	root := t.TempDir()
	vale, ok := nativeToolByID("vale")
	if !ok {
		t.Fatal("missing vale tool definition")
	}
	if got := valeStylesData(root, root, "", vale); got != nil {
		t.Errorf("missing config data = %v, want nil", got)
	}

	for name, styles := range map[string]string{
		"empty":    "",
		"absolute": filepath.Join(root, "outside"),
	} {
		t.Run(name, func(t *testing.T) {
			dir := t.TempDir()
			writeFixture(t, dir, ".vale.ini", "StylesPath = "+styles+"\n")
			if got := valeStylesData(root, dir, "", vale); got != nil {
				t.Errorf("invalid StylesPath data = %v, want nil", got)
			}
		})
	}

	dir := filepath.Join(root, "site")
	outside := filepath.Join(root, "outside")
	writeFixture(t, dir, ".vale.ini", "StylesPath = ../outside\n")
	writeFixture(t, outside, "rule.yml", "extends: existence\n")
	if got := valeStylesData(root, dir, "site", vale); got != nil {
		t.Errorf("escaping StylesPath data = %v, want nil", got)
	}
}

func TestNativeCargoHints(t *testing.T) {
	_, result := runNativeGenerate(t, "app", map[string]string{
		"app/Cargo.toml":   "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
		"app/src/main.rs":  "fn main() {}\n",
		"app/rustfmt.toml": "edition = \"2021\"\n",
	}, "")
	bin := findGenerated(result, binaryKind, "app")
	if bin == nil {
		t.Fatal("missing rust_binary(app)")
	}
	if got := strings.Join(bin.AttrStrings("aspect_hints"), ","); got != ":rustfmt_config" {
		t.Errorf("binary aspect_hints = %q", got)
	}
	if findGenerated(result, "rustfmt_config", "rustfmt_config") == nil {
		t.Error("cargo directory produced no rustfmt_config")
	}
}
