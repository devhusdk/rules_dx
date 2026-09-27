package cc

import (
	"context"
	"flag"
	"fmt"
	"github.com/ralvik/rules_dx/gazelle/common"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const languageName = "cc"

var ccKinds = map[string]rule.KindInfo{
	LibraryKind: rule.KindInfo{
		MatchAttrs:    []string{"srcs", "hdrs"},
		NonEmptyAttrs: map[string]bool{"srcs": true, "hdrs": true},
		MergeableAttrs: map[string]bool{
			"srcs": true,
			"hdrs": true,
			"deps": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type ccLang struct {
	language.BaseLang
	errors  []string
	ignores []*ignoreEntry
}

type ccConfig struct {
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

func NewLanguage() language.Language { return &ccLang{} }

func (l *ccLang) Before(context.Context) { l.errors = nil; l.ignores = nil }

func (l *ccLang) DoneGeneratingRules() {}

func (l *ccLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *ccLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*ccLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *ccLang) Configure(c *config.Config, rel string, f *rule.File) {
	var inherited []*ignoreEntry
	if raw, ok := c.Exts[languageName]; ok {
		inherited = append(inherited, raw.(*ccConfig).ignores...)
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
				l.fail("cc: //%s: malformed # gazelle:dx_ignore_import %s", rel, directive.Value)
			}
		}
	}
	c.Exts[languageName] = &ccConfig{ignores: inherited}
}

func (l *ccLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *ccLang) AfterResolvingDeps(context.Context) {
	for _, ignore := range l.ignores {
		if !ignore.used {
			l.fail("cc: //%s: stale # gazelle:dx_ignore_import cc %s matches no literal reference", ignore.path, ignore.value)
		}
	}
	if len(l.errors) == 0 {
		return
	}
	sort.Strings(l.errors)
	panic("CC generation failed:\n" + strings.Join(l.errors, "\n"))
}

func (*ccLang) Name() string { return languageName }

func (*ccLang) Kinds() map[string]rule.KindInfo { return ccKinds }

func (*ccLang) Loads() []rule.LoadInfo {
	return ccLoads("rules_dx")
}

func (l *ccLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return ccLoads(repoName)
}

func ccLoads(rulesRepo string) []rule.LoadInfo {
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//cc/rules:defs.bzl", Symbols: []string{LibraryKind}},
	}
}

func (*ccLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	var specs []resolve.ImportSpec
	for _, hdr := range r.AttrStrings("hdrs") {
		if !IsHeader(hdr) || IsTestSource(hdr) {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: HeaderIdentity(hdr)})
	}
	return specs
}

func (*ccLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *ccLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	return l.generateRules(args)
}

func (l *ccLang) generateRules(args language.GenerateArgs) language.GenerateResult {
	return common.GenerateSingleDir(args, ccKinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(name string) bool { return IsSource(name) && !IsTestSource(name) },
		IsHeader:     func(name string) bool { return IsHeader(name) && !IsTestSource(name) },
		IsMainFile:   IsSource,
		DirName:      DirTargetName,
		CheckPackage: false,
		DefinesMain:  DefinesMain,
		MainRule:     "cc_binary",
		MainPhrase:   "main-defining sources",
		ParseImports: ParseQuotedIncludes,
		MakeRule: func(name string, sources []string, headers []string) *rule.Rule {
			r := rule.NewRule(LibraryKind, name)
			if len(sources) > 0 {
				r.SetAttr("srcs", sources)
			}
			if len(headers) > 0 {
				r.SetAttr("hdrs", headers)
			}
			return r
		},
		Wrap: func(imports []string) any { return targetImports{imports: imports} },
	}, l)
}

func (l *ccLang) Fail(format string, args ...interface{}) { l.fail(format, args...) }

func (l *ccLang) Failed() bool { return len(l.errors) > 0 }

func (l *ccLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	common.ResolveSingle(c, ix, r, raw, from, common.ResolveSpec{
		LanguageName: languageName,
		WantKind:     LibraryKind,
		Attr:         "deps",
		Unwrap: func(raw any) ([]string, bool) {
			imports, ok := raw.(targetImports)
			if !ok {
				return nil, false
			}
			return imports.imports, true
		},
		MarkUsed: func(name string) bool {
			if ignore := matchingIgnore(c, name); ignore != nil {
				ignore.used = true
				return true
			}
			return false
		},
		FailConflict: func(from label.Label, name string) {
			l.fail("cc: %s: import %q has both an exact resolve mapping and ignore", from, name)
		},
		FailUnresolved: func(from label.Label, name string) {
			l.fail("cc: %s: unresolved import %q; add a local one-header library or an exact # gazelle:resolve mapping", from, name)
		},
		FailAmbiguous: func(from label.Label, name string, matches string) {
			l.fail("cc: %s: ambiguous import %q resolves to %s", from, name, matches)
		},
	})
}

func matchingIgnore(c *config.Config, name string) *ignoreEntry {
	raw, ok := c.Exts[languageName]
	if !ok {
		return nil
	}
	for i := len(raw.(*ccConfig).ignores) - 1; i >= 0; i-- {
		if entry := raw.(*ccConfig).ignores[i]; entry.value == name {
			return entry
		}
	}
	return nil
}

func CollectUsedIgnores(c *config.Config) [][2]string {
	raw, ok := c.Exts[languageName]
	if !ok || raw == nil {
		return nil
	}
	cfg, ok := raw.(*ccConfig)
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
