package vue

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
	"github.com/ralvik/rules_dx/gazelle/common"
)

const (
	languageName = "vue"
	libraryKind  = "vue_library"
)

var vueKinds = map[string]rule.KindInfo{
	libraryKind: common.LibraryKindInfo(),
}

type vueLang struct {
	language.BaseLang
	errors  []string
	ignores []*ignoreEntry
}

type vueConfig struct {
	ignores []*ignoreEntry
}

type ignoreEntry struct {
	value string
	path  string
	used  bool
}

type targetImports struct {
	imports []string
}

func NewLanguage() language.Language { return &vueLang{} }

func (l *vueLang) Before(context.Context) { l.errors = nil; l.ignores = nil }

func (l *vueLang) DoneGeneratingRules() {}

func (l *vueLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *vueLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*vueLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *vueLang) Configure(c *config.Config, rel string, f *rule.File) {
	var inherited []*ignoreEntry
	if raw, ok := c.Exts[languageName]; ok {
		inherited = append(inherited, raw.(*vueConfig).ignores...)
	}
	if f != nil {
		for _, directive := range f.Directives {
			if directive.Key != "dx_ignore_import" {
				continue
			}
			fields := strings.Fields(directive.Value)
			if len(fields) == 2 && fields[0] == languageName {
				entry := &ignoreEntry{value: fields[1], path: rel}
				inherited = append(inherited, entry)
				l.ignores = append(l.ignores, entry)
			} else if len(fields) == 3 && fields[0] == languageName && fields[1] == languageName {
				entry := &ignoreEntry{value: fields[2], path: rel}
				inherited = append(inherited, entry)
				l.ignores = append(l.ignores, entry)
			} else if len(fields) > 0 && fields[0] == languageName {
				l.fail("vue: //%s: malformed # gazelle:dx_ignore_import %s", rel, directive.Value)
			}
		}
	}
	c.Exts[languageName] = &vueConfig{ignores: inherited}
}

func (l *vueLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *vueLang) AfterResolvingDeps(context.Context) {
	for _, ignore := range l.ignores {
		if !ignore.used {
			l.fail("vue: //%s: stale # gazelle:dx_ignore_import vue %s matches no literal reference", ignore.path, ignore.value)
		}
	}
	if len(l.errors) == 0 {
		return
	}
	sort.Strings(l.errors)
	panic("Vue generation failed:\n" + strings.Join(l.errors, "\n"))
}

func (*vueLang) Name() string { return languageName }

func (*vueLang) Kinds() map[string]rule.KindInfo { return vueKinds }

func (*vueLang) Loads() []rule.LoadInfo {
	return vueLoads("rules_dx")
}

func (l *vueLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return vueLoads(repoName)
}

func vueLoads(rulesRepo string) []rule.LoadInfo {
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//vue/rules:defs.bzl", Symbols: []string{libraryKind}},
	}
}

func (*vueLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != libraryKind {
		return nil
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if !isSupported(src) {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: ModuleName(src)})
	}
	return specs
}

func (*vueLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *vueLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	return l.generateRules(args)
}

func isSupported(name string) bool {
	for _, ext := range SupportedExts {
		if strings.HasSuffix(name, ext) {
			return true
		}
	}
	return false
}

func (l *vueLang) generateRules(args language.GenerateArgs) language.GenerateResult {
	return common.GenerateSingleFile(args, vueKinds, common.SingleFileSpec{
		LanguageName: languageName,
		LibraryKind:  libraryKind,
		IsSource:     isSupported,
		TargetName:   TargetName,
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Wrap:         func(imports []string) any { return targetImports{imports: imports} },
	}, l)
}

func (l *vueLang) Fail(format string, args ...interface{}) { l.fail(format, args...) }

func (l *vueLang) Failed() bool { return len(l.errors) > 0 }

func unionStrings(a, b []string) []string { return common.UnionStrings(a, b) }

func matchingIgnore(c *config.Config, name string) *ignoreEntry {
	raw, ok := c.Exts[languageName]
	if !ok {
		return nil
	}
	for i := len(raw.(*vueConfig).ignores) - 1; i >= 0; i-- {
		if entry := raw.(*vueConfig).ignores[i]; entry.value == name {
			return entry
		}
	}
	return nil
}

func checkClaims(file *rule.File, other []*rule.Rule, claimants []Claimant) error {
	return common.CheckClaimsFile(file, other, claimants, libraryKind)
}

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStale(file, result, vueKinds)
}

func (l *vueLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	common.ResolveSingle(c, ix, r, raw, from, common.ResolveSpec{
		LanguageName: languageName,
		WantKind:     libraryKind,
		Attr:         "deps",
		Unwrap: func(raw any) ([]string, bool) {
			imports, ok := raw.(targetImports)
			if !ok {
				return nil, false
			}
			return imports.imports, true
		},
		IsStdLib: IsStdLib,
		MarkUsed: func(name string) bool {
			if ignore := matchingIgnore(c, name); ignore != nil {
				ignore.used = true
				return true
			}
			return false
		},
		FailConflict: func(from label.Label, name string) {
			l.fail("vue: %s: import %q has both an exact resolve mapping and ignore", from, name)
		},
		FailUnresolved: func(from label.Label, name string) {
			l.fail("vue: %s: unresolved import %q; add a local one-source library or an exact # gazelle:resolve mapping", from, name)
		},
		FailAmbiguous: func(from label.Label, name string, matches string) {
			l.fail("vue: %s: ambiguous import %q resolves to %s", from, name, matches)
		},
	})
}

func CollectUsedIgnores(c *config.Config) [][2]string {
	raw, ok := c.Exts[languageName]
	if !ok || raw == nil {
		return nil
	}
	cfg, ok := raw.(*vueConfig)
	if !ok || cfg == nil {
		return nil
	}
	seen := map[[2]string]bool{}
	var out [][2]string
	for _, ig := range cfg.ignores {
		if ig == nil || !ig.used {
			continue
		}
		key := [2]string{ig.path, ig.value}
		if seen[key] {
			continue
		}
		seen[key] = true
		out = append(out, key)
	}
	return out
}
