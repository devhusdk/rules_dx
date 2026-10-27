package mdx

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

func mdxDoc(esm string) string {
	if esm != "" && !strings.HasSuffix(esm, "\n") {
		esm += "\n"
	}
	return esm + "\n# Demo\n\nContent.\n"
}

func TestGenerateSourceOnlyPackage(t *testing.T) {
	regular := []string{"demo.mdx", "helper.mdx", "notes.txt", "helper.js"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.mdx":   "import helper from \"./helper.mdx\";\nimport fs from \"fs\";\n\n# Demo\n\nContent.\n",
		"pkg/demo/helper.mdx": "export const label = \"helper\";\n\n# Helper\n",
		"pkg/demo/notes.txt":  "not a source\n",
		"pkg/demo/helper.js":  "export const x = 1;\n",
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != "mdx_library" || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.mdx" {
		t.Errorf("library srcs = %q", got)
	}
	helper := result.Gen[1]
	if helper.Kind() != "mdx_library" || helper.Name() != "helper" {
		t.Fatalf("helper = %s(%s)", helper.Kind(), helper.Name())
	}
	if got := strings.Join(helper.AttrStrings("srcs"), ","); got != "helper.mdx" {
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

func TestGenerateFencedCodeInert(t *testing.T) {
	regular := []string{"demo.mdx", "helper.mdx"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.mdx":   "import helper from \"./helper.mdx\";\n\n# Demo\n\n```js\nimport fake from \"./fake.mdx\";\n```\n",
		"pkg/demo/helper.mdx": mdxDoc("export const label = \"helper\";\n"),
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	libImports := result.Imports[0].(common.RefImportSet)
	if strings.Join(libImports.Roots(), ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper]", libImports)
	}
}

func TestGenerateHeadingBoundary(t *testing.T) {
	regular := []string{"demo.mdx", "helper.mdx"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.mdx":   "# Demo\n\nimport helper from \"./helper.mdx\";\n",
		"pkg/demo/helper.mdx": mdxDoc("export const label = \"\";\n"),
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	libImports := result.Imports[0].(common.RefImportSet)
	if strings.Join(libImports.Roots(), ",") != "pkg/demo/helper" {
		t.Errorf("library imports = %+v, want [pkg/demo/helper]", libImports)
	}
}

func TestGenerateParagraphContinuationInert(t *testing.T) {
	regular := []string{"demo.mdx"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.mdx": "Some prose\nimport helper from \"./helper.mdx\";\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	libImports := result.Imports[0].(common.RefImportSet)
	if len(libImports.Roots()) != 0 {
		t.Errorf("paragraph-continuation imports = %+v, want empty", libImports)
	}
}

func TestGenerateMultiImportAndExport(t *testing.T) {
	regular := []string{"demo.mdx", "client.mdx", "server.mdx"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.mdx":   "import client from \"./client.mdx\";\nexport * from \"./server.mdx\";\nimport fs from \"fs\";\n\n# Demo\n",
		"pkg/demo/client.mdx": mdxDoc("export const label = \"\";\n"),
		"pkg/demo/server.mdx": mdxDoc("export const label = \"\";\n"),
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
