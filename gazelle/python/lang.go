package python

import (
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	languageName = "python"
	libraryKind  = "python_library"
	testKind     = "python_test"
	binaryKind   = "python_binary"
	plainKind    = "python_py_test"
	importsAttr  = "."
)

// claimAliases lets a handwritten plain-runner test stand in for the generated test of the same name.
var claimAliases = map[string]string{plainKind: testKind}

var pythonKinds = map[string]rule.KindInfo{
	libraryKind: kindInfo(),
	testKind:    kindInfo(),
	binaryKind:  binaryKindInfo(),
}

func kindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":    true,
			"imports": true,
			"deps":    true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

func binaryKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs: []string{"main"},
		MergeableAttrs: map[string]bool{
			"imports": true,
			"deps":    true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

func NewLanguage() language.Language { return newLang() }

func newLang() *common.SingleLang {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    libraryKind,
		ExtraKinds:     []string{testKind, binaryKind},
		Kinds:          pythonKinds,
		DefsPackage:    "python/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ModuleName,
		IsImportSource: func(name string) bool { return strings.HasSuffix(name, ".py") },
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		External:       "python",
		Generate:       generateRules,
	})
}

func generateRules(args language.GenerateArgs, _ map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	var sources []string
	stubs := make(map[string]bool)
	for _, name := range args.RegularFiles {
		switch {
		case strings.HasSuffix(name, ".py"):
			sources = append(sources, name)
		case strings.HasSuffix(name, ".pyi"):
			stubs[name] = true
		}
	}
	sort.Strings(sources)
	if len(sources) == 0 {
		return mergeStale(args.File, language.GenerateResult{})
	}

	type plan struct {
		name    string
		src     string
		stub    string
		test    bool
		entry   bool
		imports []string
	}
	var plans []plan
	for _, src := range sources {
		content, err := common.ReadSource(filepath.Join(args.Dir, src))
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "python: %s: read %s: %v", args.Rel, src, err)
			continue
		}
		name, err := TargetName(src)
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "python: %s: %v", args.Rel, err)
			continue
		}
		p := plan{name: name, src: src, test: IsTestFile(src)}
		p.entry = !p.test && IsEntryFile(src)
		if stub := strings.TrimSuffix(src, ".py") + ".pyi"; stubs[stub] {
			p.stub = stub
		}
		seen := make(map[string]bool)
		for _, root := range ParseImports(content) {
			if IsStdLib(root) || seen[root] {
				continue
			}
			seen[root] = true
			p.imports = append(p.imports, root)
		}
		sort.Strings(p.imports)
		plans = append(plans, p)
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}

	claimants := make([]Claimant, 0, len(plans)*2)
	for _, p := range plans {
		kind := libraryKind
		if p.test {
			kind = testKind
		}
		claimants = append(claimants, Claimant{Name: p.name, Source: p.src, Kind: kind})
		if p.entry {
			claimants = append(claimants, Claimant{Name: common.BinaryName(p.name), Source: p.src, Kind: binaryKind})
		}
	}
	if err := checkClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail(common.InPackage(args.Rel), "python: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	plain := handPlainTests(args.File, args.OtherGen)
	var result language.GenerateResult
	for _, p := range plans {
		kind := libraryKind
		if p.test {
			if plain[p.name] {
				continue
			}
			kind = testKind
		}
		r := rule.NewRule(kind, p.name)
		srcs := []string{p.src}
		if p.stub != "" {
			srcs = append(srcs, p.stub)
		}
		r.SetAttr("srcs", srcs)
		r.SetAttr("imports", []string{importsAttr})
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(append([]string(nil), p.imports...)))
		if p.entry {
			bin := rule.NewRule(binaryKind, common.BinaryName(p.name))
			bin.SetAttr("main", p.src)
			bin.SetAttr("imports", []string{importsAttr})
			bin.SetAttr("deps", []string{":" + p.name})
			result.Gen = append(result.Gen, bin)
			result.Imports = append(result.Imports, common.WrapImportSet(nil))
		}
	}
	if isFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return mergeStale(args.File, result)
}

func claimKind(c Claimant) string {
	if c.Kind != "" {
		return c.Kind
	}
	if IsTestFile(c.Source) {
		return testKind
	}
	return libraryKind
}

func checkClaims(file *rule.File, other []*rule.Rule, claimants []Claimant) error {
	filled := make([]Claimant, len(claimants))
	for i, c := range claimants {
		c.Kind = claimKind(c)
		filled[i] = c
	}
	return common.CheckClaims(file, other, filled, claimAliases)
}

func handPlainTests(file *rule.File, other []*rule.Rule) map[string]bool {
	owned := make(map[string]bool)
	if file != nil {
		for _, r := range file.Rules {
			if r.Kind() == plainKind {
				owned[r.Name()] = true
			}
		}
	}
	for _, r := range other {
		if r.Kind() == plainKind {
			owned[r.Name()] = true
		}
	}
	return owned
}

func isFixturePath(rel string) bool { return common.IsFixturePath(rel) }

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStale(file, result, pythonKinds)
}
