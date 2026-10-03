package ruby

import (
	"github.com/ralvik/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "ruby"

var rubyKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		DisplayName:    "Ruby",
		LibraryKind:    LibraryKind,
		Kinds:          rubyKinds,
		DefsPackage:    "ruby/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ClassIdentity,
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		Generate:       generateRules,
	})
}

func CollectUsedIgnores(c *config.Config) [][2]string { return common.UsedIgnores(c, languageName) }

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
		MainRule:     "ruby_binary",
		MainPhrase:   "main-bearing sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Wrap:         common.WrapImportSet,
	}, rep)
}
