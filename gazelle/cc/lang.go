package cc

import (
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

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
	CudaKind: rule.KindInfo{
		NonEmptyAttrs: map[string]bool{"srcs": true, "hdrs": true},
		MergeableAttrs: map[string]bool{
			"srcs": true,
			"hdrs": true,
			"deps": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

const archsDirective = "dx_cuda_archs"

func configureCuda(c *config.Config, rel string, f *rule.File, rep common.Reporter) {
	if f == nil {
		return
	}
	for _, d := range f.Directives {
		if d.Key != archsDirective {
			continue
		}
		archs := strings.Fields(d.Value)
		if len(archs) == 0 {
			rep.Fail("cc: %s: # gazelle:%s needs at least one arch", rel, archsDirective)
			continue
		}
		c.Exts[archsDirective] = archs
	}
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          ccKinds,
		DefsPackage:    "cc/rules:defs.bzl",
		ExtraKinds:     []string{CudaKind},
		KindDefs:       map[string]string{CudaKind: "cuda/rules:defs.bzl"},
		IndexKinds:     []string{CudaKind},
		Directives:     []string{archsDirective},
		ConfigureExt:   configureCuda,
		ImportAttr:     "hdrs",
		Identity:       HeaderIdentity,
		PathIdentity:   true,
		QualifiedIDs:   HeaderBasename,
		Lookups:        IncludeKeys,
		IsImportSource: isCudaLibraryHeader,
		IsIndexSource:  isCudaLibraryHeader,
		IsTestSource:   IsTestSource,
		UnresolvedHint: "includes with a directory resolve by workspace or package-relative path; otherwise add a local one-header library or an exact # gazelle:resolve mapping",
		Generate:       generateRules,
	})
}

// isCudaLibraryHeader reports whether a cuda_library hdrs entry is indexed for
// includes: plain C/C++ headers and .cuh headers both count.
func isCudaLibraryHeader(name string) bool { return IsHeader(name) || IsCudaHeader(name) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	// Gazelle owns only <dir>_cuda, so MergeStale must never see cuda_library.
	owned := make(map[string]rule.KindInfo, len(kinds))
	for k, v := range kinds {
		if k != CudaKind {
			owned[k] = v
		}
	}
	res := generateCc(args, owned, rep)
	cuda := generateCuda(args, rep)
	res.Gen = append(res.Gen, cuda.Gen...)
	res.Imports = append(res.Imports, cuda.Imports...)
	res.Empty = append(res.Empty, cuda.Empty...)
	return res
}

func generateCc(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		OwnsByName:   true,
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
		Extend: androidAttrs,
		Wrap:   common.WrapImportSet,
	}, rep)
}
