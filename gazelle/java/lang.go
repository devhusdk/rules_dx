package java

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "java"

const unresolvedHint = "add a local library or a # gazelle:resolve mapping on the qualified import or its package"

var javaKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		DisplayName:    "Java",
		LibraryKind:    LibraryKind,
		Kinds:          javaKinds,
		DefsPackage:    "java/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ClassIdentity,
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: unresolvedHint,
		QualifiedIDs:   qualifiedIDs,
		Candidates:     common.JVMCandidates,
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
		MainRule:     "java_binary",
		MainPhrase:   "main-bearing sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Candidates:   common.JVMCandidates,
		Wrap:         common.WrapImportSet,
	}, rep)
}
