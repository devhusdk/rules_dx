package lua

import (
	"os"
	"path/filepath"
	"sort"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const languageName = "lua"

var luaKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
	TestKind: {
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

type luaLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &luaLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		ExtraKinds:     []string{TestKind},
		Kinds:          luaKinds,
		DefsPackage:    "lua/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: isSupported,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		Generate:       generateRules,
	})}
}

// Imports indexes each library source under the module name lua_library gives
// it, reading the rule's own strip_import_prefix and import_prefix.
func (l *luaLang) Imports(_ *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	pkg := ""
	if f != nil {
		pkg = f.Pkg
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if name, ok := ModuleName(pkg, src, r.AttrString("strip_import_prefix"), r.AttrString("import_prefix")); ok {
			specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: name})
		}
	}
	return specs
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

// claimedByBinary lists the sources handwritten lua_binary rules own through
// main or srcs; generation leaves them out of the library.
func claimedByBinary(f *rule.File) map[string]bool {
	claimed := make(map[string]bool)
	if f == nil {
		return claimed
	}
	for _, r := range f.Rules {
		if r.Kind() != BinaryKind {
			continue
		}
		if main := r.AttrString("main"); main != "" {
			claimed[main] = true
		}
		for _, src := range r.AttrStrings("srcs") {
			claimed[src] = true
		}
	}
	return claimed
}

// generateRules emits one lua_library for the directory's modules and one
// lua_test per *_test.lua. lua_binary stays handwritten.
func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	binary := claimedByBinary(args.File)
	var srcs, tests []string
	for _, name := range args.RegularFiles {
		switch {
		case !isSupported(name):
		case IsTestSource(name):
			tests = append(tests, name)
		case binary[name]:
		default:
			if _, ok := ModuleName(args.Rel, name, "", ""); ok {
				srcs = append(srcs, name)
			}
		}
	}
	sort.Strings(srcs)
	sort.Strings(tests)
	if len(srcs) == 0 && len(tests) == 0 {
		return common.MergeStaleOwned(args.File, language.GenerateResult{}, kinds, ownsStale(args, ""))
	}
	libName, err := common.PackageTargetName(args)
	if args.Rel != "" {
		libName, err = DirTargetName(args.Rel)
	}
	if err != nil {
		rep.Fail(common.InPackage(args.Rel), "%s: %s: %v", languageName, args.Rel, err)
		return language.GenerateResult{}
	}
	var claimants []common.Claimant
	if len(srcs) > 0 {
		claimants = append(claimants, common.Claimant{Name: libName, Source: args.Rel, Kind: LibraryKind})
	}
	testNames := make(map[string]string, len(tests))
	for _, src := range tests {
		name, err := TestTargetName(src)
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "%s: %s: %v", languageName, args.Rel, err)
			return language.GenerateResult{}
		}
		testNames[src] = name
		claimants = append(claimants, common.Claimant{Name: name, Source: src, Kind: TestKind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, nil); err != nil {
		rep.Fail(common.InPackage(args.Rel), "%s: %s: %v", languageName, args.Rel, err)
		return language.GenerateResult{}
	}
	var result language.GenerateResult
	if len(srcs) > 0 {
		r := rule.NewRule(LibraryKind, libName)
		r.SetAttr("srcs", srcs)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(importsOf(args, srcs, rep)))
	}
	for _, src := range tests {
		r := rule.NewRule(TestKind, testNames[src])
		r.SetAttr("srcs", []string{src})
		r.SetAttr("main", src)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(importsOf(args, []string{src}, rep)))
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	if common.IsFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return common.MergeStaleOwned(args.File, result, kinds, ownsStale(args, libName))
}

// ownsStale limits stale removal to the generated library name and rules whose
// sources are all gone, so handwritten rules over existing files survive.
func ownsStale(args language.GenerateArgs, libName string) func(*rule.Rule) bool {
	return func(r *rule.Rule) bool {
		if libName != "" && r.Name() == libName {
			return true
		}
		files := r.AttrStrings("srcs")
		if main := r.AttrString("main"); main != "" {
			files = append(files, main)
		}
		for _, src := range files {
			if _, err := os.Stat(filepath.Join(args.Dir, filepath.FromSlash(src))); err == nil {
				return false
			}
		}
		return true
	}
}

func importsOf(args language.GenerateArgs, files []string, rep common.Reporter) []string {
	seen := make(map[string]bool)
	var imports []string
	for _, file := range files {
		content, err := common.ReadSource(filepath.Join(args.Dir, filepath.FromSlash(file)))
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "%s: %s: read %s: %v", languageName, args.Rel, file, err)
			continue
		}
		for _, name := range ParseImports(content) {
			if !seen[name] {
				seen[name] = true
				imports = append(imports, name)
			}
		}
	}
	sort.Strings(imports)
	return imports
}
