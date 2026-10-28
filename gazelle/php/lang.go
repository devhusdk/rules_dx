package php

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

const languageName = "php"

var phpKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
	TestKind:    common.LibraryKindInfo(),
}

type phpLang struct {
	*common.SingleLang
	excludes []string
}

func NewLanguage() language.Language {
	l := &phpLang{}
	l.SingleLang = common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		ExtraKinds:     []string{TestKind},
		Kinds:          phpKinds,
		DefsPackage:    "php/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: isSupported,
		IsStdLib:       IsStdLib,
		QualifiedIDs:   qualifiedIDs,
		UnresolvedHint: "add a local php_library that declares the class, or a # gazelle:resolve / resolve_regexp mapping",
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

func (l *phpLang) Before(ctx context.Context) {
	l.excludes = nil
	l.SingleLang.Before(ctx)
}

// qualifiedIDs returns the names a source declares. Classmap autoload goes by
// what a file declares, not by its path.
func qualifiedIDs(file string) []string {
	content, err := common.ReadSource(file)
	if err != nil {
		return nil
	}
	declares, _ := Parse(content)
	return declares
}

// Imports indexes a library under the names its srcs and autoload_files
// declare.
func (l *phpLang) Imports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	specs := l.SingleLang.Imports(c, r, f)
	if r.Kind() != LibraryKind || f == nil {
		return specs
	}
	seen := make(map[string]bool, len(specs))
	for _, s := range specs {
		seen[s.Imp] = true
	}
	for _, src := range r.AttrStrings("autoload_files") {
		if !isSupported(src) {
			continue
		}
		for _, id := range qualifiedIDs(filepath.Join(filepath.Dir(f.Path), src)) {
			if !seen[id] {
				seen[id] = true
				specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: id})
			}
		}
	}
	return specs
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

// generateRules emits one php_library for the directory's sources and one
// php_test per *Test.php under tests or test. Sources in src below a directory
// without its own BUILD file belong to the library, so those directories
// generate nothing. php_binary targets stay hand-written, and files a
// library lists in autoload_files stay out of its srcs.
func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter, excludes []string) language.GenerateResult {
	if ownedByAncestor(args) {
		return language.GenerateResult{}
	}
	autoload := autoloadFiles(args)
	srcs := librarySources(args, excludes, autoload)
	tests := testSources(args, excludes)
	support := testSupportSources(args, excludes, autoload)
	if len(srcs) == 0 && len(tests) == 0 && len(support) == 0 {
		return common.MergeStale(args.File, language.GenerateResult{}, kinds)
	}
	libName, err := common.PackageTargetName(args)
	if args.Rel != "" {
		libName, err = DirTargetName(args.Rel)
	}
	if err != nil {
		rep.Fail("%s: %s: %v", languageName, args.Rel, err)
		return language.GenerateResult{}
	}
	var claimants []common.Claimant
	if len(srcs) > 0 {
		claimants = append(claimants, common.Claimant{Name: libName, Source: args.Rel, Kind: LibraryKind})
	}
	supportName := libName + "_test_support"
	if len(support) > 0 {
		claimants = append(claimants, common.Claimant{Name: supportName, Source: "tests", Kind: LibraryKind})
	}
	testNames := make(map[string]string, len(tests))
	byName := make(map[string]string, len(tests))
	for _, src := range tests {
		name, err := TestTargetName(src)
		if err != nil {
			rep.Fail("%s: %s: %v", languageName, args.Rel, err)
			return language.GenerateResult{}
		}
		if other, dup := byName[name]; dup {
			rep.Fail("%s: %s: tests %s and %s both name the target %q", languageName, args.Rel, other, src, name)
			return language.GenerateResult{}
		}
		byName[name] = src
		testNames[src] = name
		claimants = append(claimants, common.Claimant{Name: name, Source: src, Kind: TestKind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, nil); err != nil {
		rep.Fail("%s: %s: %v", languageName, args.Rel, err)
		return language.GenerateResult{}
	}
	var result language.GenerateResult
	add := func(kind, name string, files, extra []string) {
		r := rule.NewRule(kind, name)
		r.SetAttr("srcs", files)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(importsOf(args, append(append([]string{}, files...), extra...), kind == TestKind, rep)))
	}
	if len(srcs) > 0 {
		add(LibraryKind, libName, srcs, autoload[libName])
	}
	if len(support) > 0 {
		// Helpers such as tests/TestCase.php are not tests, but tests extend
		// and load them, so they ship as a testonly library the tests depend on.
		n := len(result.Gen)
		add(LibraryKind, supportName, support, nil)
		result.Gen[n].SetAttr("testonly", true)
		result.Imports[n] = common.WrapImportSet(importsOf(args, support, true, rep))
	}
	for _, src := range tests {
		add(TestKind, testNames[src], []string{src}, nil)
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

// autoloadFiles maps each php_library of the BUILD file to its autoload_files.
func autoloadFiles(args language.GenerateArgs) map[string][]string {
	out := make(map[string][]string)
	if args.File == nil {
		return out
	}
	for _, r := range args.File.Rules {
		if r.Kind() == LibraryKind {
			if files := r.AttrStrings("autoload_files"); len(files) > 0 {
				out[r.Name()] = files
			}
		}
	}
	return out
}

func importsOf(args language.GenerateArgs, files []string, isTest bool, rep common.Reporter) []string {
	seen := make(map[string]bool)
	var imports []string
	for _, file := range files {
		content, err := common.ReadSource(filepath.Join(args.Dir, filepath.FromSlash(file)))
		if err != nil {
			rep.Fail("%s: %s: read %s: %v", languageName, args.Rel, file, err)
			continue
		}
		_, names := Parse(content)
		for _, name := range names {
			if isTest && strings.HasPrefix(name, `PHPUnit\`) {
				continue
			}
			if !seen[name] {
				seen[name] = true
				imports = append(imports, name)
			}
		}
	}
	sort.Strings(imports)
	return imports
}

// librarySources lists the package-relative .php files the library owns: the
// directory's own sources plus every source under src that no nested package
// or exclude claims. Tests and autoload_files are left out.
func librarySources(args language.GenerateArgs, excludes []string, autoload map[string][]string) []string {
	skip := make(map[string]bool)
	for _, files := range autoload {
		for _, f := range files {
			skip[path.Clean(f)] = true
		}
	}
	var srcs []string
	for _, name := range args.RegularFiles {
		if isSupported(name) && !isTestSource(name) && !skip[name] && !excluded(args, excludes, name) {
			srcs = append(srcs, name)
		}
	}
	if !claimed(args, excludes, srcDir) {
		walk(args, excludes, srcDir, func(name string) bool { return !isTestSource(path.Base(name)) && !skip[name] }, &srcs)
	}
	sort.Strings(srcs)
	return srcs
}

// testSources lists the *Test.php files under the package's tests and test
// directories, and any in the package directory itself.
func testSources(args language.GenerateArgs, excludes []string) []string {
	var tests []string
	for _, name := range args.RegularFiles {
		if isSupported(name) && isTestSource(name) && !excluded(args, excludes, name) {
			tests = append(tests, name)
		}
	}
	for _, dir := range testDirs {
		if !claimed(args, excludes, dir) {
			walk(args, excludes, dir, func(name string) bool { return isTestSource(path.Base(name)) }, &tests)
		}
	}
	sort.Strings(tests)
	return tests
}

// testSupportSources lists the non-test .php files under the package's tests
// and test directories, such as shared base classes and fixtures.
func testSupportSources(args language.GenerateArgs, excludes []string, autoload map[string][]string) []string {
	skip := make(map[string]bool)
	for _, files := range autoload {
		for _, f := range files {
			skip[path.Clean(f)] = true
		}
	}
	var out []string
	for _, dir := range testDirs {
		if !claimed(args, excludes, dir) {
			walk(args, excludes, dir, func(name string) bool { return !isTestSource(path.Base(name)) && !skip[name] }, &out)
		}
	}
	sort.Strings(out)
	return out
}

func walk(args language.GenerateArgs, excludes []string, rel string, keep func(name string) bool, out *[]string) {
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
				walk(args, excludes, sub, keep, out)
			}
		case isSupported(e.Name()) && keep(sub) && !excluded(args, excludes, sub):
			*out = append(*out, sub)
		}
	}
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

// ownedByAncestor reports whether args.Rel sits inside the src, tests or test
// tree of a parent package, with no BUILD file from that tree's root down to
// the directory itself.
func ownedByAncestor(args language.GenerateArgs) bool {
	if args.Rel == "" || args.Config == nil {
		return false
	}
	segs := strings.Split(args.Rel, "/")
	for i, seg := range segs {
		if seg != srcDir && seg != "tests" && seg != "test" {
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
