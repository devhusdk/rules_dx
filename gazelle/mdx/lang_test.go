package mdx

import (
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
	if lib.Kind() != libraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.mdx" {
		t.Errorf("library srcs = %q", got)
	}
	helper := result.Gen[1]
	if helper.Kind() != libraryKind || helper.Name() != "helper" {
		t.Fatalf("helper = %s(%s)", helper.Kind(), helper.Name())
	}
	if got := strings.Join(helper.AttrStrings("srcs"), ","); got != "helper.mdx" {
		t.Errorf("helper srcs = %q", got)
	}
	libImports := result.Imports[0].(common.ImportSet)
	if strings.Join(libImports.Names, ",") != "helper" {
		t.Errorf("library imports = %+v, want [helper]", libImports)
	}
	helperImports := result.Imports[1].(common.ImportSet)
	if len(helperImports.Names) != 0 {
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
	libImports := result.Imports[0].(common.ImportSet)
	if strings.Join(libImports.Names, ",") != "helper" {
		t.Errorf("library imports = %+v, want [helper]", libImports)
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
	libImports := result.Imports[0].(common.ImportSet)
	if strings.Join(libImports.Names, ",") != "helper" {
		t.Errorf("library imports = %+v, want [helper]", libImports)
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
	libImports := result.Imports[0].(common.ImportSet)
	if len(libImports.Names) != 0 {
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
	libImports := result.Imports[1].(common.ImportSet)
	if strings.Join(libImports.Names, ",") != "client,server" {
		t.Errorf("library imports = %+v, want [client server]", libImports)
	}
	for idx, want := range []string{"client", "server"} {
		ruleIdx := idx * 2
		if got := result.Gen[ruleIdx].Name(); got != want {
			t.Fatalf("rule[%d] = %s, want %s", ruleIdx, got, want)
		}
		if neighbor := result.Imports[ruleIdx].(common.ImportSet); len(neighbor.Names) != 0 {
			t.Errorf("%s imports = %+v, want empty", want, neighbor)
		}
	}
}

func TestGenerateEmptySweepsStale(t *testing.T) {
	result := generateFixture(t, nil, []string{"notes.txt"})
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
	writeFixture(t, root, "pkg/demo/demo.mdx", mdxDoc("export const x = 1;\n"))
	dir := filepath.Join(root, "pkg", "demo")
	result = NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.mdx"},
		File:         f,
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if len(result.Empty) != 1 || result.Empty[0].Name() != "old" {
		t.Fatalf("empty rules = %v, want [old]", result.Empty)
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := NewLanguage().(*common.SingleLang)
	if l.Name() != "mdx" || len(l.Kinds()) != 1 || l.CheckFlags(flag.NewFlagSet("test", flag.ContinueOnError), config.New()) != nil {
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
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//mdx/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "mdx_library" {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := l.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//mdx/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
	defaultApparent := l.ApparentLoads(func(string) string { return "" })
	if defaultApparent[0].Name != "@rules_dx//mdx/rules:defs.bzl" {
		t.Errorf("default apparent loads = %+v", defaultApparent)
	}
	if got := CollectUsedIgnores(config.New()); got != nil {
		t.Errorf("unused ignores = %v, want none", got)
	}
}
