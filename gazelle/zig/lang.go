package zig

import (
	"fmt"
	"path"
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
			"srcs":       true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
	TestKind: common.TestKindInfo("main"),
}

type zigLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &zigLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          zigKinds,
		ExtraKinds:     []string{TestKind},
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
		Extend: func(r *rule.Rule, files []string, contents map[string][]byte) error {
			return trackFileRefs(r, files, contents, args.File)
		},
		Wrap:   common.WrapImportSet,
		Tests: &common.TestSpec{
			Kind: TestKind,
			IsSource: func(file string) bool {
				return isSupported(file) && IsTestSource(file) && !IsBuildScript(file)
			},
			Name: func(src string) (string, error) { return common.SnakeName(src, SupportedExts) },
			Make: func(name, src string, _ []byte) (*rule.Rule, error) {
				r := rule.NewRule(TestKind, name)
				r.SetAttr("main", src)
				return r, nil
			},
		},
	}, rep)
}

func trackFileRefs(r *rule.Rule, files []string, contents map[string][]byte, build *rule.File) error {
	tracked := map[string]bool{r.AttrString("main"): true}
	for _, src := range r.AttrStrings("srcs") {
		tracked[src] = true
	}
	present := make(map[string]bool, len(files))
	for _, file := range files {
		present[file] = true
	}
	genOuts := genruleOutputs(build)
	sources := make([]string, 0, len(contents))
	for src := range contents {
		sources = append(sources, src)
	}
	sort.Strings(sources)
	extra := make(map[string]bool)
	for _, src := range sources {
		for _, raw := range ParseFileRefs(contents[src]) {
			ref := path.Clean(raw)
			switch {
			case strings.ContainsAny(ref, "/\\"):
				return fmt.Errorf("%s reads %q outside this directory; move it here or write this zig_library by hand and add # gazelle:exclude", src, raw)
			case tracked[ref]:
			case genOuts[ref]:
				extra[ref] = true
			case strings.HasSuffix(ref, ".zig") && present[ref]:
				return fmt.Errorf("%s imports %q, which generation leaves out of srcs; rename it or write this zig_library by hand and add # gazelle:exclude", src, raw)
			case !present[ref]:
				return fmt.Errorf("%s reads %q, which is missing from this directory and is not a genrule output here", src, raw)
			default:
				extra[ref] = true
			}
		}
	}
	// Keep existing extra_srcs that still resolve in this package.
	for _, old := range existingExtraSrcs(build, r.Name()) {
		if !tracked[old] && (present[old] || genOuts[old] || strings.Contains(old, ":")) {
			extra[old] = true
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

// genruleOutputs lists the outs of the genrules in build.
func genruleOutputs(build *rule.File) map[string]bool {
	outs := make(map[string]bool)
	if build == nil {
		return outs
	}
	for _, r := range build.Rules {
		if r.Kind() == "genrule" {
			for _, out := range r.AttrStrings("outs") {
				outs[out] = true
			}
		}
	}
	return outs
}

// existingExtraSrcs returns the extra_srcs of the library named name in build.
func existingExtraSrcs(build *rule.File, name string) []string {
	if build == nil {
		return nil
	}
	for _, r := range build.Rules {
		if r.Kind() == LibraryKind && r.Name() == name {
			return r.AttrStrings("extra_srcs")
		}
	}
	return nil
}
