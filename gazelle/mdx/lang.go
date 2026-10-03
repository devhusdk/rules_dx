package mdx

import (
	"github.com/ralvik/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const (
	languageName = "mdx"
	libraryKind  = "mdx_library"
)

var mdxKinds = map[string]rule.KindInfo{
	libraryKind: common.LibraryKindInfo(),
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		DisplayName:    "MDX",
		LibraryKind:    libraryKind,
		Kinds:          mdxKinds,
		DefsPackage:    "mdx/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ModuleName,
		IsImportSource: isSupported,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		Generate:       generateRules,
	})
}

func CollectUsedIgnores(c *config.Config) [][2]string { return common.UsedIgnores(c, languageName) }

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleFile(args, kinds, common.SingleFileSpec{
		LanguageName: languageName,
		LibraryKind:  libraryKind,
		IsSource:     isSupported,
		TargetName:   TargetName,
		ParseImports: ParseImports,
		IsStdLib:     common.IsNodeBuiltin,
		Wrap:         common.WrapImportSet,
	}, rep)
}
