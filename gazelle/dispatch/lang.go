package dispatch

import (
	"context"
	"flag"
	"fmt"
	"io"
	"os"
	"slices"
	"sort"
	"strings"
	"sync"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"

	"github.com/devhusdk/rules_dx/gazelle/astro"
	"github.com/devhusdk/rules_dx/gazelle/cc"
	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/devhusdk/rules_dx/gazelle/csharp"
	"github.com/devhusdk/rules_dx/gazelle/elixir"
	"github.com/devhusdk/rules_dx/gazelle/fsharp"
	"github.com/devhusdk/rules_dx/gazelle/go"
	"github.com/devhusdk/rules_dx/gazelle/java"
	"github.com/devhusdk/rules_dx/gazelle/javascript"
	"github.com/devhusdk/rules_dx/gazelle/kotlin"
	"github.com/devhusdk/rules_dx/gazelle/mdx"
	"github.com/devhusdk/rules_dx/gazelle/powershell"
	"github.com/devhusdk/rules_dx/gazelle/python"
	"github.com/devhusdk/rules_dx/gazelle/ruby"
	"github.com/devhusdk/rules_dx/gazelle/rust"
	"github.com/devhusdk/rules_dx/gazelle/scala"
	"github.com/devhusdk/rules_dx/gazelle/svelte"
	"github.com/devhusdk/rules_dx/gazelle/typescript"
	"github.com/devhusdk/rules_dx/gazelle/vue"
	"github.com/devhusdk/rules_dx/gazelle/zig"
)

const languageName = "dx_dispatch"

func composedLanguages() []language.Language {
	return []language.Language{
		astro.NewLanguage(),
		cc.NewLanguage(),
		csharp.NewLanguage(),
		elixir.NewLanguage(),
		fsharp.NewLanguage(),
		golang.NewLanguage(),
		java.NewLanguage(),
		javascript.NewLanguage(),
		kotlin.NewLanguage(),
		mdx.NewLanguage(),
		powershell.NewLanguage(),
		python.NewLanguage(),
		ruby.NewLanguage(),
		rust.NewLanguage(),
		scala.NewLanguage(),
		svelte.NewLanguage(),
		typescript.NewLanguage(),
		vue.NewLanguage(),
		zig.NewLanguage(),
	}
}

func unionApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	return unionApparentLoadsFrom(composedLanguages(), moduleToApparentName)
}

func unionApparentLoadsFrom(langs []language.Language, moduleToApparentName func(string) string) []rule.LoadInfo {
	var loads []rule.LoadInfo
	for _, lang := range langs {
		if aware, ok := lang.(language.ModuleAwareLanguage); ok {
			loads = append(loads, aware.ApparentLoads(moduleToApparentName)...)
			continue
		}
		loads = append(loads, lang.Loads()...)
	}
	return loads
}

func ignoreLanguageNames(langs []language.Language) []string {
	seen := map[string]bool{}
	var names []string
	for _, lang := range langs {
		for _, key := range lang.KnownDirectives() {
			if key == "dx_ignore_import" && !seen[lang.Name()] {
				seen[lang.Name()] = true
				names = append(names, lang.Name())
			}
		}
	}
	sort.Strings(names)
	return names
}

var ignoreLanguages = sync.OnceValue(func() []string { return ignoreLanguageNames(composedLanguages()) })

type dispatchLang struct {
	language.BaseLang
	errors   []string
	manifest *common.ManifestRecorder
	configs  []*config.Config
}

func NewLanguage() language.Language { return &dispatchLang{} }

var (
	exitProcess           = os.Exit
	stderr      io.Writer = os.Stderr
)

func (l *dispatchLang) Before(context.Context) {
	l.errors = nil
	common.Errors.Reset()
	l.configs = nil
	l.manifest = nil
	if rec, err := common.LoadManifestRecorder("dispatch"); err != nil {
		l.fail("%v", err)
	} else {
		l.manifest = rec
	}
	if l.manifest != nil {
		l.manifest.IncludeOtherGen = true
		l.manifest.ApparentLoads = unionApparentLoads
	}
}

func (*dispatchLang) DoneGeneratingRules() {}

func (l *dispatchLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *dispatchLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*dispatchLang) KnownDirectives() []string { return nil }

func (l *dispatchLang) Configure(c *config.Config, rel string, f *rule.File) {
	if f == nil {
		return
	}
	if c != nil && len(c.Langs) > 0 && !containsString(c.Langs, languageName) {
		for _, d := range f.Directives {
			if d.Key == "lang" {
				l.fail("lang: //%s: # gazelle:lang %s drops %s; add %s to the list", rel, strings.Join(c.Langs, ","), languageName, languageName)
				break
			}
		}
	}
	known := ignoreLanguages()
	for _, d := range f.Directives {
		if d.Key != "dx_ignore_import" {
			continue
		}
		fields := strings.Fields(d.Value)
		if len(fields) == 0 {
			l.fail("dx_ignore_import: //%s: malformed # gazelle:dx_ignore_import %s; want <language> <import>", rel, d.Value)
			continue
		}
		idx := sort.SearchStrings(known, fields[0])
		if idx == len(known) || known[idx] != fields[0] {
			l.fail("dx_ignore_import: //%s: unknown language %q in # gazelle:dx_ignore_import %s; use one of: %s", rel, fields[0], d.Value, strings.Join(known, ", "))
		}
	}
}

func containsString(list []string, want string) bool {
	for _, v := range list {
		if v == want {
			return true
		}
	}
	return false
}

func (l *dispatchLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *dispatchLang) collectUsedIgnores() []common.IgnoredImport {
	var out []common.IgnoredImport
	seen := map[[3]string]int{}
	emit := func(items []common.IgnoredImport) {
		for _, item := range items {
			key := [3]string{item.Path, item.Language, item.Value}
			if at, ok := seen[key]; ok {
				for _, rel := range item.UsedBy {
					if !slices.Contains(out[at].UsedBy, rel) {
						out[at].UsedBy = append(out[at].UsedBy, rel)
					}
				}
				sort.Strings(out[at].UsedBy)
				continue
			}
			seen[key] = len(out)
			out = append(out, item)
		}
	}
	for _, cfg := range l.configs {
		if cfg == nil {
			continue
		}
		for _, name := range ignoreLanguages() {
			emit(common.UsedIgnores(cfg, name))
		}
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Path != out[j].Path {
			return out[i].Path < out[j].Path
		}
		if out[i].Language != out[j].Language {
			return out[i].Language < out[j].Language
		}
		if out[i].Value != out[j].Value {
			return out[i].Value < out[j].Value
		}
		return strings.Join(out[i].UsedBy, "\x00") < strings.Join(out[j].UsedBy, "\x00")
	})
	return out
}

// AfterResolvingDeps runs last in the dispatch gazelle_binary, after every
// language has added its errors to common.Errors. It emits the manifest only
// when no language failed, and otherwise reports every error once and exits 1.
func (l *dispatchLang) AfterResolvingDeps(context.Context) {
	common.Errors.Add(languageName, "", l.errors...)
	all := common.Errors.Take()
	if len(all) == 0 && l.manifest != nil {
		if err := l.manifest.Emit(l.collectUsedIgnores()); err != nil {
			common.Errors.Add(languageName, "", err.Error())
			all = common.Errors.Take()
		}
	}
	if len(all) > 0 {
		reportFailure(all)
	}
}

// reportFailure is the single failure exit for a generation run.
func reportFailure(all []common.GenerationError) {
	fmt.Fprintln(stderr, "Generation failed:\n"+strings.Join(common.Messages(all), "\n"))
	exitProcess(1)
}

func (*dispatchLang) Name() string { return languageName }

func (*dispatchLang) Kinds() map[string]rule.KindInfo { return nil }

func (l *dispatchLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	if args.Config != nil {
		l.configs = append(l.configs, args.Config)
	}
	for _, msg := range checkAndroid(args) {
		l.fail("%s", msg)
	}
	if l.manifest != nil {
		l.manifest.Record(args, language.GenerateResult{})
	}
	return language.GenerateResult{}
}
