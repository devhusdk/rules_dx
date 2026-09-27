package common

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

type Reporter interface {
	Fail(format string, args ...interface{})
	Failed() bool
}

func LibraryKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs": true,
			"deps": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

func CheckClaimsDir(file *rule.File, other []*rule.Rule, claimants []Claimant, libraryKind string) error {
	existing := make(map[string]string)
	if file != nil {
		for _, r := range file.Rules {
			existing[r.Name()] = r.Kind()
		}
	}
	for _, r := range other {
		existing[r.Name()] = r.Kind()
	}
	for _, c := range claimants {
		if have, ok := existing[c.Name]; ok && have != libraryKind {
			return fmt.Errorf("target name %q is claimed by generated %s(%s) and existing %s", c.Name, libraryKind, c.Source, have)
		}
	}
	return nil
}

func CheckClaimsPerKind(file *rule.File, other []*rule.Rule, claimants []Claimant) error {
	existing := make(map[string]string)
	if file != nil {
		for _, r := range file.Rules {
			existing[r.Name()] = r.Kind()
		}
	}
	for _, r := range other {
		existing[r.Name()] = r.Kind()
	}
	for _, c := range claimants {
		if have, ok := existing[c.Name]; ok && have != c.Kind {
			return fmt.Errorf("target name %q is claimed by generated %s(%s) and existing %s", c.Name, c.Kind, c.Source, have)
		}
	}
	return nil
}

func CheckClaimsFile(file *rule.File, other []*rule.Rule, claimants []Claimant, libraryKind string) error {
	byName := make(map[string][]string, len(claimants))
	order := make([]string, 0, len(claimants))
	for _, c := range claimants {
		if _, ok := byName[c.Name]; !ok {
			order = append(order, c.Name)
		}
		byName[c.Name] = append(byName[c.Name], c.Source)
	}
	existing := make(map[string]string)
	if file != nil {
		for _, r := range file.Rules {
			existing[r.Name()] = r.Kind()
		}
	}
	for _, r := range other {
		existing[r.Name()] = r.Kind()
	}
	for _, name := range order {
		sources := byName[name]
		if len(sources) > 1 {
			all := append([]string(nil), sources...)
			if have, ok := existing[name]; ok {
				all = append(all, "handwritten:"+have+":"+name)
			}
			return &CollisionError{Name: name, Claimants: all}
		}
		if have, ok := existing[name]; ok && have != libraryKind {
			return fmt.Errorf("target name %q is claimed by generated %s(%s) and existing %s", name, libraryKind, sources[0], have)
		}
	}
	return nil
}

func CheckClaimsMulti(file *rule.File, other []*rule.Rule, claimants []Claimant, kindOf func(Claimant) string) error {
	byName := make(map[string][]string, len(claimants))
	order := make([]string, 0, len(claimants))
	for _, c := range claimants {
		if _, ok := byName[c.Name]; !ok {
			order = append(order, c.Name)
		}
		byName[c.Name] = append(byName[c.Name], c.Source)
	}
	existing := make(map[string]string)
	if file != nil {
		for _, r := range file.Rules {
			existing[r.Name()] = r.Kind()
		}
	}
	for _, r := range other {
		existing[r.Name()] = r.Kind()
	}
	for _, name := range order {
		sources := byName[name]
		kind := ""
		for _, p := range claimants {
			if p.Name == name {
				kind = kindOf(p)
				break
			}
		}
		if len(sources) > 1 {
			all := append([]string(nil), sources...)
			if have, ok := existing[name]; ok {
				all = append(all, "handwritten:"+have+":"+name)
			}
			return &CollisionError{Name: name, Claimants: all}
		}
		if have, ok := existing[name]; ok && have != kind {
			return fmt.Errorf("target name %q is claimed by generated %s(%s) and existing %s", name, kind, sources[0], have)
		}
	}
	return nil
}

func MergeStale(file *rule.File, result language.GenerateResult, kinds map[string]rule.KindInfo) language.GenerateResult {
	desired := make(map[string]bool, len(result.Gen))
	for _, r := range result.Gen {
		desired[r.Kind()+"\x00"+r.Name()] = true
	}
	if file == nil {
		return result
	}
	for _, existing := range file.Rules {
		if _, owned := kinds[existing.Kind()]; !owned || desired[existing.Kind()+"\x00"+existing.Name()] {
			continue
		}
		result.Empty = append(result.Empty, rule.NewRule(existing.Kind(), existing.Name()))
	}
	return result
}

func FormatMatches(matches []resolve.FindResult) string {
	labels := make([]string, 0, len(matches))
	for _, match := range matches {
		labels = append(labels, match.Label.String())
	}
	sort.Strings(labels)
	return fmt.Sprintf("[%s]", strings.Join(labels, ", "))
}

type SingleFileSpec struct {
	LanguageName string
	LibraryKind  string
	IsSource     func(name string) bool
	TargetName   func(name string) (string, error)
	ParseImports func(content []byte) []string
	IsStdLib     func(name string) bool
	Wrap         func(imports []string) any
}

func GenerateSingleFile(args language.GenerateArgs, kinds map[string]rule.KindInfo, spec SingleFileSpec, rep Reporter) language.GenerateResult {
	var sources []string
	for _, name := range args.RegularFiles {
		if spec.IsSource(name) {
			sources = append(sources, name)
		}
	}
	sort.Strings(sources)
	if len(sources) == 0 {
		return MergeStale(args.File, language.GenerateResult{}, kinds)
	}
	type plan struct {
		name    string
		src     string
		imports []string
	}
	var plans []plan
	for _, src := range sources {
		content, err := os.ReadFile(filepath.Join(args.Dir, src))
		if err != nil {
			rep.Fail("%s: %s: read %s: %v", spec.LanguageName, args.Rel, src, err)
			continue
		}
		name, err := spec.TargetName(src)
		if err != nil {
			rep.Fail("%s: %s: %v", spec.LanguageName, args.Rel, err)
			continue
		}
		p := plan{name: name, src: src}
		seen := make(map[string]bool)
		for _, root := range spec.ParseImports(content) {
			if spec.IsStdLib(root) || seen[root] {
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
	claimants := make([]Claimant, 0, len(plans))
	for _, p := range plans {
		claimants = append(claimants, Claimant{Name: p.name, Source: p.src, Kind: spec.LibraryKind})
	}
	if err := CheckClaimsFile(args.File, args.OtherGen, claimants, spec.LibraryKind); err != nil {
		rep.Fail("%s: %s: %v", spec.LanguageName, args.Rel, err)
		return language.GenerateResult{}
	}
	var result language.GenerateResult
	for _, p := range plans {
		r := rule.NewRule(spec.LibraryKind, p.name)
		r.SetAttr("srcs", []string{p.src})
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, spec.Wrap(append([]string(nil), p.imports...)))
	}
	if IsFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return MergeStale(args.File, result, kinds)
}

type SingleDirSpec struct {
	LanguageName string
	LibraryKind  string
	IsSource     func(name string) bool
	IsHeader     func(name string) bool
	IsMainFile   func(name string) bool
	DirName      func(dir string) (string, error)
	CheckPackage bool
	ParsePackage func(content []byte) (string, error)
	DefinesMain  func(content []byte) bool
	MainRule     string
	MainPhrase   string
	ParseImports func(content []byte) []string
	IsStdLib     func(name string) bool
	OrderSrcs    func(sources []string, contents map[string][]byte) ([]string, error)
	MakeRule     func(name string, sources []string, headers []string) *rule.Rule
	Wrap         func(imports []string) any
}

func GenerateSingleDir(args language.GenerateArgs, kinds map[string]rule.KindInfo, spec SingleDirSpec, rep Reporter) language.GenerateResult {
	var sources []string
	var headers []string
	for _, name := range args.RegularFiles {
		switch {
		case spec.IsSource(name):
			sources = append(sources, name)
		case spec.IsHeader != nil && spec.IsHeader(name):
			headers = append(headers, name)
		}
	}
	sort.Strings(sources)
	sort.Strings(headers)
	if len(sources) == 0 && len(headers) == 0 {
		return MergeStale(args.File, language.GenerateResult{}, kinds)
	}
	name, err := spec.DirName(args.Rel)
	if err != nil {
		rep.Fail("%s: %s: %v", spec.LanguageName, args.Rel, err)
		return language.GenerateResult{}
	}
	isMain := spec.IsMainFile
	if isMain == nil {
		isMain = func(string) bool { return true }
	}
	packages := make(map[string]bool)
	seen := make(map[string]bool)
	contents := make(map[string][]byte, len(sources))
	var imports []string
	for _, file := range append(append([]string{}, sources...), headers...) {
		content, err := os.ReadFile(filepath.Join(args.Dir, file))
		if err != nil {
			rep.Fail("%s: %s: read %s: %v", spec.LanguageName, args.Rel, file, err)
			continue
		}
		contents[file] = content
		if isMain(file) && spec.DefinesMain(content) {
			rep.Fail("%s: %s: %s defines main; thin %s entries stay handwritten, so split %s into their own directory before adopting generation", spec.LanguageName, args.Rel, file, spec.MainRule, spec.MainPhrase)
			continue
		}
		if spec.CheckPackage {
			pkg, err := spec.ParsePackage(content)
			if err != nil {
				rep.Fail("%s: %s: parse package %s: %v", spec.LanguageName, args.Rel, file, err)
				continue
			}
			packages[pkg] = true
		}
		for _, root := range spec.ParseImports(content) {
			if (spec.IsStdLib != nil && spec.IsStdLib(root)) || seen[root] {
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
		rep.Fail("%s: %s: mixed packages %s in one directory; split the directory before adopting generation", spec.LanguageName, args.Rel, strings.Join(names, ", "))
		return language.GenerateResult{}
	}
	sort.Strings(imports)
	ordered := sources
	if spec.OrderSrcs != nil {
		ordered, err = spec.OrderSrcs(sources, contents)
		if err != nil {
			rep.Fail("%s: %s: %v", spec.LanguageName, args.Rel, err)
			return language.GenerateResult{}
		}
	}
	if err := CheckClaimsDir(args.File, args.OtherGen, []Claimant{{Name: name, Source: args.Rel, Kind: spec.LibraryKind}}, spec.LibraryKind); err != nil {
		rep.Fail("%s: %s: %v", spec.LanguageName, args.Rel, err)
		return language.GenerateResult{}
	}
	result := language.GenerateResult{}
	makeRule := spec.MakeRule
	if makeRule == nil {
		makeRule = func(ruleName string, srcs []string, _ []string) *rule.Rule {
			r := rule.NewRule(spec.LibraryKind, ruleName)
			r.SetAttr("srcs", srcs)
			return r
		}
	}
	result.Gen = append(result.Gen, makeRule(name, ordered, headers))
	result.Imports = append(result.Imports, spec.Wrap(imports))
	if IsFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return MergeStale(args.File, result, kinds)
}

type ResolveSpec struct {
	LanguageName   string
	WantKind       string
	Attr           string
	UnresolvedHint string
	Unwrap         func(raw any) ([]string, bool)
	IsStdLib       func(name string) bool
	MarkUsed       func(name string) bool
	FailConflict   func(from label.Label, name string)
	FailUnresolved func(from label.Label, name string)
	FailAmbiguous  func(from label.Label, name string, matches string)
}

func ResolveSingle(c *config.Config, ix *resolve.RuleIndex, r *rule.Rule, raw any, from label.Label, spec ResolveSpec) {
	imports, ok := spec.Unwrap(raw)
	if !ok {
		return
	}
	if r.Kind() != spec.WantKind {
		return
	}
	deps := make(map[string]bool)
	for _, name := range imports {
		if spec.IsStdLib != nil && spec.IsStdLib(name) {
			continue
		}
		imp := resolve.ImportSpec{Lang: spec.LanguageName, Imp: name}
		if override, found := resolve.FindRuleWithOverride(c, imp, spec.LanguageName); found {
			if spec.MarkUsed(name) {
				spec.FailConflict(from, name)
				continue
			}
			deps[override.Rel(from.Repo, from.Pkg).String()] = true
			continue
		}
		matches := ix.FindRulesByImportWithConfig(c, imp, spec.LanguageName)
		switch len(matches) {
		case 1:
			if matches[0].Label != from {
				deps[matches[0].Label.Rel(from.Repo, from.Pkg).String()] = true
			}
		case 0:
			if spec.MarkUsed(name) {
				continue
			}
			spec.FailUnresolved(from, name)
		default:
			spec.FailAmbiguous(from, name, FormatMatches(matches))
		}
	}
	if len(deps) == 0 {
		return
	}
	labels := make([]string, 0, len(deps))
	for dep := range deps {
		labels = append(labels, dep)
	}
	sort.Strings(labels)
	r.SetAttr(spec.Attr, UnionStrings(r.AttrStrings(spec.Attr), labels))
}
