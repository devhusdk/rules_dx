package vue

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

func sfc(script string) string {
	return "<template>\n  <div>hello</div>\n</template>\n\n<script>\n" + script + "</script>\n\n<style>\n.demo {\n  color: black;\n}\n</style>\n"
}

func TestGenerateSourceOnlyPackage(t *testing.T) {
	regular := []string{"demo.vue", "helper.vue", "notes.txt", "helper.js"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.vue":   sfc("import helper from \"./helper.vue\";\nimport fs from \"fs\";\n"),
		"pkg/demo/helper.vue": sfc("export default {};\n"),
		"pkg/demo/notes.txt":  "not a source\n",
		"pkg/demo/helper.js":  "export const x = 1;\n",
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != "vue_library" || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.vue" {
		t.Errorf("library srcs = %q", got)
	}
	helper := result.Gen[1]
	if helper.Kind() != "vue_library" || helper.Name() != "helper" {
		t.Fatalf("helper = %s(%s)", helper.Kind(), helper.Name())
	}
	if got := strings.Join(helper.AttrStrings("srcs"), ","); got != "helper.vue" {
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
