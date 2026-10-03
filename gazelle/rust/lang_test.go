package rust

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

func generateFixture(t *testing.T, files map[string]string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	for name, content := range files {
		writeFixture(t, root, name, content)
	}
	dir := filepath.Join(root, "crates", "demo")
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config: &config.Config{RepoRoot: root},
		Dir:    dir,
		Rel:    "crates/demo",
	})
}

func TestGenerateSourceOnlyCrate(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"crates/demo/src/lib.rs":          "mod model;\nuse local_dep::Thing;\n#[cfg(test)] mod tests;\n",
		"crates/demo/src/model.rs":        "use std::fmt::Debug;\n",
		"crates/demo/src/tests.rs":        "use test_dep::Helper;\n",
		"crates/demo/tests/common.rs":     "use demo::Thing;\n",
		"crates/demo/tests/helper/mod.rs": "use ignored_nested_root::Nope;\n",
	})
	if len(result.Gen) != 4 || len(result.Imports) != 4 {
		t.Fatalf("generated %d rules and %d import sets, want 4 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != libraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "src/lib.rs,src/model.rs,src/tests.rs" {
		t.Errorf("library srcs = %q", got)
	}
	if lib.AttrString("crate_root") != "src/lib.rs" || lib.AttrString("crate_name") != "demo" {
		t.Errorf("library attributes: root=%q crate=%q", lib.AttrString("crate_root"), lib.AttrString("crate_name"))
	}
	unit := result.Gen[1]
	if unit.Kind() != testKind || unit.Name() != "demo_test" || unit.AttrString("crate") != ":demo" || len(unit.AttrStrings("srcs")) != 0 {
		t.Errorf("unit test = %s(%s), crate=%q srcs=%v", unit.Kind(), unit.Name(), unit.AttrString("crate"), unit.AttrStrings("srcs"))
	}
	integration := result.Gen[2]
	if integration.Kind() != testKind || integration.Name() != "common_test" {
		t.Errorf("integration test = %s(%s)", integration.Kind(), integration.Name())
	}
	libImports := result.Imports[0].(targetImports)
	if strings.Join(libImports.production, ",") != "local_dep" || strings.Join(libImports.test, ",") != "test_dep" {
		t.Errorf("library imports = %+v", libImports)
	}
	unitImports := result.Imports[1].(targetImports)
	if len(unitImports.production) != 0 || strings.Join(unitImports.test, ",") != "test_dep" {
		t.Errorf("unit imports = %+v", unitImports)
	}
}

func TestGenerateSourceOnlyBinaryUnitTest(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"crates/demo/src/main.rs": "#[test]\nfn works() {}\n",
	})
	if len(result.Gen) != 3 || result.Gen[0].Kind() != binaryKind || result.Gen[1].AttrString("crate") != ":demo" {
		t.Errorf("binary unit-test generation = %+v", result.Gen)
	}
}

func TestGenerateSourceOnlyFailures(t *testing.T) {
	cases := []struct {
		files map[string]string
		want  string
	}{
		{map[string]string{"crates/demo/src/lib.rs": "mod missing;\n"}, "orphan module"},
		{map[string]string{
			"crates/demo/src/lib.rs":    "mod shared;\n",
			"crates/demo/src/main.rs":   "#[path = \"shared.rs\"] mod shared;\n",
			"crates/demo/src/shared.rs": "",
		}, "owned by both"},
		{map[string]string{
			"crates/demo/src/lib.rs":    "#[test]\nfn works() {}\n",
			"crates/demo/tests/demo.rs": "",
		}, "normalized name"},
	}
	for _, tc := range cases {
		root := t.TempDir()
		for name, content := range tc.files {
			writeFixture(t, root, name, content)
		}
		l := &rustLang{}
		l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, "crates/demo"), Rel: "crates/demo"})
		if len(l.errors) != 1 || !strings.Contains(l.errors[0], tc.want) {
			t.Errorf("files %v errors = %v, want %q", tc.files, l.errors, tc.want)
		}
	}
}

func TestGenerateStaleRules(t *testing.T) {
	f := rule.EmptyFile("BUILD.bazel", "crates/demo")
	f.Rules = append(f.Rules,
		rule.NewRule(libraryKind, "old"),
		rule.NewRule("filegroup", "keep"),
	)
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config: &config.Config{RepoRoot: t.TempDir()},
		Dir:    t.TempDir(),
		Rel:    "crates/demo",
		File:   f,
	})
	if len(result.Empty) != 1 || result.Empty[0].Kind() != libraryKind || result.Empty[0].Name() != "old" {
		t.Fatalf("empty rules = %v", result.Empty)
	}
}

func TestImportsIndexesOnlyLibraries(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(libraryKind, "demo")
	lib.SetAttr("crate_name", "cargo_name")
	imports := lang.Imports(&config.Config{}, lib, nil)
	if len(imports) != 1 || imports[0].Lang != languageName || imports[0].Imp != "cargo_name" {
		t.Errorf("imports = %+v", imports)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(testKind, "demo_test"), nil); got != nil {
		t.Errorf("test imports = %+v, want nil", got)
	}
	unnamed := rule.NewRule(libraryKind, "fallback")
	if got := lang.Imports(&config.Config{}, unnamed, nil); len(got) != 1 || got[0].Imp != "fallback" {
		t.Errorf("fallback imports = %+v", got)
	}
}

func stubExitProcess(t *testing.T) *int {
	t.Helper()
	old := exitProcess
	code := -1
	exitProcess = func(c int) { code = c }
	t.Cleanup(func() { exitProcess = old })
	return &code
}

func TestErrorsAbortBeforeEmission(t *testing.T) {
	code := stubExitProcess(t)
	l := &rustLang{}
	l.Before(context.Background())
	l.fail("second")
	l.fail("first")
	l.AfterResolvingDeps(context.Background())
	if *code != 1 {
		t.Fatalf("exit code = %d, want 1", *code)
	}
	if got := strings.Join(l.errors, "\n"); got != "first\nsecond" {
		t.Errorf("errors were not deterministic: %q", got)
	}
}

func TestBeforeRecordsRecorderError(t *testing.T) {
	out := filepath.Join(t.TempDir(), "intended.json")
	t.Setenv(common.EnvIntendedManifest, out)
	t.Setenv(common.EnvGenerateMode, "print")
	l := &rustLang{}
	l.Before(context.Background())
	if l.manifest != nil {
		t.Fatal("Before with bad mode sets recorder")
	}
	if got := strings.Join(l.errors, "\n"); !strings.Contains(got, `must be "check" or "default"`) {
		t.Fatalf("errors = %q, want mode error", got)
	}
}

func TestAfterResolvingDepsReportsManifestError(t *testing.T) {
	code := stubExitProcess(t)
	l := &rustLang{}
	l.manifest = &common.ManifestRecorder{
		Prefix:        "rust",
		OutPath:       filepath.Join(t.TempDir(), "missing", "intended.json"),
		Mode:          "default",
		Scopes:        []common.ScopeElement{{Element: "//...", Dirs: []string{""}}},
		ApparentLoads: l.ApparentLoads,
	}
	l.AfterResolvingDeps(context.Background())
	if *code != 1 {
		t.Fatalf("exit code = %d, want 1", *code)
	}
	if got := strings.Join(l.errors, "\n"); !strings.Contains(got, "cannot write intended manifest") {
		t.Errorf("errors = %q, want write error", got)
	}
}

func TestIgnoreDirectiveInheritanceAndStaleCheck(t *testing.T) {
	l := &rustLang{}
	l.Before(context.Background())
	root := config.New()
	file, err := rule.LoadData("BUILD.bazel", "", []byte("# gazelle:dx_ignore_import rust missing_crate\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(root, "", file)
	child := root.Clone()
	l.Configure(child, "child", nil)
	ignore := matchingIgnore(child, "missing_crate")
	if ignore == nil || ignore.path != "" {
		t.Fatalf("inherited ignore = %+v", ignore)
	}
	ignore.used = true
	l.AfterResolvingDeps(context.Background())
}

func TestStaleIgnoreFails(t *testing.T) {
	l := &rustLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import rust stale\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	code := stubExitProcess(t)
	l.AfterResolvingDeps(context.Background())
	if *code != 1 {
		t.Fatalf("exit code = %d, want 1", *code)
	}
	if got := strings.Join(l.errors, "\n"); !strings.Contains(got, "stale") {
		t.Errorf("stale ignore errors = %q, want stale", got)
	}
}

func TestMalformedIgnoreFails(t *testing.T) {
	l := &rustLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:dx_ignore_import rust\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	code := stubExitProcess(t)
	l.AfterResolvingDeps(context.Background())
	if *code != 1 {
		t.Fatalf("exit code = %d, want 1", *code)
	}
	if got := strings.Join(l.errors, "\n"); !strings.Contains(got, "malformed") {
		t.Errorf("malformed ignore errors = %q, want malformed", got)
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := &rustLang{}
	if l.Name() != "rust" || len(l.Kinds()) != 12 || l.CheckFlags(flag.NewFlagSet("test", flag.ContinueOnError), config.New()) != nil {
		t.Fatalf("invalid language metadata")
	}
	l.RegisterFlags(flag.NewFlagSet("test", flag.ContinueOnError), "update", config.New())
	if strings.Join(l.KnownDirectives(), ",") != "dx_ignore_import,dx_native_tools" || l.Embeds(nil, label.NoLabel) != nil {
		t.Fatal("invalid directives or embeds")
	}
	loads := l.ApparentLoads(func(name string) string {
		if name == "rules_dx" {
			return "renamed_dx"
		}
		if name == "crates" {
			return "renamed_crates"
		}
		return ""
	})
	if loads[0].Name != "@renamed_dx//rust/rules:defs.bzl" || loads[1].Name != "@rules_rust//cargo:defs.bzl" || loads[2].Name != "@renamed_crates//:crates.bzl" {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := l.Loads(); defaults[0].Name != "@rules_dx//rust/rules:defs.bzl" || defaults[1].Name != "@rules_rust//cargo:defs.bzl" || defaults[3].Name != "@rules_dx//quality:native_config.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
	defaultApparent := l.ApparentLoads(func(string) string { return "" })
	if defaultApparent[0].Name != "@rules_dx//rust/rules:defs.bzl" || defaultApparent[1].Name != "@rules_rust//cargo:defs.bzl" || defaultApparent[2].Name != "@crates//:crates.bzl" {
		t.Errorf("default apparent loads = %+v", defaultApparent)
	}
	l.DoneGeneratingRules()
}

func TestConfigureDirectiveForms(t *testing.T) {
	l := &rustLang{}
	l.Before(context.Background())
	cfg := config.New()
	file, err := rule.LoadData("BUILD.bazel", "pkg", []byte("# gazelle:other ignored\n# gazelle:dx_ignore_import rust rust external\n# gazelle:dx_ignore_import python foreign\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "pkg", file)
	if len(l.ignores) != 1 || l.ignores[0].value != "external" {
		t.Errorf("ignores = %+v", l.ignores)
	}
	l.ignores[0].used = true
	l.AfterResolvingDeps(context.Background())
	if matchingIgnore(config.New(), "none") != nil || matchingIgnore(cfg, "none") != nil {
		t.Error("nonmatching ignore found")
	}
}

func TestGenerateCargoAPIAndFailures(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "crates/app/Cargo.toml", "[package]\nname = \"app\"\nedition = \"2021\"\n[dependencies]\nserde_json = \"1\"\n[dev-dependencies]\ntempfile = \"3\"\n[lib]\npath = \"src/lib.rs\"\n")
	writeFixture(t, root, "crates/app/src/lib.rs", "use serde_json::Value;\n#[cfg(test)] use tempfile::tempdir;\n")
	l := &rustLang{}
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "crates/app"),
		Rel:          "crates/app",
		RegularFiles: []string{"Cargo.toml"},
	})
	if len(l.errors) != 0 || len(result.Gen) != 4 {
		t.Fatalf("cargo generation errors=%v result=%+v", l.errors, result)
	}
	if result.Gen[0].AttrString("edition") != "2021" || result.Gen[1].AttrString("crate") != ":app" {
		t.Errorf("cargo rules = %+v", result.Gen)
	}
	if result.Gen[2].Kind() != corpusKind || result.Gen[3].Kind() != corpusKind {
		t.Errorf("corpus splits = %+v, want starlark plus toml", result.Gen[2:])
	}

	badRoot := t.TempDir()
	writeFixture(t, badRoot, "Cargo.toml", "[package]\nname = \"bad\"\n[[test]]\nname = \"missing\"\npath = \"tests/missing.rs\"\n")
	bad := &rustLang{}
	bad.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: badRoot}, Dir: badRoot, RegularFiles: []string{"Cargo.toml"}})
	if len(bad.errors) != 1 || !strings.Contains(bad.errors[0], "does not exist") {
		t.Errorf("missing target errors = %v", bad.errors)
	}

	malformed := t.TempDir()
	writeFixture(t, malformed, "Cargo.toml", "[package]\nname = nope\n")
	broken := &rustLang{}
	broken.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: malformed}, Dir: malformed, RegularFiles: []string{"Cargo.toml"}})
	if len(broken.errors) != 1 || !strings.Contains(broken.errors[0], "quoted string") {
		t.Errorf("manifest errors = %v", broken.errors)
	}

	missingManifest := &rustLang{}
	missingManifest.generateCargo(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, "missing"), Rel: "missing"}, nil, &nativePlan{})
	if len(missingManifest.errors) != 1 || !strings.Contains(missingManifest.errors[0], "Cargo.toml") {
		t.Errorf("missing manifest errors = %v", missingManifest.errors)
	}

	missingRoot := t.TempDir()
	writeFixture(t, missingRoot, "Cargo.toml", "[package]\nname = \"app\"\n[[test]]\nname = \"missing\"\npath = \"tests/missing.rs\"\n")
	missing := &rustLang{}
	missing.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: missingRoot}, Dir: missingRoot, RegularFiles: []string{"Cargo.toml"}})
	if len(missing.errors) != 1 || !strings.Contains(missing.errors[0], "does not exist") {
		t.Errorf("missing root errors = %v", missing.errors)
	}
}

func TestGenerateCargoOwnershipAndImportFailures(t *testing.T) {
	cases := []struct {
		manifest string
		files    map[string]string
		want     string
	}{
		{
			manifest: "[package]\nname = \"app\"\n[lib]\npath = \"src/lib.rs\"\n[[bin]]\nname = \"tool\"\npath = \"src/lib.rs\"\n",
			files:    map[string]string{"src/lib.rs": ""},
			want:     "one source may have only one owner",
		},
		{
			manifest: "[package]\nname = \"app\"\n[lib]\npath = \"src/lib.rs\"\n",
			files:    map[string]string{"src/lib.rs": "use undeclared::Thing;\n"},
			want:     "unresolved production import",
		},
		{
			manifest: "[package]\nname = \"app\"\n[lib]\npath = \"src/lib.rs\"\n",
			files:    map[string]string{"src/lib.rs": "#[cfg(test)] use undeclared::Thing;\n"},
			want:     "unresolved test import",
		},
		{
			manifest: "[package]\nname = \"app\"\n[lib]\npath = \"src/lib.rs\"\n",
			files:    map[string]string{"src/lib.rs": "mod missing;\n"},
			want:     "orphan module",
		},
		{
			manifest: "[package]\nname = \"app\"\n[lib]\nname = \"---\"\n",
			files:    map[string]string{"src/lib.rs": ""},
			want:     "empty target name",
		},
		{
			manifest: "[package]\nname = \"app\"\n[lib]\npath = \"src/lib.rs\"\n[[test]]\nname = \"other\"\npath = \"tests/other.rs\"\n",
			files: map[string]string{
				"src/lib.rs":     "#[path = \"../shared.rs\"] mod shared;\n",
				"tests/other.rs": "#[path = \"../shared.rs\"] mod shared;\n",
				"shared.rs":      "",
			},
			want: "owned by both",
		},
	}
	for _, tc := range cases {
		root := t.TempDir()
		writeFixture(t, root, "Cargo.toml", tc.manifest)
		for name, content := range tc.files {
			writeFixture(t, root, name, content)
		}
		l := &rustLang{}
		l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}})
		if len(l.errors) == 0 || !strings.Contains(strings.Join(l.errors, "\n"), tc.want) {
			t.Errorf("errors = %v, want %q", l.errors, tc.want)
		}
	}
}

func TestGenerateCargoCustomHarness(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "Cargo.toml", "[package]\nname = \"app\"\n[[test]]\nname = \"custom\"\npath = \"tests/custom.rs\"\nharness = false\n")
	writeFixture(t, root, "tests/custom.rs", "fn main() {}\n")
	l := &rustLang{}
	result := l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}})
	if len(l.errors) != 0 || len(result.Gen) != 3 || result.Gen[0].Attr("use_libtest_harness") == nil {
		t.Errorf("custom harness generation errors=%v result=%+v", l.errors, result.Gen)
	}
}

func TestGenerateCargoLibBinTakeover(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "Cargo.toml", "[package]\nname = \"demo\"\n[lib]\nname = \"demo\"\npath = \"src/lib.rs\"\n[[bin]]\nname = \"demo\"\npath = \"src/main.rs\"\n")
	writeFixture(t, root, "src/lib.rs", "#[test]\nfn probe() {}\n")
	writeFixture(t, root, "src/main.rs", "fn main() {}\n#[test]\nfn bint() {}\n")
	l := &rustLang{}
	result := l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}})
	if len(l.errors) != 0 {
		t.Fatalf("generation errors = %v", l.errors)
	}
	var names []string
	for _, r := range result.Gen {
		names = append(names, r.Kind()+":"+r.Name())
	}
	want := []string{libraryKind + ":demo_lib", testKind + ":demo_test", binaryKind + ":demo", testKind + ":demo_bin_test", corpusKind + ":corpus_starlark", corpusKind + ":corpus_toml"}
	if strings.Join(names, ",") != strings.Join(want, ",") {
		t.Fatalf("generated = %v, want %v", names, want)
	}
	if got := result.Gen[0].AttrString("crate_name"); got != "demo" {
		t.Errorf("lib crate_name = %q, want demo", got)
	}
	if got := result.Gen[1].AttrString("crate"); got != ":demo_lib" {
		t.Errorf("lib test crate = %q, want :demo_lib", got)
	}
	if got := result.Gen[3].AttrString("crate"); got != ":demo" {
		t.Errorf("bin test crate = %q, want :demo", got)
	}
	binImports, ok := result.Imports[2].(targetImports)
	if len(result.Imports) != 6 || !ok || binImports.siblingLib != "demo_lib" {
		t.Fatalf("bin imports = %+v, want sibling demo_lib", result.Imports)
	}
	bin := rule.NewRule(binaryKind, "demo")
	l.Resolve(resolverConfig(t, nil), resolverIndex(l), nil, bin, binImports, label.New("", "pkg", "demo"))
	if got := strings.Join(bin.AttrStrings("deps"), ","); got != ":demo_lib" {
		t.Errorf("bin deps = %q, want :demo_lib", got)
	}
}

func TestGenerateCargoTestLinksSiblingAndMirror(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "Cargo.toml", "[package]\nname = \"app\"\n[dependencies]\nlocal = { path = \"../local\" }\n")
	writeFixture(t, root, "src/lib.rs", "#[test]\nfn probe() {}\n")
	writeFixture(t, root, "tests/smoke.rs", "")
	l := &rustLang{}
	result := l.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root, RegularFiles: []string{"Cargo.toml"}})
	if len(l.errors) != 0 {
		t.Fatalf("generation errors = %v", l.errors)
	}
	var libName string
	for _, r := range result.Gen {
		if r.Kind() == libraryKind {
			libName = r.Name()
		}
	}
	if libName == "" {
		t.Fatalf("no library generated: %+v", result.Gen)
	}
	for i, r := range result.Gen {
		if r.Kind() != testKind {
			continue
		}
		imports, ok := result.Imports[i].(targetImports)
		if !ok {
			t.Fatalf("test imports = %T, want targetImports", result.Imports[i])
		}
		if r.AttrString("crate") != "" {
			if len(imports.mirrorPaths) != 0 || imports.siblingLib != "" {
				t.Errorf("wrapper imports = %+v, want no mirror or sibling", imports)
			}
			continue
		}
		if imports.siblingLib != libName {
			t.Errorf("integration sibling = %q, want %q", imports.siblingLib, libName)
		}
		if strings.Join(imports.mirrorPaths, ",") != "local" {
			t.Errorf("integration mirror = %q, want [local]", imports.mirrorPaths)
		}
	}
}

func TestLocalCargoImportsDevProductionAndMirror(t *testing.T) {
	manifest := &cargoManifest{
		packageName: "app",
		normalDeps: map[string]cargoDependency{
			"local": {depPath: "../local"},
			"ext":   {external: true, version: "1", depPath: "helper"},
		},
		devDeps: map[string]cargoDependency{
			"local_dev": {depPath: "../local-dev"},
		},
	}
	imports := targetImports{production: []string{"local_dev"}}
	withDev := localCargoImports(&config.Config{}, manifest, imports, true)
	if strings.Join(withDev.production, ",") != "local_dev" {
		t.Errorf("dev production = %+v, want [local_dev]", withDev.production)
	}
	if strings.Join(withDev.mirrorPaths, ",") != "local,local_dev" {
		t.Errorf("dev mirror = %q, want [local local_dev]", withDev.mirrorPaths)
	}
	withoutDev := localCargoImports(&config.Config{}, manifest, imports, false)
	if len(withoutDev.production) != 0 {
		t.Errorf("production local imports include dev: %+v", withoutDev.production)
	}
	if strings.Join(withoutDev.mirrorPaths, ",") != "local" {
		t.Errorf("production mirror = %q, want [local]", withoutDev.mirrorPaths)
	}
}
