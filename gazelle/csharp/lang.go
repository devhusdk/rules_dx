package csharp

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "csharp"

const LibraryKind = "csharp_library"

var SupportedExts = []string{".cs"}

const unresolvedHint = "add a local library that declares the namespace or a # gazelle:resolve mapping on the full namespace"

const TestKind = "csharp_test"

func TestTargetName(src string) (string, error) { return common.SnakeName(src, SupportedExts) }

func NewLanguage() language.Language {
	return common.NewClassLang(common.ClassLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		MainRule:       "csharp_binary",
		DefsPackage:    "@rules_dx//csharp:defs.bzl",
		UnresolvedHint: unresolvedHint,
		Exts:           SupportedExts,
		TestSuffixes:   []string{"Test"},
		PackageIDs:     true,
		ParsePackage:   ParsePackage,
		DefinesMain:    DefinesMain,
		ParseImports:   ParseImports,
		IsStdLib:       IsStdLib,
		External:       "nuget",
		Candidates:     namespaceCandidates,
		Tests:          &common.TestSpec{Kind: TestKind, Name: TestTargetName},
	})
}

func init() { common.RegisterLanguage(NewLanguage) }
