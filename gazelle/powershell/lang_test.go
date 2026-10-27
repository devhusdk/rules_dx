package powershell

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

func generateArgs(t *testing.T, lang language.Language, rel string, file *rule.File, files map[string]string) language.GenerateResult {
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
	return lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          rel,
		File:         file,
		RegularFiles: regular,
	})
}

func buildFile(t *testing.T, content string) *rule.File {
	t.Helper()
	f, err := rule.LoadData("pkg/greet/BUILD.bazel", "pkg/greet", []byte(content))
	if err != nil {
		t.Fatal(err)
	}
	return f
}

var greetPackage = map[string]string{
	"Greet.psd1":                  "@{\n  RootModule = 'Greet.psm1'\n  RequiredModules = @('Microsoft.PowerShell.Utility', 'Greet')\n}\n",
	"Greet.psm1":                  "Import-Module Shared\nfunction Get-Greeting { 'hi' }\n",
	"Greet.Tests.ps1":             "Import-Module (Join-Path $PSScriptRoot 'Greet.psm1')\nImport-Module Pester\n",
	"Hello.ps1":                   "Import-Module Greet\n",
	"PSGallery.requirements.psd1": "@{ Pester = '5.7.1' }\n",
}

func names(result language.GenerateResult) string {
	var out []string
	for _, r := range result.Gen {
		out = append(out, r.Kind()+":"+r.Name()+":"+strings.Join(r.AttrStrings("srcs"), "+"))
	}
	return strings.Join(out, ",")
}

func TestGenerateLibraryAndTests(t *testing.T) {
	lang := NewLanguage()
	result := generateArgs(t, lang, "pkg/greet", nil, greetPackage)
	if lang.(*pwshLang).Failed() {
		t.Fatal("generation failed")
	}
	if got, want := names(result), "pwsh_library:greet:Greet.psd1+Greet.psm1,pwsh_test:greet_test:Greet.Tests.ps1"; got != want {
		t.Fatalf("rules = %s, want %s", got, want)
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "Microsoft.PowerShell.Utility,Shared" {
		t.Errorf("library imports = %v; want own modules dropped", got)
	}
	if got := result.Imports[1].(common.ImportSet).Names; strings.Join(got, ",") != "Greet,Pester" {
		t.Errorf("test imports = %v", got)
	}
	if result.Gen[0].Attr("testonly") != nil {
		t.Error("library outside fixtures is testonly")
	}
}

func TestGenerateFixtureLibraryIsTestonly(t *testing.T) {
	result := generateArgs(t, NewLanguage(), "tests/fixtures/greet", nil, greetPackage)
	if !result.Gen[0].AttrBool("testonly") || result.Gen[1].Attr("testonly") != nil {
		t.Errorf("testonly = %v / %v, want library only", result.Gen[0].AttrBool("testonly"), result.Gen[1].Attr("testonly"))
	}
}

func TestGenerateKeepsHandwrittenRules(t *testing.T) {
	file := buildFile(t, `pwsh_library(name = "stale", srcs = ["Old.psm1"])

pwsh_test(name = "old_test", srcs = ["Old.Tests.ps1"])

pwsh_test(name = "pack_test", srcs = ["pack_test.ps1"])

pwsh_binary(name = "hello", srcs = ["Hello.ps1"])

pwsh_library(name = "mixed", srcs = ["Greet.psm1", "helpers.ps1"])
`)
	lang := NewLanguage()
	result := generateArgs(t, lang, "pkg/greet", file, greetPackage)
	if lang.(*pwshLang).Failed() {
		t.Fatal("generation failed")
	}
	var empty []string
	for _, r := range result.Empty {
		empty = append(empty, r.Kind()+":"+r.Name())
	}
	if got := strings.Join(empty, ","); got != "pwsh_library:stale,pwsh_test:old_test" {
		t.Errorf("removed = %s, want only the stale generated-shape rules", got)
	}
}

func TestGenerateWithoutSourcesRemovesOnlyOwnedRules(t *testing.T) {
	file := buildFile(t, `pwsh_library(name = "greet", srcs = ["Greet.psm1"])

pwsh_test(name = "pack_test", srcs = ["pack_test.ps1"])
`)
	result := generateArgs(t, NewLanguage(), "pkg/greet", file, map[string]string{"pack_test.ps1": ""})
	if len(result.Gen) != 0 || len(result.Empty) != 1 || result.Empty[0].Name() != "greet" {
		t.Fatalf("gen = %v, empty = %v; want only greet removed", result.Gen, result.Empty)
	}
}

func TestGenerateRequirementsManifestAlone(t *testing.T) {
	result := generateArgs(t, NewLanguage(), "pkg/root", nil, map[string]string{"PSGallery.requirements.psd1": "@{ Pester = '5.7.1' }\n"})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %s for a lone requirements psd1", names(result))
	}
}

func TestGenerateFailsOnNameClaimedByHandwrittenRule(t *testing.T) {
	lang := NewLanguage()
	file := buildFile(t, "pwsh_binary(name = \"greet\", srcs = [\"Hello.ps1\"])\n")
	result := generateArgs(t, lang, "pkg/greet", file, greetPackage)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %s, want none", names(result))
	}
	lang.(*pwshLang).AfterResolvingDeps(context.Background())
	msg := strings.Join(common.Messages(common.Errors.Take()), "\n")
	for _, want := range []string{`"greet" is claimed by generated pwsh_library(pkg/greet) and existing pwsh_binary`, "# gazelle:exclude"} {
		if !strings.Contains(msg, want) {
			t.Errorf("errors = %q, want it to contain %q", msg, want)
		}
	}
}

func TestImportsDedupeModuleAndManifest(t *testing.T) {
	lib := rule.NewRule(LibraryKind, "greet")
	lib.SetAttr("srcs", []string{"Greet.psd1", "Greet.psm1", "Other.psm1", "helper.ps1"})
	got := NewLanguage().Imports(nil, lib, nil)
	if len(got) != 2 || got[0] != (resolve.ImportSpec{Lang: languageName, Imp: "Greet"}) || got[1].Imp != "Other" {
		t.Errorf("imports = %+v, want one Greet and one Other", got)
	}
	if NewLanguage().Imports(nil, rule.NewRule(TestKind, "t"), nil) != nil {
		t.Error("tests export nothing")
	}
}

func TestResolveModuleNamesAreCaseSensitive(t *testing.T) {
	lang := NewLanguage().(*pwshLang)
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "app", nil)
	lang.Configure(cfg, "app", nil)
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	lib := rule.NewRule(LibraryKind, "greet")
	lib.SetAttr("srcs", []string{"Greet.psd1", "Greet.psm1"})
	index.AddRule(config.New(), lib, rule.EmptyFile("pkg/greet/BUILD.bazel", "pkg/greet"))
	index.Finish()

	test := rule.NewRule(TestKind, "app_test")
	lang.Resolve(cfg, index, nil, test, common.WrapImportSet([]string{"Greet", "Microsoft.PowerShell.Utility"}), label.New("", "app", "app_test"))
	if got := strings.Join(test.AttrStrings("deps"), ","); got != "//pkg/greet" || lang.Failed() {
		t.Fatalf("deps = %s (failed=%v), want //pkg/greet with stdlib skipped", got, lang.Failed())
	}
	lang.Resolve(cfg, index, nil, rule.NewRule(TestKind, "lower_test"), common.WrapImportSet([]string{"greet"}), label.New("", "app", "lower_test"))
	lang.AfterResolvingDeps(context.Background())
	msg := strings.Join(common.Messages(common.Errors.Take()), "\n")
	if !strings.Contains(msg, `unresolved import "greet"`) || !strings.Contains(msg, UnresolvedHint) || !strings.Contains(msg, `add "# gazelle:dx_ignore_import powershell greet" to app/BUILD.bazel`) {
		t.Errorf("errors = %q, want the unresolved hint and ignore remedy for lowercase greet", msg)
	}
}

func TestUnresolvedHint(t *testing.T) {
	if strings.Contains(UnresolvedHint, "dx_ignore_import") || !strings.Contains(UnresolvedHint, "case-sensitively") {
		t.Errorf("hint = %q", UnresolvedHint)
	}
}
