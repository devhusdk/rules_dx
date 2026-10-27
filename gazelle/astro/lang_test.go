package astro

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"

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

func component(script string) string {
	return "---\nconst title = \"hello\";\n---\n\n<script>\n" + script + "</script>\n\n<div class=\"hello\">hello</div>\n\n<style>\n.hello {\n  color: black;\n}\n</style>\n"
}

func TestGenerateSourceOnlyPackage(t *testing.T) {
	regular := []string{"demo.astro", "helper.astro", "notes.txt", "helper.js"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.astro":   component("import helper from \"./helper.astro\";\nimport fs from \"fs\";\n"),
		"pkg/demo/helper.astro": component("export const label = \"\";\n"),
		"pkg/demo/notes.txt":    "not a source\n",
		"pkg/demo/helper.js":    "export const x = 1;\n",
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != "astro_library" || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.astro" {
		t.Errorf("library srcs = %q", got)
	}
	helper := result.Gen[1]
	if helper.Kind() != "astro_library" || helper.Name() != "helper" {
		t.Fatalf("helper = %s(%s)", helper.Kind(), helper.Name())
	}
	if got := strings.Join(helper.AttrStrings("srcs"), ","); got != "helper.astro" {
		t.Errorf("helper srcs = %q", got)
	}
	libImports := result.Imports[0].(common.RefImportSet)
	if strings.Join(libImports.Roots(), ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper]", libImports)
	}
	helperImports := result.Imports[1].(common.RefImportSet)
	if len(helperImports.Roots()) != 0 {
		t.Errorf("helper imports = %+v, want empty", helperImports)
	}
}

func TestGenerateClientScriptImports(t *testing.T) {
	regular := []string{"demo.astro", "helper.astro"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.astro":   "---\nconst title = \"demo\";\n---\n\n<script>\nimport helper from \"./helper.astro\";\n</script>\n\n<script>\nimport fs from \"fs\";\n</script>\n\n<div>x</div>\n",
		"pkg/demo/helper.astro": component("export const label = \"\";\n"),
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	libImports := result.Imports[0].(common.RefImportSet)
	if strings.Join(libImports.Roots(), ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper]", libImports)
	}
}

func TestGenerateFrontmatterImports(t *testing.T) {
	regular := []string{"demo.astro", "helper.astro"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.astro":   "---\nimport helper from \"./helper.astro\";\n---\n\n<div>x</div>\n",
		"pkg/demo/helper.astro": component("export const label = \"\";\n"),
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	libImports := result.Imports[0].(common.RefImportSet)
	if strings.Join(libImports.Roots(), ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper]", libImports)
	}
}

func TestGenerateMixedRegions(t *testing.T) {
	regular := []string{"demo.astro", "client.astro", "server.astro"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.astro":   "---\nimport server from \"./server.astro\";\nimport fs from \"fs\";\n---\n\n<div>x</div>\n\n<script>\nimport client from \"./client.astro\";\n</script>\n",
		"pkg/demo/client.astro": component("export const label = \"\";\n"),
		"pkg/demo/server.astro": component("export const label = \"\";\n"),
	}, regular)
	if len(result.Gen) != 3 || len(result.Imports) != 3 {
		t.Fatalf("generated %d rules and %d import sets, want 3 each", len(result.Gen), len(result.Imports))
	}
	if result.Gen[1].Name() != "demo" {
		t.Fatalf("middle rule = %s, want demo", result.Gen[1].Name())
	}
	libImports := result.Imports[1].(common.RefImportSet)
	if strings.Join(libImports.Roots(), ",") != "pkg/demo/client,pkg/demo/server" {
		t.Errorf("library imports = %+v, want [client server]", libImports)
	}
	for idx, want := range []string{"client", "server"} {
		ruleIdx := idx * 2
		if got := result.Gen[ruleIdx].Name(); got != want {
			t.Fatalf("rule[%d] = %s, want %s", ruleIdx, got, want)
		}
		if neighbor := result.Imports[ruleIdx].(common.RefImportSet); len(neighbor.Roots()) != 0 {
			t.Errorf("%s imports = %+v, want empty", want, neighbor)
		}
	}
}

func TestGenerateBOMKeepsFrontmatterImports(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.astro":   "\xEF\xBB\xBF---\nimport helper from \"./helper.astro\";\n---\n\n<div>x</div>\n",
		"pkg/demo/helper.astro": component("export const label = \"\";\n"),
	}, []string{"demo.astro", "helper.astro"})
	if len(result.Imports) != 2 {
		t.Fatalf("generated %d import sets, want 2", len(result.Imports))
	}
	libImports := result.Imports[0].(common.RefImportSet)
	if strings.Join(libImports.Roots(), ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper]", libImports)
	}
}
