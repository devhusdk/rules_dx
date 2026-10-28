package perl

import (
	"context"
	"os"
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

const languageName = "perl"

var perlKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
	TestKind:    common.LibraryKindInfo(),
}

type perlLang struct {
	*common.SingleLang
	excludes []string
}

func NewLanguage() language.Language {
	l := &perlLang{}
	l.SingleLang = common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		ExtraKinds:     []string{TestKind},
		Kinds:          perlKinds,
		DefsPackage:    "perl/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: isSupported,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		Generate: func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
			return generateRules(args, kinds, rep, l.excludes)
		},
		ConfigureExt: func(_ *config.Config, rel string, f *rule.File, _ common.Reporter) {
			if f == nil {
				return
			}
			for _, d := range f.Directives {
				if d.Key == "exclude" {
					l.excludes = append(l.excludes, path.Join(rel, d.Value))
				}
			}
		},
	})
	return l
}

func (l *perlLang) Before(ctx context.Context) {
	l.excludes = nil
	l.SingleLang.Before(ctx)
}

// Imports indexes each library source under its module name.
func (l *perlLang) Imports(_ *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	pkg := ""
	if f != nil {
		pkg = f.Pkg
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if isSupported(src) {
			specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: ModuleName(pkg, src)})
		}
	}
	return specs
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

// generateRules emits one perl_library for the directory's modules and one
// perl_test per t/*.t. Modules in lib below a directory without its own BUILD
// file belong to the library, so those directories generate nothing.
func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter, excludes []string) language.GenerateResult {
	if ownedByAncestor(args) {
		return language.GenerateResult{}
	}
	srcs := moduleSources(args, excludes)
	tests := testSources(args, excludes)
	if len(srcs) == 0 && len(tests) == 0 {
		return common.MergeStale(args.File, language.GenerateResult{}, kinds)
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
	add := func(kind, name string, files []string) {
		r := rule.NewRule(kind, name)
		r.SetAttr("srcs", files)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(importsOf(args, files, rep)))
	}
	if len(srcs) > 0 {
		add(LibraryKind, libName, srcs)
	}
	for _, src := range tests {
		add(TestKind, testNames[src], []string{src})
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	if common.IsFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	owns := func(r *rule.Rule) bool {
		if r.Name() == libName {
			return true
		}
		for _, src := range r.AttrStrings("srcs") {
			if _, err := os.Stat(filepath.Join(args.Dir, filepath.FromSlash(src))); err == nil {
				return false
			}
		}
		return true
	}
	return common.MergeStaleOwned(args.File, result, kinds, owns)
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

// moduleSources lists the package-relative .pm files the library owns: the
// directory's own modules plus every module under lib that no nested package
// or exclude claims.
func moduleSources(args language.GenerateArgs, excludes []string) []string {
	var srcs []string
	for _, name := range args.RegularFiles {
		if isSupported(name) {
			srcs = append(srcs, name)
		}
	}
	if !claimed(args, excludes, libDir) {
		walkLib(args, excludes, libDir, &srcs)
	}
	sort.Strings(srcs)
	return srcs
}

func walkLib(args language.GenerateArgs, excludes []string, rel string, out *[]string) {
	dir := filepath.Join(args.Dir, filepath.FromSlash(rel))
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	for _, e := range entries {
		sub := path.Join(rel, e.Name())
		switch {
		case e.IsDir():
			if !hiddenDir(e.Name()) && !claimed(args, excludes, sub) {
				walkLib(args, excludes, sub, out)
			}
		case isSupported(e.Name()) && !excluded(args, excludes, sub):
			*out = append(*out, sub)
		}
	}
}

// testSources lists the t/*.t files directly under the package.
func testSources(args language.GenerateArgs, excludes []string) []string {
	if claimed(args, excludes, testDir) {
		return nil
	}
	entries, err := os.ReadDir(filepath.Join(args.Dir, testDir))
	if err != nil {
		return nil
	}
	var tests []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), testExt) && !excluded(args, excludes, path.Join(testDir, e.Name())) {
			tests = append(tests, path.Join(testDir, e.Name()))
		}
	}
	sort.Strings(tests)
	return tests
}

func hiddenDir(name string) bool { return strings.HasPrefix(name, ".") }

// claimed reports whether the package-relative directory sub is excluded or
// holds a BUILD file of its own.
func claimed(args language.GenerateArgs, excludes []string, sub string) bool {
	if excluded(args, excludes, sub) {
		return true
	}
	return hasBuildFile(args, sub)
}

func hasBuildFile(args language.GenerateArgs, sub string) bool {
	names := []string{"BUILD", "BUILD.bazel"}
	if args.Config != nil && len(args.Config.ValidBuildFileNames) > 0 {
		names = args.Config.ValidBuildFileNames
	}
	for _, name := range names {
		if _, err := os.Stat(filepath.Join(args.Dir, filepath.FromSlash(sub), name)); err == nil {
			return true
		}
	}
	return false
}

func excluded(args language.GenerateArgs, excludes []string, sub string) bool {
	if args.Config == nil {
		return false
	}
	full := path.Join(args.Rel, sub)
	for _, pattern := range excludes {
		if ok, _ := path.Match(pattern, full); ok {
			return true
		}
		if ok, _ := path.Match(pattern, path.Base(full)); ok {
			return true
		}
	}
	return false
}

// ownedByAncestor reports whether args.Rel sits inside the lib or t tree of a
// parent package, with no BUILD file from that tree's root down to the
// directory itself.
func ownedByAncestor(args language.GenerateArgs) bool {
	if args.Rel == "" || args.Config == nil {
		return false
	}
	segs := strings.Split(args.Rel, "/")
	for i, seg := range segs {
		if seg != libDir && seg != testDir {
			continue
		}
		parent := language.GenerateArgs{
			Config: args.Config,
			Dir:    filepath.Join(args.Config.RepoRoot, filepath.FromSlash(strings.Join(segs[:i], "/"))),
			Rel:    strings.Join(segs[:i], "/"),
		}
		owned := true
		for j := i; j < len(segs) && owned; j++ {
			owned = !hasBuildFile(parent, strings.Join(segs[i:j+1], "/"))
		}
		if owned {
			return true
		}
	}
	return false
}
