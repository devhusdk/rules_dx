package d

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

func generate(t *testing.T, lang language.Language, rel string, files map[string]string, build string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, filepath.FromSlash(rel))
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
		Rel:          rel,
		RegularFiles: regular,
	}
	if build != "" {
		f, err := rule.LoadData(filepath.Join(dir, "BUILD.bazel"), rel, []byte(build))
		if err != nil {
			t.Fatal(err)
		}
		args.File = f
	}
	return lang.GenerateRules(args)
}

func failed(lang language.Language) bool { return lang.(*dLang).Failed() }

func errorsOf(lang language.Language) string {
	return strings.Join(lang.(*dLang).Errors(), "\n")
}

func TestGenerateSetsImportsRootAndSkipsStdlib(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, "src/app/cli", map[string]string{
		"cli.d": "module app.cli.cli;\nimport std.stdio;\nimport app.core.text;\n",
	}, "")
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules: %s", len(result.Gen), errorsOf(lang))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "cli" {
		t.Fatalf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("imports"), ","); got != "../.." {
		t.Errorf("imports = %q, want ../..", got)
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "app.core.text" {
		t.Errorf("imports = %v", got)
	}
}

func TestGenerateOmitsImportsAtPackageRoot(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, "tools/x", map[string]string{"util.d": "import std.stdio;\n"}, "")
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("errors: %s", errorsOf(lang))
	}
	if result.Gen[0].Attr("imports") != nil {
		t.Errorf("imports = %v, want none", result.Gen[0].AttrStrings("imports"))
	}
}

func TestGenerateSkipsForeignDFiles(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, "native", map[string]string{
		"probes.d": "provider app {\n  probe start(int);\n};\n",
		"deps.d":   "build/foo.o: foo.c foo.h\n",
		"foo.c":    "int x;\n",
	}, "")
	if failed(lang) || len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, errors: %s", len(result.Gen), errorsOf(lang))
	}
}

func TestGenerateFailsOnSyntaxError(t *testing.T) {
	lang := NewLanguage()
	generate(t, lang, "pkg", map[string]string{"a.d": "module a;\nint x = 1 +;\n"}, "")
	if !failed(lang) || !strings.Contains(errorsOf(lang), "a.d: syntax error at line 2") || !strings.Contains(errorsOf(lang), "# gazelle:exclude a.d") {
		t.Fatalf("errors = %q, want a syntax error with an exclude hint", errorsOf(lang))
	}
}

func TestGenerateFailsOnPathMismatch(t *testing.T) {
	lang := NewLanguage()
	generate(t, lang, "src/app", map[string]string{"a.d": "module other.a;\n"}, "")
	if !failed(lang) || !strings.Contains(errorsOf(lang), "module other.a in src/app/a.d does not match its path") {
		t.Fatalf("errors = %q", errorsOf(lang))
	}
}

func TestGenerateFailsOnMain(t *testing.T) {
	lang := NewLanguage()
	generate(t, lang, "pkg", map[string]string{"m.d": "void main() {}\n"}, "")
	if !failed(lang) || !strings.Contains(errorsOf(lang), "defines main") {
		t.Fatalf("errors = %q", errorsOf(lang))
	}
}

func TestStringImports(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, "pkg", map[string]string{
		"a.d":      "enum t = import(\"data.txt\");\n",
		"data.txt": "x\n",
	}, "")
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("errors: %s", errorsOf(lang))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("string_srcs"), ","); got != "data.txt" {
		t.Errorf("string_srcs = %q", got)
	}
	for name, tc := range map[string]struct {
		src  string
		want string
	}{
		"outside": {"enum t = import(\"../x.txt\");\n", "outside this directory"},
		"missing": {"enum t = import(\"nope.txt\");\n", "missing from this directory"},
		"dynamic": {"enum t = import(name);\n", "not a string literal"},
	} {
		lang := NewLanguage()
		generate(t, lang, "pkg", map[string]string{"a.d": tc.src}, "")
		if !failed(lang) || !strings.Contains(errorsOf(lang), tc.want) {
			t.Errorf("%s: errors = %q, want %q", name, errorsOf(lang), tc.want)
		}
	}
}

func TestImportsIndexExactModules(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "src", "app")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	for name, content := range map[string]string{"package.d": "module app;\n", "util.d": "module app.util;\n", "plain.d": "void f() {}\n"} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	f, err := rule.LoadData(filepath.Join(dir, "BUILD.bazel"), "src/app", []byte("d_library(name = \"app\", srcs = [\"package.d\", \"util.d\", \"plain.d\"])\n"))
	if err != nil {
		t.Fatal(err)
	}
	specs := NewLanguage().(*dLang).Imports(nil, f.Rules[0], f)
	var got []string
	for _, s := range specs {
		got = append(got, s.Imp)
	}
	if strings.Join(got, ",") != "app,app.util,plain" {
		t.Errorf("indexed %v, want app, app.util, plain", got)
	}
}
