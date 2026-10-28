package java

import (
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "java"

const LibraryKind = "java_library"

// TestKind is the rule generated for each *Test.java source.
const TestKind = "java_test"

var SupportedExts = []string{".java"}

func NewLanguage() language.Language {
	return common.NewClassLang(common.ClassLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		MainRule:       "java_binary",
		DefsPackage:    "java/rules:defs.bzl",
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
		Variant:        common.AndroidVariant("java_android_library"),
		External:       "maven",
		Tests: &common.TestSpec{
			Kind:       TestKind,
			Name:       TestTargetName,
			Make:       makeTest,
			IsImplicit: IsTestImplicit,
		},
		TestMergeable: []string{"test_class"},
	})
}

// TestTargetName names the java_test for a test source: GreetTest.java gives
// greet_test.
func TestTargetName(src string) (string, error) { return common.SnakeName(src, SupportedExts) }

// IsTestImplicit reports JUnit 4 imports, which the rules_java test runner
// already supplies. JUnit 5 (jupiter, platform) still resolves like any import.
func IsTestImplicit(imp string) bool {
	if strings.HasPrefix(imp, "org.junit.jupiter.") || strings.HasPrefix(imp, "org.junit.platform.") {
		return false
	}
	return imp == "org.junit" || strings.HasPrefix(imp, "org.junit.")
}

// makeTest emits a java_test whose test_class is the source's qualified class.
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
