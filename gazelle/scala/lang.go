package scala

import (
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "scala"

const LibraryKind = "scala_library"

var SupportedExts = []string{".scala"}

// TestKind is the rule generated for each test source.
const TestKind = "scala_test"

// TestTargetName names the test rule for a test source: DemoTest gives demo_test.
func TestTargetName(src string) (string, error) { return common.SnakeName(src, SupportedExts) }

func NewLanguage() language.Language {
	return common.NewClassLang(common.ClassLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		MainRule:       "scala_binary",
		DefsPackage:    "scala/rules:defs.bzl",
		UnresolvedHint: "add a local library, a maven_install.json artifact, or a # gazelle:resolve mapping on the qualified import or its package",
		Exts:           SupportedExts,
		TestSuffixes:   []string{"Test", "Spec", "Suite"},
		ClassIDs:       true,
		PackageIDs:     true,
		ParsePackage:   ParsePackage,
		DefinesMain:    DefinesMain,
		ParseImports:   ParseImports,
		IsStdLib:       IsStdLib,
		Candidates:     common.JVMCandidates,
		External:       "maven",
		Tests: &common.TestSpec{Kind: TestKind, Name: TestTargetName, IsImplicit: isTestImplicit},
	})
}

// isTestImplicit reports ScalaTest imports, which scala_test targets get
// from their own framework deps rather than from generation.
func isTestImplicit(imp string) bool {
	return imp == "org.scalatest" || strings.HasPrefix(imp, "org.scalatest.")
}
