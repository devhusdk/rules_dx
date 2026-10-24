package zig

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const languageName = "zig"

var zigKinds = map[string]rule.KindInfo{
	LibraryKind: {
		MatchAttrs:    []string{"main"},
		NonEmptyAttrs: map[string]bool{"main": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"main": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type zigLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &zigLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		DisplayName:    "Zig",
		LibraryKind:    LibraryKind,
		Kinds:          zigKinds,
		DefsPackage:    "zig/rules:defs.bzl",
		ImportAttr:     "main",
		Identity:       func(name string) string { return name },
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a zig_library with that name or import_name, or an exact # gazelle:resolve zig mapping",
		Generate:       generateRules,
	})}
}

func (l *zigLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	name := r.AttrString("import_name")
	if name == "" {
		name = r.Name()
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: name}}
}

func CollectUsedIgnores(c *config.Config) [][2]string { return common.UsedIgnores(c, languageName) }

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource: func(file string) bool {
			return isSupported(file) && !IsTestSource(file) && !IsBuildScript(file)
		},
		DirName:      DirTargetName,
		ParsePackage: ParsePackage,
		DefinesMain:  DefinesMain,
		MainRule:     "zig_binary",
		MainPhrase:   "pub fn main sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		OrderSrcs: func(name string, sources []string, _ map[string][]byte) ([]string, error) {
			return OrderRootFirst(name, sources)
		},
		MakeRule: func(ruleName string, srcs []string, _ []string) *rule.Rule {
			r := rule.NewRule(LibraryKind, ruleName)
			r.SetAttr("main", srcs[0])
			if len(srcs) > 1 {
				r.SetAttr("srcs", srcs[1:])
			}
			return r
		},
		Wrap: common.WrapImportSet,
	}, rep)
}
