package dispatch

import (
	"context"
	"flag"
	"fmt"
	"os"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"

	"github.com/ralvik/rules_dx/gazelle/astro"
	"github.com/ralvik/rules_dx/gazelle/cc"
	"github.com/ralvik/rules_dx/gazelle/common"
	"github.com/ralvik/rules_dx/gazelle/csharp"
	"github.com/ralvik/rules_dx/gazelle/fsharp"
	"github.com/ralvik/rules_dx/gazelle/go"
	"github.com/ralvik/rules_dx/gazelle/java"
	"github.com/ralvik/rules_dx/gazelle/javascript"
	"github.com/ralvik/rules_dx/gazelle/kotlin"
	"github.com/ralvik/rules_dx/gazelle/mdx"
	"github.com/ralvik/rules_dx/gazelle/python"
	"github.com/ralvik/rules_dx/gazelle/ruby"
	"github.com/ralvik/rules_dx/gazelle/rust"
	"github.com/ralvik/rules_dx/gazelle/scala"
	"github.com/ralvik/rules_dx/gazelle/svelte"
	"github.com/ralvik/rules_dx/gazelle/typescript"
	"github.com/ralvik/rules_dx/gazelle/vue"
)

const languageName = "dx_dispatch"

func composedLanguages() []language.Language {
	return []language.Language{
		astro.NewLanguage(),
		cc.NewLanguage(),
		csharp.NewLanguage(),
		fsharp.NewLanguage(),
		golang.NewLanguage(),
		java.NewLanguage(),
		javascript.NewLanguage(),
		kotlin.NewLanguage(),
		mdx.NewLanguage(),
		python.NewLanguage(),
		ruby.NewLanguage(),
		rust.NewLanguage(),
		scala.NewLanguage(),
		svelte.NewLanguage(),
		typescript.NewLanguage(),
		vue.NewLanguage(),
	}
}

func unionApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	var loads []rule.LoadInfo
	for _, lang := range composedLanguages() {
		if aware, ok := lang.(language.ModuleAwareLanguage); ok {
			loads = append(loads, aware.ApparentLoads(moduleToApparentName)...)
			continue
		}
		loads = append(loads, lang.Loads()...)
	}
	return loads
}

type dispatchLang struct {
	language.BaseLang
	errors   []string
	manifest *common.ManifestRecorder
	configs  []*config.Config
}

func NewLanguage() language.Language { return &dispatchLang{} }

var exitProcess = os.Exit

func (l *dispatchLang) Before(context.Context) {
	l.errors = nil
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

func (l *dispatchLang) Configure(*config.Config, string, *rule.File) {}

func (l *dispatchLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *dispatchLang) collectUsedIgnores() []common.IgnoredImport {
	var out []common.IgnoredImport
	seen := map[common.IgnoredImport]bool{}
	emit := func(language string, pairs [][2]string) {
		for _, pair := range pairs {
			entry := common.IgnoredImport{Path: pair[0], Language: language, Value: pair[1]}
			if seen[entry] {
				continue
			}
			seen[entry] = true
			out = append(out, entry)
		}
	}
	for _, cfg := range l.configs {
		if cfg == nil {
			continue
		}
		emit("astro", astro.CollectUsedIgnores(cfg))
		emit("cc", cc.CollectUsedIgnores(cfg))
		emit("csharp", csharp.CollectUsedIgnores(cfg))
		emit("fsharp", fsharp.CollectUsedIgnores(cfg))
		emit("go", golang.CollectUsedIgnores(cfg))
		emit("java", java.CollectUsedIgnores(cfg))
		emit("javascript", javascript.CollectUsedIgnores(cfg))
		emit("kotlin", kotlin.CollectUsedIgnores(cfg))
		emit("mdx", mdx.CollectUsedIgnores(cfg))
		emit("python", python.CollectUsedIgnores(cfg))
		emit("ruby", ruby.CollectUsedIgnores(cfg))
		emit("rust", rust.CollectUsedIgnores(cfg))
		emit("scala", scala.CollectUsedIgnores(cfg))
		emit("svelte", svelte.CollectUsedIgnores(cfg))
		emit("typescript", typescript.CollectUsedIgnores(cfg))
		emit("vue", vue.CollectUsedIgnores(cfg))
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Path != out[j].Path {
			return out[i].Path < out[j].Path
		}
		if out[i].Language != out[j].Language {
			return out[i].Language < out[j].Language
		}
		return out[i].Value < out[j].Value
	})
	return out
}

func (l *dispatchLang) AfterResolvingDeps(context.Context) {
	if len(l.errors) == 0 && l.manifest != nil {
		if err := l.manifest.Emit(l.collectUsedIgnores()); err != nil {
			l.fail("%v", err)
		}
	}
	if len(l.errors) > 0 {
		sort.Strings(l.errors)
		fmt.Fprintln(os.Stderr, "Dispatch generation failed before BUILD emission:\n"+strings.Join(l.errors, "\n"))
		exitProcess(1)
	}
}

func (*dispatchLang) Name() string { return languageName }

func (*dispatchLang) Kinds() map[string]rule.KindInfo { return nil }

func (l *dispatchLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	if args.Config != nil {
		l.configs = append(l.configs, args.Config)
	}
	if l.manifest != nil {
		l.manifest.Record(args, language.GenerateResult{})
	}
	return language.GenerateResult{}
}
