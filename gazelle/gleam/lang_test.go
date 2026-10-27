package gleam

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

func writePackage(t *testing.T, rel string, files map[string]string) (string, string, []string) {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, filepath.FromSlash(rel))
	var regular []string
	for name, content := range files {
		path := filepath.Join(dir, name)
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(name, "/") {
			regular = append(regular, name)
		}
	}
	return root, dir, regular
}

func generateArgs(t *testing.T, lang language.Language, rel string, file *rule.File, files map[string]string, setup func(dir string)) language.GenerateResult {
	t.Helper()
	root, dir, regular := writePackage(t, rel, files)
	if setup != nil {
		setup(dir)
	}
	return lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          rel,
		File:         file,
		RegularFiles: regular,
	})
}

func generate(t *testing.T, lang language.Language, files map[string]string) language.GenerateResult {
	t.Helper()
	return generateArgs(t, lang, "pkg/app", nil, files, nil)
}

var appPackage = map[string]string{
	"gleam.toml":          "name = \"app\"\n",
	"src/app.gleam":       "import gleam/io\n",
	"test/app_test.gleam": "import gleeunit\n",
}

func buildFile(t *testing.T, content string) *rule.File {
	t.Helper()
	f, err := rule.LoadData("pkg/app/BUILD.bazel", "pkg/app", []byte(content))
	if err != nil {
		t.Fatal(err)
	}
	return f
}

func expectFailure(t *testing.T, lang language.Language, result language.GenerateResult, want string) {
	t.Helper()
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want none", len(result.Gen))
	}
	gl := lang.(*gleamLang)
	if !gl.Failed() {
		t.Fatal("generation did not fail")
	}
	gl.AfterResolvingDeps(context.Background())
	msg := strings.Join(common.Messages(common.Errors.Take()), "\n")
	if !strings.Contains(msg, want) {
		t.Fatalf("errors = %q, want it to contain %q", msg, want)
	}
}

func TestGenerateFailsOnNameClaimedByHandwrittenRule(t *testing.T) {
	lang := NewLanguage()
	file := buildFile(t, "gleam_binary(name = \"app\")\n")
	result := generateArgs(t, lang, "pkg/app", file, appPackage, nil)
	expectFailure(t, lang, result, `"app" is claimed by generated gleam_library(pkg/app) and existing gleam_binary`)
}

func TestGenerateFailsOnTestNameClaimedByHandwrittenRule(t *testing.T) {
	lang := NewLanguage()
	file := buildFile(t, "genrule(name = \"app_test\")\n")
	result := generateArgs(t, lang, "pkg/app", file, appPackage, nil)
	expectFailure(t, lang, result, `"app_test" is claimed by generated gleam_test(pkg/app) and existing genrule`)
}

func TestGenerateAllowsExistingGeneratedKinds(t *testing.T) {
	lang := NewLanguage()
	file := buildFile(t, "gleam_library(name = \"app\")\n\ngleam_test(name = \"app_test\")\n")
	result := generateArgs(t, lang, "pkg/app", file, appPackage, nil)
	if len(result.Gen) != 2 || lang.(*gleamLang).Failed() {
		t.Fatalf("generated %d rules, failed = %v; want 2 rules and no failure", len(result.Gen), lang.(*gleamLang).Failed())
	}
}

func TestGenerateFailsOnUnreadableSource(t *testing.T) {
	lang := NewLanguage()
	result := generateArgs(t, lang, "pkg/app", nil, appPackage, func(dir string) {
		if err := os.Symlink(filepath.Join(dir, "missing.gleam"), filepath.Join(dir, "src", "broken.gleam")); err != nil {
			t.Fatal(err)
		}
	})
	expectFailure(t, lang, result, "gleam: pkg/app: read src/broken.gleam")
}

func TestGenerateFixturePathIsTestonly(t *testing.T) {
	result := generateArgs(t, NewLanguage(), "tests/fixtures/app", nil, appPackage, nil)
	if len(result.Gen) != 2 {
		t.Fatalf("generated %d rules, want 2", len(result.Gen))
	}
	lib, test := result.Gen[0], result.Gen[1]
	if lib.Kind() != LibraryKind || lib.Name() != "app" || !lib.AttrBool("testonly") {
		t.Errorf("library %s(%s) testonly = %v, want true", lib.Kind(), lib.Name(), lib.AttrBool("testonly"))
	}
	if test.Kind() != TestKind || test.Attr("testonly") != nil {
		t.Errorf("test %s(%s) has testonly attr; the test forwarder sets it", test.Kind(), test.Name())
	}
	plain := generate(t, NewLanguage(), appPackage)
	if len(plain.Gen) != 2 || plain.Gen[0].Attr("testonly") != nil {
		t.Errorf("pkg/app library testonly = %v, want unset", plain.Gen)
	}
}

func TestGenerateLibraryAndTest(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"gleam.toml":          "name = \"app\"\n",
		"src/app.gleam":       "import app/core\nimport gleam/io\n",
		"src/app/core.gleam":  "import other/thing\n",
		"test/app_test.gleam": "import app/core\nimport gleeunit\nimport helper\n",
		"test/helper.gleam":   "",
	})
	if len(result.Gen) != 3 {
		t.Fatalf("generated %d rules, want 3", len(result.Gen))
	}
	lib, support, test := result.Gen[0], result.Gen[1], result.Gen[2]
	if lib.Kind() != LibraryKind || lib.Name() != "app" || strings.Join(lib.AttrStrings("srcs"), ",") != "src/app.gleam,src/app/core.gleam" {
		t.Errorf("library = %s(%s, %v)", lib.Kind(), lib.Name(), lib.AttrStrings("srcs"))
	}
	if support.Kind() != TestLibraryKind || support.Name() != "app_test_support" || strings.Join(support.AttrStrings("srcs"), ",") != "test/helper.gleam" {
		t.Errorf("support = %s(%s, %v)", support.Kind(), support.Name(), support.AttrStrings("srcs"))
	}
	if test.Kind() != TestKind || test.Name() != "app_test" || strings.Join(test.AttrStrings("srcs"), ",") != "test/app_test.gleam" || strings.Join(test.AttrStrings("deps"), ",") != ":app_test_support" {
		t.Errorf("test = %s(%s, %v, deps %v)", test.Kind(), test.Name(), test.AttrStrings("srcs"), test.AttrStrings("deps"))
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "gleam/io,other/thing" {
		t.Errorf("library imports = %v", got)
	}
	if got := result.Imports[1].(common.ImportSet).Names; len(got) != 0 {
		t.Errorf("support imports = %v", got)
	}
	if got := result.Imports[2].(common.ImportSet).Names; strings.Join(got, ",") != "app/core,gleeunit" {
		t.Errorf("test imports = %v", got)
	}
}

func TestGenerateUnknownTestImportStaysUnresolved(t *testing.T) {
	result := generate(t, NewLanguage(), map[string]string{
		"gleam.toml":          "name = \"app\"\n",
		"test/app_test.gleam": "import gleeunit\nimport missing\n",
	})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != TestKind {
		t.Fatalf("rules = %v, want one gleam_test", result.Gen)
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "gleeunit,missing" {
		t.Errorf("test imports = %v", got)
	}
}

func TestGenerateHelpersWithoutTests(t *testing.T) {
	result := generate(t, NewLanguage(), map[string]string{
		"gleam.toml":        "name = \"app\"\n",
		"src/app.gleam":     "",
		"test/helper.gleam": "",
	})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind {
		t.Fatalf("rules = %v, want only gleam_library", result.Gen)
	}
}

func TestGenerateFailsOnSupportNameClaimed(t *testing.T) {
	lang := NewLanguage()
	file := buildFile(t, "filegroup(name = \"app_test_support\")\n")
	files := map[string]string{"test/helper.gleam": ""}
	for name, content := range appPackage {
		files[name] = content
	}
	result := generateArgs(t, lang, "pkg/app", file, files, nil)
	expectFailure(t, lang, result, `"app_test_support" is claimed by generated gleam_test_library(pkg/app) and existing filegroup`)
}

func TestGenerateHelperImportingTestStaysUnresolved(t *testing.T) {
	result := generate(t, NewLanguage(), map[string]string{
		"gleam.toml":          "name = \"app\"\n",
		"test/app_test.gleam": "import helper\n",
		"test/helper.gleam":   "import app_test\nimport gleeunit/should\n",
	})
	if len(result.Gen) != 2 || result.Gen[0].Kind() != TestLibraryKind {
		t.Fatalf("rules = %v, want gleam_test_library and gleam_test", result.Gen)
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "app_test,gleeunit/should" {
		t.Errorf("support imports = %v", got)
	}
}

func TestGenerateSkipsDirectoriesWithoutManifest(t *testing.T) {
	result := generate(t, NewLanguage(), map[string]string{"src/app.gleam": ""})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules without gleam.toml", len(result.Gen))
	}
}

func TestGenerateSkipsNestedPackages(t *testing.T) {
	result := generate(t, NewLanguage(), map[string]string{
		"gleam.toml":              "",
		"src/app.gleam":           "",
		"src/vendored/gleam.toml": "",
		"src/vendored/dep.gleam":  "",
	})
	if len(result.Gen) != 1 || strings.Join(result.Gen[0].AttrStrings("srcs"), ",") != "src/app.gleam" {
		t.Fatalf("rules = %v", result.Gen)
	}
}

func TestImportsAreModulePaths(t *testing.T) {
	lib := rule.NewRule(LibraryKind, "app")
	lib.SetAttr("srcs", []string{"src/app.gleam", "src/app/core.gleam", "src/ffi.mjs"})
	got := NewLanguage().Imports(nil, lib, nil)
	if len(got) != 2 || got[0].Imp != "app" || got[1].Imp != "app/core" || got[0].Lang != languageName {
		t.Errorf("imports = %+v", got)
	}
	if NewLanguage().Imports(nil, rule.NewRule(TestKind, "t"), nil) != nil {
		t.Error("tests export nothing")
	}
	support := rule.NewRule(TestLibraryKind, "app_test_support")
	support.SetAttr("srcs", []string{"test/helper.gleam"})
	if got := NewLanguage().Imports(nil, support, nil); len(got) != 0 {
		t.Errorf("test libraries export nothing, got %+v", got)
	}
}

func TestLanguageMetadata(t *testing.T) {
	lang := NewLanguage().(*gleamLang)
	if lang.Name() != languageName || len(lang.Kinds()) != 3 {
		t.Fatalf("metadata = %s / %v", lang.Name(), lang.Kinds())
	}
	loads := lang.Loads()
	if len(loads) != 1 || loads[0].Name != "@rules_dx//gleam/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "gleam_library,gleam_test,gleam_test_library" {
		t.Errorf("loads = %+v", loads)
	}
}

func TestGenerateRootPackage(t *testing.T) {
	lang := NewLanguage()
	result := generateArgs(t, lang, "", nil, map[string]string{
		"gleam.toml":          "name = \"app\"\n\n[dependencies]\nname = \"decoy\"\n",
		"src/app.gleam":       "pub fn name() -> String { \"app\" }\n",
		"test/app_test.gleam": "import app\nimport gleeunit\n",
	}, nil)
	if lang.(*gleamLang).Failed() || len(result.Gen) != 2 {
		t.Fatalf("generated %d rules (failed=%v), want gleam_library and gleam_test", len(result.Gen), lang.(*gleamLang).Failed())
	}
	if lib := result.Gen[0]; lib.Kind() != LibraryKind || lib.Name() != "app" {
		t.Fatalf("library = %s(%s), want gleam_library(app)", lib.Kind(), lib.Name())
	}
	if test := result.Gen[1]; test.Kind() != TestKind || test.Name() != "app_test" {
		t.Fatalf("test = %s(%s), want gleam_test(app_test)", test.Kind(), test.Name())
	}
}
