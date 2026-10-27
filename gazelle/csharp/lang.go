package csharp

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "csharp"

const unresolvedHint = "add a local library that declares the namespace or a # gazelle:resolve mapping on the full namespace"

var csharpKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          csharpKinds,
		DefsPackage:    "csharp/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: unresolvedHint,
		QualifiedIDs:   namespaceIDs,
		Candidates:     namespaceCandidates,
		Generate:       generateRules,
	})
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(name string) bool { return isSupported(name) && !IsTestSource(name) },
		DirName:      DirTargetName,
		CheckPackage: true,
		ParsePackage: ParsePackage,
		DefinesMain:  DefinesMain,
		MainRule:     "csharp_binary",
		MainPhrase:   "main-bearing sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Candidates:   namespaceCandidates,
		Wrap:         common.WrapImportSet,
	}, rep)
}
