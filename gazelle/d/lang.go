package d

import (
	"fmt"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const languageName = "d"

const unresolvedHint = "add a d_library whose srcs declare that module, or an exact # gazelle:resolve d <module> <label>"

// dKinds limits merging to the attributes generation owns. Handwritten
// versions, dopts, linkopts and target_compatible_with stay as written.
var dKinds = map[string]rule.KindInfo{
	LibraryKind: {
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps":        true,
			"imports":     true,
			"srcs":        true,
			"string_srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type dLang struct {
	*common.SingleLang
}

// NewLanguage returns the D language. Modules resolve by exact name only: a
// prefix match could bind an import to a rule that defines a shorter module.
func NewLanguage() language.Language {
	return &dLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          dKinds,
		DefsPackage:    "d/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: isSupported,
		IsStdLib:       IsStdLib,
		UnresolvedHint: unresolvedHint,
		Generate:       generateRules,
	})}
}

// Imports indexes each source under the module it defines, read from its
// declaration or, without one, from its file name.
func (l *dLang) Imports(_ *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind || f == nil {
		return nil
	}
	var specs []resolve.ImportSpec
	seen := make(map[string]bool)
	for _, src := range r.AttrStrings("srcs") {
		if !isSupported(src) {
			continue
		}
		content, err := common.ReadSource(filepath.Join(filepath.Dir(f.Path), filepath.FromSlash(src)))
		if err != nil {
			continue
		}
		info, err := Analyze(content)
		if err != nil {
			continue
		}
		module, err := ModuleOf(f.Pkg, path.Base(src), info.Module)
		if err != nil || seen[module] {
			continue
		}
		seen[module] = true
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: module})
	}
	return specs
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

// DirTargetName names the library of directory dir.
func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	root := "."
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		// Only parsed D claims a .d file: DTrace scripts and Make
		// dependency files share the extension.
		IsSource: func(file string) bool {
			if !isSupported(file) {
				return false
			}
			content, err := common.ReadSource(filepath.Join(args.Dir, file))
			return err != nil || !IsForeign(content)
		},
		DirName:      DirTargetName,
		ParsePackage: func([]byte) (string, error) { return "", nil },
		DefinesMain:  DefinesMain,
		MainRule:     "d_binary",
		MainPhrase:   "sources with a main function",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		OrderSrcs: func(_ string, sources []string, contents map[string][]byte) ([]string, error) {
			declared := make(map[string]string, len(sources))
			for _, src := range sources {
				info, err := Analyze(contents[src])
				if err != nil {
					return nil, fmt.Errorf("%s: %v; fix it, or write this d_library by hand and add # gazelle:exclude %s", src, err, src)
				}
				declared[src] = info.Module
			}
			var err error
			root, err = PackageRoot(args.Rel, declared)
			return sources, err
		},
		Extend: func(r *rule.Rule, files []string, contents map[string][]byte) error {
			if root != "." {
				r.SetAttr("imports", []string{root})
			}
			return trackStringImports(r, files, contents, args.File)
		},
		Wrap: common.WrapImportSet,
	}, rep)
}

// trackStringImports lists the files import("...") reads in string_srcs.
func trackStringImports(r *rule.Rule, files []string, contents map[string][]byte, build *rule.File) error {
	tracked := make(map[string]bool)
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
		refs, err := ParseStringImports(contents[src])
		if err != nil {
			return fmt.Errorf("%s: %v; use a string literal or write this d_library by hand and add # gazelle:exclude", src, err)
		}
		for _, raw := range refs {
			ref := path.Clean(raw)
			switch {
			case strings.ContainsAny(ref, "/\\"):
				return fmt.Errorf("%s reads %q outside this directory; move it here or write this d_library by hand and add # gazelle:exclude", src, raw)
			case tracked[ref]:
			case genOuts[ref], present[ref]:
				extra[ref] = true
			default:
				return fmt.Errorf("%s reads %q, which is missing from this directory and is not a genrule output here", src, raw)
			}
		}
	}
	// Keep existing string_srcs that still resolve in this package.
	for _, old := range existingStringSrcs(build, r.Name()) {
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
	r.SetAttr("string_srcs", names)
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

// existingStringSrcs returns the string_srcs of the library named name.
func existingStringSrcs(build *rule.File, name string) []string {
	if build == nil {
		return nil
	}
	for _, r := range build.Rules {
		if r.Kind() == LibraryKind && r.Name() == name {
			return r.AttrStrings("string_srcs")
		}
	}
	return nil
}

