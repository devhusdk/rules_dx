package kotlin

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "kotlin"

const LibraryKind = "kotlin_library"

// TestKind is the rule generated for each *Test.kt source.
const TestKind = "kotlin_test"

var SupportedExts = []string{".kt"}

func NewLanguage() language.Language {
	return common.NewClassLang(common.ClassLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		MainRule:       "kotlin_binary",
		DefsPackage:    "kotlin/rules:defs.bzl",
		UnresolvedHint: "add a local library, a maven_install.json artifact, or a # gazelle:resolve mapping on the qualified import or its package",
		Exts:           SupportedExts,
		TestSuffixes:   []string{"Test"},
		ClassIDs:       true,
		PackageIDs:     true,
		ParsePackage:   ParsePackage,
		DefinesMain:    DefinesMain,
		ParseImports:   ParseImports,
		IsStdLib:       IsStdLib,
		Candidates:     common.JVMCandidates,
		Variant:        common.AndroidVariant("kotlin_android_library"),
		External:       "maven",
		Tests: &common.TestSpec{
			Kind:       TestKind,
			Name:       TestTargetName,
			Make:       makeTest,
		},
		TestMergeable: []string{"test_class"},
	})
}

// TestTargetName names the kotlin_test for a test source: GreetTest.kt gives
// greet_test.
func TestTargetName(src string) (string, error) { return common.SnakeName(src, SupportedExts) }

// makeTest emits a kotlin_test whose test_class is the source's qualified class.
func makeTest(name, src string, content []byte) (*rule.Rule, error) {
	pkg, err := ParsePackage(content)
	if err != nil {
		return nil, err
	}
	class := common.ClassIdentity(src, SupportedExts)
	if pkg != "" {
		class = pkg + "." + class
	}
	r := rule.NewRule(TestKind, name)
	r.SetAttr("srcs", []string{src})
	r.SetAttr("test_class", class)
	return r, nil
}
