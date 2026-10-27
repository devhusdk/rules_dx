package gleam

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "gleam"

var gleamKinds = map[string]rule.KindInfo{
	LibraryKind: {
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
	TestLibraryKind: {
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
	TestKind: {
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type gleamLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &gleamLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		ExtraKinds:     []string{TestKind, TestLibraryKind},
		Kinds:          gleamKinds,
		DefsPackage:    "gleam/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ModuleName,
		IsImportSource: IsLibrarySource,
		IsStdLib:       IsStdLib,
		Candidates:     Candidates,
		UnresolvedHint: "add a gleam_library that owns that module or an exact # gazelle:resolve gleam mapping",
		Generate:       generateRules,
	})}
}

func listSources(root, dir string, keep func(string) bool) ([]string, error) {
	base := filepath.Join(root, dir)
	if _, err := os.Stat(base); err != nil {
		return nil, nil
	}
	var out []string
	err := filepath.WalkDir(base, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() && p != base {
			if _, err := os.Stat(filepath.Join(p, ManifestFile)); err == nil {
				return filepath.SkipDir
			}
			return nil
		}
		rel, err := filepath.Rel(root, p)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		if !d.IsDir() && keep(rel) {
			out = append(out, rel)
		}
		return nil
	})
	sort.Strings(out)
	return out, err
}

func isGleam(path string) bool { return strings.HasSuffix(path, Extension) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	if !contains(args.RegularFiles, ManifestFile) {
		return common.MergeStale(args.File, language.GenerateResult{}, kinds)
	}
	lib, err := listSources(args.Dir, SourceDir, isGleam)
	if err != nil {
		rep.Fail("gleam: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	tests, err := listSources(args.Dir, TestDir, IsTestSource)
	if err != nil {
		rep.Fail("gleam: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	testModules, err := listSources(args.Dir, TestDir, isGleam)
	if err != nil {
		rep.Fail("gleam: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	var helpers []string
	if len(tests) > 0 {
		for _, src := range testModules {
			if IsTestHelper(src) {
				helpers = append(helpers, src)
			}
		}
	}
	if len(lib) == 0 && len(tests) == 0 {
		return common.MergeStale(args.File, language.GenerateResult{}, kinds)
	}
	hint := ""
	if args.Rel == "" {
		hint = rootPackageName(args.Dir)
	}
	name, err := common.PackageTargetName(args, hint)
	if err != nil {
		rep.Fail("gleam: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	var claimants []common.Claimant
	if len(lib) > 0 {
		claimants = append(claimants, common.Claimant{Name: name, Source: args.Rel, Kind: LibraryKind})
	}
	if len(tests) > 0 {
		claimants = append(claimants, common.Claimant{Name: name + "_test", Source: args.Rel, Kind: TestKind})
	}
	if len(helpers) > 0 {
		claimants = append(claimants, common.Claimant{Name: name + "_test_support", Source: args.Rel, Kind: TestLibraryKind})
	}
	if err := common.CheckClaimsMulti(args.File, args.OtherGen, claimants, func(c common.Claimant) string { return c.Kind }); err != nil {
		rep.Fail("gleam: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	libOwn := make(map[string]bool)
	for _, src := range lib {
		libOwn[ModuleName(src)] = true
	}
	testOwn := make(map[string]bool)
	for _, src := range testModules {
		testOwn[ModuleName(src)] = true
	}
	helperOwn := make(map[string]bool)
	for _, src := range helpers {
		helperOwn[ModuleName(src)] = true
	}
	var libImports, testImports, supportImports []string
	if len(lib) > 0 {
		if libImports, err = external(args.Dir, lib, libOwn); err != nil {
			rep.Fail("gleam: %s: %v", args.Rel, err)
			return language.GenerateResult{}
		}
	}
	if len(tests) > 0 {
		if testImports, err = external(args.Dir, tests, testOwn); err != nil {
			rep.Fail("gleam: %s: %v", args.Rel, err)
			return language.GenerateResult{}
		}
	}
	if len(helpers) > 0 {
		if supportImports, err = external(args.Dir, helpers, helperOwn); err != nil {
			rep.Fail("gleam: %s: %v", args.Rel, err)
			return language.GenerateResult{}
		}
	}
	var result language.GenerateResult
	if len(lib) > 0 {
		r := rule.NewRule(LibraryKind, name)
		r.SetAttr("srcs", lib)
		if common.IsFixturePath(args.Rel) {
			r.SetAttr("testonly", true)
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(libImports))
	}
	if len(helpers) > 0 {
		r := rule.NewRule(TestLibraryKind, name+"_test_support")
		r.SetAttr("srcs", helpers)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(supportImports))
	}
	if len(tests) > 0 {
		r := rule.NewRule(TestKind, name+"_test")
		r.SetAttr("srcs", tests)
		if len(helpers) > 0 {
			r.SetAttr("deps", []string{":" + name + "_test_support"})
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(testImports))
	}
	return common.MergeStale(args.File, result, kinds)
}

func external(root string, files []string, own map[string]bool) ([]string, error) {
	set := make(map[string]bool)
	for _, file := range files {
		content, err := os.ReadFile(filepath.Join(root, file))
		if err != nil {
			return nil, fmt.Errorf("read %s: %w", file, err)
		}
		for _, name := range ParseImports(content) {
			if !own[name] {
				set[name] = true
			}
		}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out, nil
}

func contains(list []string, want string) bool {
	for _, item := range list {
		if item == want {
			return true
		}
	}
	return false
}
