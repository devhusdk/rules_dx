package scala

import (
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "scala"

const LibraryKind = "scala_library"

var SupportedExts = []string{".scala"}

const TestKind = "scala_test"

func TestTargetName(src string) (string, error) { return common.SnakeName(src, SupportedExts) }

func NewLanguage() language.Language {
	return common.NewClassLang(common.ClassLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		MainRule:       "scala_binary",
		DefsPackage:    "scala:defs.bzl",
		UnresolvedHint: "add a local library, a Maven lock artifact, or a # gazelle:resolve mapping on the qualified import or its package",
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
		Tests:          &common.TestSpec{Kind: TestKind, Name: TestTargetName, IsImplicit: isTestImplicit},
	})
}

func isTestImplicit(imp string) bool {
	return imp == "org.scalatest" || strings.HasPrefix(imp, "org.scalatest.")
}

func init() { common.RegisterLanguage(NewLanguage) }
