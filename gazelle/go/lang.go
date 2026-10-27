package golang

import (
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/devhusdk/rules_dx/gazelle/common"
	"golang.org/x/mod/modfile"
)

const languageName = "go"

var goKinds = map[string]rule.KindInfo{
	LibraryKind: rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":       true,
			"deps":       true,
			"importpath": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
	TestKind: rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":  true,
			"deps":  true,
			"embed": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type goLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language { return newLang() }

func newLang() *goLang {
	return &goLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		ExtraKinds:     []string{TestKind},
		Kinds:          goKinds,
		DefsPackage:    "go/rules:defs.bzl",
		IsStdLib:       IsStdLib,
		External:       "go",
		UnresolvedHint: "add a go_library with that importpath or an exact # gazelle:resolve mapping",
		Generate:       generateRules,
	})}
}

func (*goLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	importpath := r.AttrString("importpath")
	if importpath == "" {
		return nil
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: importpath}}
}

func isSupported(name string) bool {
	for _, ext := range SupportedExts {
		if strings.HasSuffix(name, ext) {
			return true
		}
	}
	return false
}

func generateRules(args language.GenerateArgs, _ map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	var sources []string
	var testSources []string
	contents := make(map[string][]byte)
	for _, name := range args.RegularFiles {
		if !isSupported(name) {
			continue
		}
		content, err := os.ReadFile(filepath.Join(args.Dir, name))
		if err != nil {
			rep.Fail("go: %s: read %s: %v", args.Rel, name, err)
			continue
		}
		excluded, err := BuildExcluded(content)
		if err != nil {
			rep.Fail("go: %s: parse build constraint %s: %v", args.Rel, name, err)
			continue
		}
		if excluded {
			continue
		}
		contents[name] = content
		if IsTestSource(name) {
			testSources = append(testSources, name)
			continue
		}
		sources = append(sources, name)
	}
	sort.Strings(sources)
	sort.Strings(testSources)
	if rep.Failed() {
		return language.GenerateResult{}
	}
	if len(sources) == 0 && len(testSources) == 0 {
		return mergeStale(args.File, language.GenerateResult{})
	}
	if len(sources) == 0 {
		rep.Fail("go: %s: test sources %s without non-test sources; add the package sources to this directory or split the tests before adopting generation", args.Rel, strings.Join(testSources, ", "))
		return language.GenerateResult{}
	}

	name, err := common.PackageTargetName(args, goRootHint(args))
	if err != nil {
		rep.Fail("go: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	testName := name + "_test"

	packages := make(map[string]bool)
	seen := make(map[string]bool)
	var imports []string
	var libPkg string
	for _, src := range sources {
		content := contents[src]
		pkg, err := ParsePackage(content)
		if err != nil {
			rep.Fail("go: %s: parse package %s: %v", args.Rel, src, err)
			continue
		}
		if pkg == "main" {
			rep.Fail("go: %s: %s declares package main; thin go_binary entries stay handwritten, so split package-main sources into their own directory before adopting generation", args.Rel, src)
			continue
		}
		packages[pkg] = true
		libPkg = pkg
		roots, err := ParseImports(content)
		if err != nil {
			rep.Fail("go: %s: parse imports %s: %v", args.Rel, src, err)
			continue
		}
		for _, root := range roots {
			if IsCgoImport(root) {
				rep.Fail("go: %s: %s imports cgo (\"C\"); cgo stays handwritten, so split cgo sources into their own handwritten directory before adopting generation", args.Rel, src)
				continue
			}
			if IsStdLib(root) || seen[root] {
				continue
			}
			seen[root] = true
			imports = append(imports, root)
		}
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	if len(packages) > 1 {
		names := make([]string, 0, len(packages))
		for pkg := range packages {
			names = append(names, pkg)
		}
		sort.Strings(names)
		rep.Fail("go: %s: mixed packages %s in one directory; split the directory before adopting generation", args.Rel, strings.Join(names, ", "))
		return language.GenerateResult{}
	}
	for pkg := range packages {
		libPkg = pkg
	}
	sort.Strings(imports)

	testSeen := make(map[string]bool)
	var testImports []string
	for _, src := range testSources {
		content := contents[src]
		pkg, err := ParsePackage(content)
		if err != nil {
			rep.Fail("go: %s: parse package %s: %v", args.Rel, src, err)
			continue
		}
		if pkg != libPkg && pkg != libPkg+"_test" {
			rep.Fail("go: %s: %s declares package %s, want %s or %s_test; split the directory before adopting generation", args.Rel, src, pkg, libPkg, libPkg)
			continue
		}
		roots, err := ParseImports(content)
		if err != nil {
			rep.Fail("go: %s: parse imports %s: %v", args.Rel, src, err)
			continue
		}
		for _, root := range roots {
			if IsCgoImport(root) {
				rep.Fail("go: %s: %s imports cgo (\"C\"); cgo stays handwritten, so split cgo sources into their own handwritten directory before adopting generation", args.Rel, src)
				continue
			}
			if IsStdLib(root) || seen[root] || testSeen[root] {
				continue
			}
			testSeen[root] = true
			testImports = append(testImports, root)
		}
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	sort.Strings(testImports)

	claimants := []Claimant{{Name: name, Source: args.Rel, Kind: LibraryKind}}
	if len(testSources) > 0 {
		claimants = append(claimants, Claimant{Name: testName, Source: args.Rel, Kind: TestKind})
	}
	if err := checkClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail("go: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	result := language.GenerateResult{}
	r := rule.NewRule(LibraryKind, name)
	r.SetAttr("srcs", sources)
	if importpath, ok := goImportPath(args.Config.RepoRoot, args.Dir); ok {
		r.SetAttr("importpath", importpath)
	}
	result.Gen = append(result.Gen, r)
	result.Imports = append(result.Imports, common.WrapImportSet(imports))
	if len(testSources) > 0 {
		t := rule.NewRule(TestKind, testName)
		t.SetAttr("srcs", testSources)
		t.SetAttr("embed", []string{":" + name})
		result.Gen = append(result.Gen, t)
		result.Imports = append(result.Imports, common.WrapImportSet(testImports))
	}
	if isFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return mergeStale(args.File, result)
}

func checkClaims(file *rule.File, other []*rule.Rule, claimants []Claimant) error {
	return common.CheckClaims(file, other, claimants, nil)
}

func isFixturePath(rel string) bool { return common.IsFixturePath(rel) }

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStale(file, result, goKinds)
}

func goImportPath(repoRoot, dir string) (string, bool) {
	cleanDir := filepath.Clean(dir)
	cleanRoot := filepath.Clean(repoRoot)
	for cur := cleanDir; ; cur = filepath.Dir(cur) {
		candidate := filepath.Join(cur, "go.mod")
		if content, err := os.ReadFile(candidate); err == nil {
			module := modfile.ModulePath(content)
			if module == "" {
				return "", false
			}
			rel, err := filepath.Rel(cur, cleanDir)
			if err != nil {
				return "", false
			}
			slash := filepath.ToSlash(rel)
			if slash == "." || slash == "" {
				return module, true
			}
			return module + "/" + slash, true
		}
		if cur == cleanRoot || cur == filepath.Dir(cur) {
			return "", false
		}
		if len(cur) < len(cleanRoot) || !strings.HasPrefix(cur, cleanRoot) {
			return "", false
		}
	}
}

func embeds(r *rule.Rule, from, dep label.Label) bool {
	for _, e := range r.AttrStrings("embed") {
		lbl, err := label.Parse(e)
		if err != nil {
			continue
		}
		if lbl.Relative {
			lbl = label.New(from.Repo, from.Pkg, lbl.Name)
		}
		if lbl.Repo == "" {
			lbl.Repo = from.Repo
		}
		if lbl == dep {
			return true
		}
	}
	return false
}

func (l *goLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	spec := l.ResolveSpecFor(c, r, from)
	if r.Kind() == TestKind {
		spec.SkipDep = func(dep label.Label) bool { return embeds(r, from, dep) }
	}
	unresolved := spec.FailUnresolved
	spec.FailUnresolved = func(from label.Label, name string, hint string) {
		if leaf := path.Base(name); leaf != name {
			if override, found := resolve.FindRuleWithOverride(c, resolve.ImportSpec{Lang: languageName, Imp: leaf}, languageName); found {
				l.Fail("go: %s: unresolved import %q; # gazelle:resolve go %s %s names only the last path element, so key it by the full import path", from, name, leaf, override)
				return
			}
		}
		unresolved(from, name, hint)
	}
	common.ResolveSingle(c, ix, r, raw, from, spec)
}
