package common

import (
	"bytes"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"slices"
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

// CheckClaims rejects duplicate generated names and names whose existing rule kind, after kindAliases, differs from the claimant's kind.
func CheckClaims(file *rule.File, other []*rule.Rule, claimants []Claimant, kindAliases map[string]string) error {
	byName := make(map[string][]Claimant, len(claimants))
	order := make([]string, 0, len(claimants))
	for _, c := range claimants {
		if _, ok := byName[c.Name]; !ok {
			order = append(order, c.Name)
		}
		byName[c.Name] = append(byName[c.Name], c)
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
		claims := byName[name]
		have, handwritten := existing[name]
		if len(claims) > 1 {
			all := make([]string, 0, len(claims)+1)
			for _, c := range claims {
				all = append(all, c.Source)
			}
			if handwritten {
				all = append(all, "handwritten:"+have+":"+name)
			}
			return &CollisionError{Name: name, Claimants: all}
		}
		if !handwritten {
			continue
		}
		c := claims[0]
		effective := have
		if alias, ok := kindAliases[have]; ok {
			effective = alias
		}
		if effective != c.Kind {
			return fmt.Errorf("target name %q is claimed by generated %s(%s) and existing %s", name, c.Kind, c.Source, have)
		}
	}
	return nil
}

// RuleNamesOfKind returns the names of rules of the given kind in file and other.
func RuleNamesOfKind(file *rule.File, other []*rule.Rule, kind string) map[string]bool {
	names := make(map[string]bool)
	if file != nil {
		for _, r := range file.Rules {
			if r.Kind() == kind {
				names[r.Name()] = true
			}
		}
	}
	for _, r := range other {
		if r.Kind() == kind {
			names[r.Name()] = true
		}
	}
	return names
}

func MergeStale(file *rule.File, result language.GenerateResult, kinds map[string]rule.KindInfo) language.GenerateResult {
	return MergeStaleOwned(file, result, kinds, nil)
}

// MergeStaleOwned is MergeStale for generators that share their kinds with
// handwritten rules: an existing rule of an owned kind is removed only when
// owns reports that generation could have produced it. A nil owns claims every
// rule of an owned kind, as MergeStale does.
func MergeStaleOwned(file *rule.File, result language.GenerateResult, kinds map[string]rule.KindInfo, owns func(*rule.Rule) bool) language.GenerateResult {
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
		if owns != nil && !owns(existing) {
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
	// OwnsByName limits stale removal to rules named like the generated
	// library or naming no existing source, so handwritten rules survive.
	OwnsByName bool
	TargetName   func(name string) (string, error)
	ParseImports func(content []byte, dir string) []ImportRef
	IsStdLib     func(name string) bool
	Candidates   func(name string) []string
	Wrap         func(imports []ImportRef) any
}

func mappedByOverride(c *config.Config, lang string, candidates func(string) []string, name string) bool {
	if c == nil || c.Exts["_resolve"] == nil {
		return false
	}
	names := []string{name}
	if candidates != nil {
		names = append(names, candidates(name)...)
	}
	for _, n := range names {
		if _, found := resolve.FindRuleWithOverride(c, resolve.ImportSpec{Lang: lang, Imp: n}, lang); found {
			return true
		}
	}
	return false
}

func skipStdLib(args language.GenerateArgs, lang string, isStdLib func(string) bool, candidates func(string) []string, name string) bool {
	return isStdLib != nil && isStdLib(name) && !mappedByOverride(args.Config, lang, candidates, name)
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
		imports []ImportRef
	}
	var plans []plan
	for _, src := range sources {
		content, err := ReadSource(filepath.Join(args.Dir, src))
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
		seen := make(map[refKey]bool)
		for _, ref := range spec.ParseImports(content, path.Dir(path.Join(args.Rel, src))) {
			key := refKey{ref.Root, ref.ModuleExt}
			if seen[key] || (!ref.Relative && skipStdLib(args, spec.LanguageName, spec.IsStdLib, spec.Candidates, ref.Root)) {
				continue
			}
			seen[key] = true
			p.imports = append(p.imports, ref)
		}
		SortRefs(p.imports)
		plans = append(plans, p)
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	claimants := make([]Claimant, 0, len(plans))
	for _, p := range plans {
		claimants = append(claimants, Claimant{Name: p.name, Source: p.src, Kind: spec.LibraryKind})
	}
	if err := CheckClaims(args.File, args.OtherGen, claimants, nil); err != nil {
		rep.Fail("%s: %s: %v", spec.LanguageName, args.Rel, err)
		return language.GenerateResult{}
	}
	var result language.GenerateResult
	for _, p := range plans {
		r := rule.NewRule(spec.LibraryKind, p.name)
		r.SetAttr("srcs", []string{p.src})
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, spec.Wrap(append([]ImportRef(nil), p.imports...)))
	}
	if IsFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return MergeStale(args.File, result, kinds)
}

var utf8BOM = []byte{0xEF, 0xBB, 0xBF}

func StripBOM(content []byte) []byte {
	return bytes.TrimPrefix(content, utf8BOM)
}

func ReadSource(path string) ([]byte, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return StripBOM(content), nil
}

type SingleDirSpec struct {
	// OwnsByName limits stale removal to rules named like the generated
	// library or naming no existing source, so handwritten rules survive.
	OwnsByName bool
	LanguageName string
	LibraryKind  string
	IsSource     func(name string) bool
	IsHeader     func(name string) bool
	IsMainFile   func(name string) bool
	DirName      func(dir string) (string, error)
	RootHint     func(args language.GenerateArgs) string
	CheckPackage bool
	ParsePackage func(content []byte) (string, error)
	DefinesMain  func(content []byte) bool
	MainRule     string
	MainPhrase   string
	ParseImports func(content []byte) []string
	IsStdLib     func(name string) bool
	Candidates   func(name string) []string
	OrderSrcs    func(name string, sources []string, contents map[string][]byte) ([]string, error)
	MakeRule     func(name string, sources []string, headers []string) *rule.Rule
	Extend       func(r *rule.Rule, files []string, contents map[string][]byte) error
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
	var hints []string
	if args.Rel == "" && spec.RootHint != nil {
		hints = append(hints, spec.RootHint(args))
	}
	name, err := PackageTargetName(args, hints...)
	if args.Rel != "" {
		name, err = spec.DirName(args.Rel)
	}
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
		content, err := ReadSource(filepath.Join(args.Dir, file))
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
			if skipStdLib(args, spec.LanguageName, spec.IsStdLib, spec.Candidates, root) || seen[root] {
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
		ordered, err = spec.OrderSrcs(name, sources, contents)
		if err != nil {
			rep.Fail("%s: %s: %v", spec.LanguageName, args.Rel, err)
			return language.GenerateResult{}
		}
	}
	if err := CheckClaims(args.File, args.OtherGen, []Claimant{{Name: name, Source: args.Rel, Kind: spec.LibraryKind}}, nil); err != nil {
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
	lib := makeRule(name, ordered, headers)
	if spec.Extend != nil {
		if err := spec.Extend(lib, args.RegularFiles, contents); err != nil {
			rep.Fail("%s: %s: %v", spec.LanguageName, args.Rel, err)
			return language.GenerateResult{}
		}
	}
	result.Gen = append(result.Gen, lib)
	result.Imports = append(result.Imports, spec.Wrap(imports))
	if IsFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	var owns func(*rule.Rule) bool
	if spec.OwnsByName {
		owns = func(r *rule.Rule) bool {
			if r.Name() == name {
				return true
			}
			for _, attr := range []string{"srcs", "hdrs"} {
				for _, src := range r.AttrStrings(attr) {
					if _, err := os.Stat(filepath.Join(args.Dir, filepath.FromSlash(src))); err == nil {
						return false
					}
				}
			}
			return true
		}
	}
	return MergeStaleOwned(args.File, result, kinds, owns)
}

type ResolveSpec struct {
	LanguageName   string
	WantKind       string
	Attr           string
	Unwrap         func(raw any) ([]string, bool)
	// UnwrapRefs, when set and matching, takes precedence over Unwrap and
	// gives import refs whose relative entries may use Fallback.
	UnwrapRefs func(raw any) ([]ImportRef, bool)
	// Fallback lists the languages whose rule index and overrides a relative
	// import searches, in order, when the own language has no match.
	Fallback   []string
	IsStdLib   func(name string) bool
	Candidates func(name string) []string
	Lookups    func(pkg, name string) []string
	MarkUsed   func(name string) bool
	// SkipDep, when set and true, drops a resolved dep. The rule itself is
	// always dropped.
	SkipDep func(dep label.Label) bool
	// External looks a non-relative import up in the third-party sources
	// after the index and ignores. It returns the labels found, or a hint.
	External func(name string) ([]label.Label, string)
	FailConflict   func(from label.Label, name string)
	FailUnresolved func(from label.Label, name string, hint string)
	FailAmbiguous  func(from label.Label, name string, matches string)
}

func ResolveSingle(c *config.Config, ix *resolve.RuleIndex, r *rule.Rule, raw any, from label.Label, spec ResolveSpec) {
	refs, ok := unwrapRefs(raw, spec)
	if !ok {
		return
	}
	if r.Kind() != spec.WantKind {
		return
	}
	deps := make(map[string]bool)
	add := func(dep label.Label) {
		if dep == from || (spec.SkipDep != nil && spec.SkipDep(dep)) {
			return
		}
		deps[dep.Rel(from.Repo, from.Pkg).String()] = true
	}
	for _, ref := range refs {
		name := ref.Root
		imp := resolve.ImportSpec{Lang: spec.LanguageName, Imp: name}
		override, found := resolve.FindRuleWithOverride(c, imp, spec.LanguageName)
		if !found && !ref.Relative && spec.IsStdLib != nil && spec.IsStdLib(name) {
			continue
		}
		if found {
			if spec.MarkUsed(name) {
				spec.FailConflict(from, name)
				continue
			}
			add(override)
			continue
		}
		// A relative import naming a JavaScript or TypeScript extension can
		// never be a source of this language, so it skips the own index.
		ownIndex := !ref.Relative || !slices.Contains(JSModuleExts, ref.ModuleExt)
		keys := []string{name}
		if spec.Lookups != nil {
			keys = spec.Lookups(from.Pkg, name)
		}
		var matches []resolve.FindResult
		mappedKey := false
		for _, key := range keys {
			if !ownIndex {
				break
			}
			kimp := resolve.ImportSpec{Lang: spec.LanguageName, Imp: key}
			if key != name {
				if override, found := resolve.FindRuleWithOverride(c, kimp, spec.LanguageName); found {
					add(override)
					mappedKey = true
					break
				}
			}
			if matches = ix.FindRulesByImportWithConfig(c, kimp, spec.LanguageName); len(matches) > 0 {
				break
			}
		}
		if mappedKey {
			continue
		}
		if len(matches) == 0 && ownIndex {
			var cands []string
			if spec.Candidates != nil {
				cands = spec.Candidates(name)
			}
			mapped := false
			for _, cand := range cands {
				cimp := resolve.ImportSpec{Lang: spec.LanguageName, Imp: cand}
				if override, found := resolve.FindRuleWithOverride(c, cimp, spec.LanguageName); found {
					add(override)
					mapped = true
					break
				}
				if matches = ix.FindRulesByImportWithConfig(c, cimp, spec.LanguageName); len(matches) > 0 {
					break
				}
			}
			if mapped {
				continue
			}
		}
		if len(matches) == 0 && ref.Relative && len(spec.Fallback) > 0 {
			fb, mapped := findFallback(c, ix, spec.Fallback, name, from)
			if mapped {
				add(fb.Override)
				continue
			}
			matches = fb.Matches
			if len(matches) == 0 && fb.SelfMatch {
				continue
			}
		}
		if len(matches) == 0 && ref.Relative && ref.Ext != "" && len(spec.Fallback) > 0 {
			// An asset no target owns is left to the bundler. A matching
			// ignore is still marked used so it never reads as stale.
			spec.MarkUsed(name)
			continue
		}
		switch len(matches) {
		case 1:
			add(matches[0].Label)
		case 0:
			if spec.MarkUsed(name) {
				continue
			}
			hint := ""
			if spec.External != nil && !ref.Relative {
				var ext []label.Label
				ext, hint = spec.External(name)
				switch {
				case len(ext) == 1:
					if ext[0] != from {
						deps[ext[0].Rel(from.Repo, from.Pkg).String()] = true
					}
					continue
				case len(ext) > 1:
					spec.FailAmbiguous(from, name, formatLabels(ext))
					continue
				}
			}
			spec.FailUnresolved(from, name, hint)
		default:
			if slices.ContainsFunc(matches, func(m resolve.FindResult) bool { return m.Label == from }) {
				continue
			}
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

func unwrapRefs(raw any, spec ResolveSpec) ([]ImportRef, bool) {
	if spec.UnwrapRefs != nil {
		if refs, ok := spec.UnwrapRefs(raw); ok {
			return refs, true
		}
	}
	if spec.Unwrap == nil {
		return nil, false
	}
	names, ok := spec.Unwrap(raw)
	if !ok {
		return nil, false
	}
	refs := make([]ImportRef, 0, len(names))
	for _, name := range names {
		refs = append(refs, ImportRef{Spec: name, Root: name})
	}
	return refs, true
}

type fallbackLookup struct {
	Override  label.Label
	Matches   []resolve.FindResult
	SelfMatch bool
}

// findFallback looks a relative import up in each fallback language. The first
// override, in fallback order, wins. Otherwise index matches from every
// fallback language are combined with from itself left out; SelfMatch reports
// that from was the only match.
func findFallback(c *config.Config, ix *resolve.RuleIndex, langs []string, name string, from label.Label) (fallbackLookup, bool) {
	var out fallbackLookup
	seen := make(map[label.Label]bool)
	for _, lang := range langs {
		found := FindJSImport(c, ix, lang, name, true)
		if found.HasOverride {
			return fallbackLookup{Override: found.Override}, true
		}
		for _, m := range found.Matches {
			if m.Label == from {
				out.SelfMatch = true
				continue
			}
			if !seen[m.Label] {
				seen[m.Label] = true
				out.Matches = append(out.Matches, m)
			}
		}
	}
	if len(out.Matches) > 0 {
		out.SelfMatch = false
	}
	return out, false
}

func formatLabels(labels []label.Label) string {
	out := make([]string, 0, len(labels))
	for _, l := range labels {
		out = append(out, l.String())
	}
	sort.Strings(out)
	return fmt.Sprintf("[%s]", strings.Join(out, ", "))
}
