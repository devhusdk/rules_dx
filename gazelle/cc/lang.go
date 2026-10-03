package cc

import (
	"github.com/ralvik/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "cc"

var ccKinds = map[string]rule.KindInfo{
	LibraryKind: rule.KindInfo{
		MatchAttrs:    []string{"srcs", "hdrs"},
		NonEmptyAttrs: map[string]bool{"srcs": true, "hdrs": true},
		MergeableAttrs: map[string]bool{
			"srcs": true,
			"hdrs": true,
			"deps": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		DisplayName:    "CC",
		LibraryKind:    LibraryKind,
		Kinds:          ccKinds,
		DefsPackage:    "cc/rules:defs.bzl",
		ImportAttr:     "hdrs",
		Identity:       HeaderIdentity,
		IsImportSource: IsHeader,
		IsTestSource:   IsTestSource,
		UnresolvedHint: "add a local one-header library or an exact # gazelle:resolve mapping",
		Generate:       generateRules,
	})
}

func CollectUsedIgnores(c *config.Config) [][2]string { return common.UsedIgnores(c, languageName) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(name string) bool { return IsSource(name) && !IsTestSource(name) },
		IsHeader:     func(name string) bool { return IsHeader(name) && !IsTestSource(name) },
		IsMainFile:   IsSource,
		DirName:      DirTargetName,
		CheckPackage: false,
		DefinesMain:  DefinesMain,
		MainRule:     "cc_binary",
		MainPhrase:   "main-defining sources",
		ParseImports: ParseQuotedIncludes,
		MakeRule: func(name string, sources []string, headers []string) *rule.Rule {
			r := rule.NewRule(LibraryKind, name)
			if len(sources) > 0 {
				r.SetAttr("srcs", sources)
			}
			if len(headers) > 0 {
				r.SetAttr("hdrs", headers)
			}
			return r
		},
		Wrap: common.WrapImportSet,
	}, rep)
}
