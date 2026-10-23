package gleam

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func generate(t *testing.T, lang language.Language, files map[string]string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "app")
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
	return lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/app",
		RegularFiles: regular,
	})
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
	if len(result.Gen) != 2 {
		t.Fatalf("generated %d rules, want 2", len(result.Gen))
	}
	lib, test := result.Gen[0], result.Gen[1]
	if lib.Kind() != LibraryKind || lib.Name() != "app" || strings.Join(lib.AttrStrings("srcs"), ",") != "src/app.gleam,src/app/core.gleam" {
		t.Errorf("library = %s(%s, %v)", lib.Kind(), lib.Name(), lib.AttrStrings("srcs"))
	}
	if test.Kind() != TestKind || test.Name() != "app_test" || strings.Join(test.AttrStrings("srcs"), ",") != "test/app_test.gleam" {
		t.Errorf("test = %s(%s, %v)", test.Kind(), test.Name(), test.AttrStrings("srcs"))
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "gleam/io,other/thing" {
		t.Errorf("library imports = %v", got)
	}
	if got := result.Imports[1].(common.ImportSet).Names; strings.Join(got, ",") != "app/core,gleeunit" {
		t.Errorf("test imports = %v", got)
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
}

func TestLanguageMetadata(t *testing.T) {
	lang := NewLanguage().(*gleamLang)
	if lang.Name() != languageName || len(lang.Kinds()) != 2 {
		t.Fatalf("metadata = %s / %v", lang.Name(), lang.Kinds())
	}
	loads := lang.Loads()
	if len(loads) != 1 || loads[0].Name != "@rules_dx//gleam/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "gleam_library,gleam_test" {
		t.Errorf("loads = %+v", loads)
	}
}
