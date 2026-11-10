package ownership

import (
	"path"
	"slices"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "ownership"

type family struct {
	kind     string
	target   string
	defs     string
	isSource func(name string) bool
}

var families = []family{
	{"dockerfile_library", "dockerfile", "dockerfile:defs.bzl", isDockerfile},
	{"cmake_library", "cmake", "cmake:defs.bzl", isCMake},
	{"bicep_library", "bicep", "bicep:defs.bzl", isBicep},
	{"asciidoc_library", "asciidoc", "asciidoc:defs.bzl", isAsciiDoc},
}

func isDockerfile(name string) bool {
	lower := strings.ToLower(name)
	for _, base := range []string{"dockerfile", "containerfile"} {
		if lower == base || strings.HasPrefix(lower, base+".") || strings.HasSuffix(lower, "."+base) {
			return true
		}
	}
	return false
}

func isCMake(name string) bool {
	return name == "CMakeLists.txt" || strings.HasSuffix(name, ".cmake")
}

func isBicep(name string) bool {
	return strings.HasSuffix(name, ".bicep") || strings.HasSuffix(name, ".bicepparam")
}

func isAsciiDoc(name string) bool {
	return strings.HasSuffix(name, ".adoc") || strings.HasSuffix(name, ".asciidoc")
}

func kinds() map[string]rule.KindInfo {
	out := map[string]rule.KindInfo{}
	for _, f := range families {
		out[f.kind] = common.LibraryKindInfo()
	}
	return out
}

func generate(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	var result language.GenerateResult
	var claimants []common.Claimant
	for _, f := range families {
		var srcs []string
		for _, name := range args.RegularFiles {
			if f.isSource(path.Base(name)) {
				srcs = append(srcs, name)
			}
		}
		if len(srcs) == 0 || handwritten(args.File, f, srcs) {
			continue
		}
		sort.Strings(srcs)
		r := rule.NewRule(f.kind, f.target)
		r.SetAttr("srcs", srcs)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, nil)
		claimants = append(claimants, common.Claimant{Name: f.target, Source: path.Join(args.Rel, srcs[0]), Kind: f.kind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail(common.InPackage(args.Rel), "%s: %s: %v", languageName, args.Rel, err)
		return language.GenerateResult{}
	}
	return common.MergeStaleOwned(args.File, result, kinds, func(existing *rule.Rule) bool {
		return slices.ContainsFunc(families, func(f family) bool {
			return f.kind == existing.Kind() && (f.target == existing.Name() || len(existing.AttrStrings("srcs")) == 0)
		})
	})
}

// handwritten reports whether the package already has a rule of the kind under
// another name that lists one of the files, which then owns them.
func handwritten(file *rule.File, f family, srcs []string) bool {
	return file != nil && slices.ContainsFunc(file.Rules, func(r *rule.Rule) bool {
		return r.Kind() == f.kind && r.Name() != f.target && slices.ContainsFunc(r.AttrStrings("srcs"), func(s string) bool { return slices.Contains(srcs, s) })
	})
}

func NewLanguage() language.Language {
	kindDefs := map[string]string{}
	var extra, index []string
	for _, f := range families {
		kindDefs[f.kind] = f.defs
		extra = append(extra, f.kind)
		index = append(index, f.kind)
	}
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    families[0].kind,
		ExtraKinds:     extra[1:],
		Kinds:          kinds(),
		DefsPackage:    families[0].defs,
		KindDefs:       kindDefs,
		IndexKinds:     index[1:],
		ImportAttr:     "srcs",
		IsImportSource: func(string) bool { return true },
		UnresolvedHint: "ownership rules have no dependencies",
		Generate:       generate,
	})
}

func init() { common.RegisterLanguage(NewLanguage) }
