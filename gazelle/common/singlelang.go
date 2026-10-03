package common

import (
	"context"
	"flag"
	"fmt"
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

type SingleLangSpec struct {
	LanguageName   string
	DisplayName    string
	LibraryKind    string
	Kinds          map[string]rule.KindInfo
	DefsPackage    string
	ImportAttr     string
	Identity       func(name string) string
	IsImportSource func(name string) bool
	IsTestSource   func(name string) bool
	IsStdLib       func(name string) bool
	UnresolvedHint string
	Generate       func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep Reporter) language.GenerateResult
}

type SingleLang struct {
	language.BaseLang
	spec    SingleLangSpec
	errors  []string
	ignores []*IgnoreEntry
}

func NewSingleLang(spec SingleLangSpec) *SingleLang { return &SingleLang{spec: spec} }

func (l *SingleLang) Before(context.Context) { l.errors = nil; l.ignores = nil }

func (l *SingleLang) DoneGeneratingRules() {}

func (l *SingleLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *SingleLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*SingleLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *SingleLang) Configure(c *config.Config, rel string, f *rule.File) {
	declared, errs := DeclaredIgnores(l.spec.LanguageName, rel, f)
	for _, msg := range errs {
		l.fail("%s", msg)
	}
	l.ignores = append(l.ignores, declared...)
	c.Exts[l.spec.LanguageName] = &IgnoreConfig{Ignores: MergeIgnores(InheritedIgnores(c, l.spec.LanguageName), declared)}
}

func (l *SingleLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *SingleLang) AfterResolvingDeps(context.Context) {
	for _, msg := range StaleIgnores(l.spec.LanguageName, l.ignores) {
		l.fail("%s", msg)
	}
	if len(l.errors) == 0 {
		return
	}
	sort.Strings(l.errors)
	panic(l.spec.DisplayName + " generation failed:\n" + strings.Join(l.errors, "\n"))
}

func (l *SingleLang) Name() string { return l.spec.LanguageName }

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
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//" + l.spec.DefsPackage, Symbols: []string{l.spec.LibraryKind}},
	}
}

func (l *SingleLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != l.spec.LibraryKind {
		return nil
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings(l.spec.ImportAttr) {
		if !l.spec.IsImportSource(src) {
			continue
		}
		if l.spec.IsTestSource != nil && l.spec.IsTestSource(src) {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: l.spec.LanguageName, Imp: l.spec.Identity(src)})
	}
	return specs
}

func (*SingleLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *SingleLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	return l.spec.Generate(args, l.spec.Kinds, l)
}

func (l *SingleLang) Fail(format string, args ...interface{}) { l.fail(format, args...) }

func (l *SingleLang) Failed() bool { return len(l.errors) > 0 }

func (l *SingleLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	ResolveSingle(c, ix, r, raw, from, ResolveSpec{
		LanguageName: l.spec.LanguageName,
		WantKind:     l.spec.LibraryKind,
		Attr:         "deps",
		Unwrap:       UnwrapImportSet,
		IsStdLib:     l.spec.IsStdLib,
		MarkUsed: func(name string) bool {
			if ignore := MatchingIgnore(c, l.spec.LanguageName, name); ignore != nil {
				ignore.Used = true
				return true
			}
			return false
		},
		FailConflict: func(from label.Label, name string) {
			l.fail("%s: %s: import %q has both an exact resolve mapping and ignore", l.spec.LanguageName, from, name)
		},
		FailUnresolved: func(from label.Label, name string) {
			l.fail("%s: %s: unresolved import %q; %s", l.spec.LanguageName, from, name, l.spec.UnresolvedHint)
		},
		FailAmbiguous: func(from label.Label, name string, matches string) {
			l.fail("%s: %s: ambiguous import %q resolves to %s", l.spec.LanguageName, from, name, matches)
		},
	})
}

func HasExt(name string, exts []string) bool {
	for _, ext := range exts {
		if strings.HasSuffix(name, ext) {
			return true
		}
	}
	return false
}
