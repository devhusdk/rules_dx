package common

import (
	"context"
	"flag"
	"fmt"
	"path/filepath"
	"slices"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

type ImportSet struct {
	Names []string
}

func WrapImportSet(names []string) any { return ImportSet{Names: names} }

func UnwrapImportSet(raw any) ([]string, bool) {
	set, ok := raw.(ImportSet)
	if !ok {
		return nil, false
	}
	return set.Names, true
}

// RefImportSet carries the import refs of one generated rule for languages
// that resolve relative imports with a fallback.
type RefImportSet struct {
	Refs []ImportRef
}

func WrapRefImportSet(refs []ImportRef) any { return RefImportSet{Refs: refs} }

func UnwrapRefImportSet(raw any) ([]ImportRef, bool) {
	set, ok := raw.(RefImportSet)
	if !ok {
		return nil, false
	}
	return set.Refs, true
}

// Roots returns the Root of each ref, in order.
func (s RefImportSet) Roots() []string { return RefRoots(s.Refs) }

type SingleLangSpec struct {
	LanguageName string
	DisplayName  string
	LibraryKind  string
	ExtraKinds   []string
	Kinds        map[string]rule.KindInfo
	DefsPackage  string
	ImportAttr   string
	// Identity maps a source file name to the import it is indexed under.
	// Nil indexes no per-file identity, so only QualifiedIDs index the rule.
	Identity       func(name string) string
	PathIdentity   bool
	IsImportSource func(name string) bool
	IsTestSource   func(name string) bool
	IsStdLib       func(name string) bool
	UnresolvedHint string
	QualifiedIDs   func(path string) []string
	Candidates     func(name string) []string
	// Fallback lists the languages a relative import also searches when the
	// own index has no match.
	Fallback []string
	Lookups  func(pkg, name string) []string
	Generate func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep Reporter) language.GenerateResult

	// KindDefs maps a kind to the defs package that exports it. Kinds without
	// an entry load from DefsPackage.
	KindDefs map[string]string
	// IndexKinds are extra kinds whose ImportAttr sources Imports indexes.
	IndexKinds []string
	// IsIndexSource replaces IsImportSource for IndexKinds when set.
	IsIndexSource func(name string) bool
	// Directives are extra Gazelle directives the language accepts.
	Directives []string
	// External is the ecosystem whose dx_external sources resolve
	// third-party imports. Empty turns the lookup off.
	External string
	// ConfigureExt runs at the end of Configure for the language's own state.
	ConfigureExt func(c *config.Config, rel string, f *rule.File, rep Reporter)
}

type SingleLang struct {
	language.BaseLang
	spec      SingleLangSpec
	errors    []string
	ignores   []*IgnoreEntry
	generated GeneratedRels
}

func NewSingleLang(spec SingleLangSpec) *SingleLang { return &SingleLang{spec: spec} }

func (l *SingleLang) Before(context.Context) {
	l.errors = nil
	l.ignores = nil
	l.generated = GeneratedRels{}
	Errors.Reset()
}

func (l *SingleLang) DoneGeneratingRules() {}

func (l *SingleLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *SingleLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (l *SingleLang) KnownDirectives() []string {
	known := []string{"dx_ignore_import"}
	if l.spec.External != "" {
		known = append(known, ExternalDirective)
	}
	return append(known, l.spec.Directives...)
}

func (l *SingleLang) Configure(c *config.Config, rel string, f *rule.File) {
	declared, errs := DeclaredIgnores(l.spec.LanguageName, rel, f)
	for _, msg := range errs {
		l.fail("%s", msg)
	}
	l.ignores = append(l.ignores, declared...)
	c.Exts[l.spec.LanguageName] = &IgnoreConfig{Ignores: MergeIgnores(InheritedIgnores(c, l.spec.LanguageName), declared)}
	if l.spec.External != "" {
		for _, msg := range ConfigureExternal(c, rel, f) {
			l.fail("%s", msg)
		}
	}
	if l.spec.ConfigureExt != nil {
		l.spec.ConfigureExt(c, rel, f, l)
	}
}

func (l *SingleLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *SingleLang) AfterResolvingDeps(context.Context) {
	for _, msg := range StaleIgnores(l.spec.LanguageName, l.ignores, l.generated) {
		l.fail("%s", msg)
	}
	Errors.Add(l.spec.LanguageName, "", l.errors...)
}

func (l *SingleLang) Name() string { return l.spec.LanguageName }

// Fallback returns the languages a relative import also searches.
func (l *SingleLang) Fallback() []string { return l.spec.Fallback }

func (l *SingleLang) Kinds() map[string]rule.KindInfo { return l.spec.Kinds }

func (l *SingleLang) Loads() []rule.LoadInfo { return l.loads("rules_dx") }

func (l *SingleLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return l.loads(repoName)
}

func (l *SingleLang) loads(rulesRepo string) []rule.LoadInfo {
	bySymbols := map[string][]string{}
	for _, kind := range append([]string{l.spec.LibraryKind}, l.spec.ExtraKinds...) {
		pkg := l.spec.DefsPackage
		if p, ok := l.spec.KindDefs[kind]; ok {
			pkg = p
		}
		bySymbols[pkg] = append(bySymbols[pkg], kind)
	}
	pkgs := make([]string, 0, len(bySymbols))
	for pkg := range bySymbols {
		if pkg != l.spec.DefsPackage {
			pkgs = append(pkgs, pkg)
		}
	}
	sort.Strings(pkgs)
	pkgs = append([]string{l.spec.DefsPackage}, pkgs...)
	var loads []rule.LoadInfo
	for _, pkg := range pkgs {
		if symbols := bySymbols[pkg]; len(symbols) > 0 {
			loads = append(loads, rule.LoadInfo{Name: "@" + rulesRepo + "//" + pkg, Symbols: symbols})
		}
	}
	return loads
}

func (l *SingleLang) Imports(_ *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() != l.spec.LibraryKind && !slices.Contains(l.spec.IndexKinds, r.Kind()) {
		return nil
	}
	isSource := l.spec.IsImportSource
	if r.Kind() != l.spec.LibraryKind && l.spec.IsIndexSource != nil {
		isSource = l.spec.IsIndexSource
	}
	var specs []resolve.ImportSpec
	seen := make(map[string]bool)
	add := func(imp string) {
		if !seen[imp] {
			seen[imp] = true
			specs = append(specs, resolve.ImportSpec{Lang: l.spec.LanguageName, Imp: imp})
		}
	}
	for _, src := range r.AttrStrings(l.spec.ImportAttr) {
		if !isSource(src) {
			continue
		}
		if l.spec.IsTestSource != nil && l.spec.IsTestSource(src) {
			continue
		}
		if l.spec.Identity != nil {
			imp := l.spec.Identity(src)
			if l.spec.PathIdentity && f != nil {
				imp = ModuleIdentity(f.Pkg, imp)
			}
			add(imp)
		}
		if l.spec.QualifiedIDs != nil && f != nil {
			for _, id := range l.spec.QualifiedIDs(filepath.Join(filepath.Dir(f.Path), src)) {
				add(id)
			}
		}
	}
	return specs
}

func (*SingleLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *SingleLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	l.generated.Add(args.Rel)
	return l.spec.Generate(args, l.spec.Kinds, l)
}

func (l *SingleLang) Fail(format string, args ...interface{}) { l.fail(format, args...) }

func (l *SingleLang) Failed() bool { return len(l.errors) > 0 }

// Errors returns a copy of the errors reported so far.
func (l *SingleLang) Errors() []string { return slices.Clone(l.errors) }

func (l *SingleLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	ResolveSingle(c, ix, r, raw, from, l.ResolveSpecFor(c, r, from))
}

// ResolveSpecFor builds the ResolveSpec the driver resolves r with. Languages
// that embed SingleLang adjust it for their own hooks before ResolveSingle.
func (l *SingleLang) ResolveSpecFor(c *config.Config, r *rule.Rule, from label.Label) ResolveSpec {
	want := l.spec.LibraryKind
	if slices.Contains(l.spec.ExtraKinds, r.Kind()) {
		want = r.Kind()
	}
	var external func(name string) ([]label.Label, string)
	if eco := l.spec.External; eco != "" {
		external = func(name string) ([]label.Label, string) { return ExternalLabels(c, eco, name) }
	}
	return ResolveSpec{
		LanguageName:   l.spec.LanguageName,
		WantKind:       want,
		Attr:           "deps",
		Unwrap:         UnwrapImportSet,
		UnwrapRefs:     UnwrapRefImportSet,
		Fallback:       l.spec.Fallback,
		IsStdLib:       l.spec.IsStdLib,
		Candidates:     l.spec.Candidates,
		Lookups:        l.spec.Lookups,
		External:       external,
		MarkUsed: func(name string) bool {
			if ignore := MatchingIgnore(c, l.spec.LanguageName, name); ignore != nil {
				ignore.MarkUsed(from.Pkg)
				return true
			}
			return false
		},
		FailConflict: func(from label.Label, name string) {
			l.fail("%s: %s: import %q has both an exact resolve mapping and ignore", l.spec.LanguageName, from, name)
		},
		FailUnresolved: func(from label.Label, name string, hint string) {
			if hint != "" {
				hint += "; "
			}
			l.fail("%s", UnresolvedImport(c, l.spec.LanguageName, from, name, hint+l.spec.UnresolvedHint))
		},
		FailAmbiguous: func(from label.Label, name string, matches string) {
			l.fail("%s", AmbiguousImport(c, l.spec.LanguageName, from, name, matches))
		},
	}
}

func HasExt(name string, exts []string) bool {
	for _, ext := range exts {
		if strings.HasSuffix(name, ext) {
			return true
		}
	}
	return false
}
