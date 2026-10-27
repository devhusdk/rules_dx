package zig

import (
	"fmt"
	"sort"
	"strings"

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
			"deps":       true,
			"extra_srcs": true,
			"main":       true,
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

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource: func(file string) bool {
			return isSupported(file) && !IsTestSource(file) && !IsBuildScript(file)
		},
		DirName:      DirTargetName,
		RootHint:     rootHint,
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
		Extend: trackFileRefs,
		Wrap:   common.WrapImportSet,
	}, rep)
}

func trackFileRefs(r *rule.Rule, files []string, contents map[string][]byte) error {
	tracked := map[string]bool{r.AttrString("main"): true}
	for _, src := range r.AttrStrings("srcs") {
		tracked[src] = true
	}
	present := make(map[string]bool, len(files))
	for _, file := range files {
		present[file] = true
	}
	sources := make([]string, 0, len(contents))
	for src := range contents {
		sources = append(sources, src)
	}
	sort.Strings(sources)
	extra := make(map[string]bool)
	for _, src := range sources {
		for _, ref := range ParseFileRefs(contents[src]) {
			switch {
			case strings.ContainsAny(ref, "/\\"):
				return fmt.Errorf("%s reads %q outside this directory; move it here or write this zig_library by hand and add # gazelle:exclude", src, ref)
			case tracked[ref]:
			case strings.HasSuffix(ref, ".zig") && present[ref]:
				return fmt.Errorf("%s imports %q, which generation leaves out of srcs; rename it or write this zig_library by hand and add # gazelle:exclude", src, ref)
			case !present[ref]:
				return fmt.Errorf("%s reads %q, which is missing from this directory", src, ref)
			default:
				extra[ref] = true
			}
		}
	}
	if len(extra) == 0 {
		return nil
	}
	names := make([]string, 0, len(extra))
	for name := range extra {
		names = append(names, name)
	}
	sort.Strings(names)
	r.SetAttr("extra_srcs", names)
	return nil
}
