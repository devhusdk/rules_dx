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

	"github.com/devhusdk/rules_dx/gazelle/common"
)

const languageName = "dx_dispatch"

func composedLanguages() []language.Language { return common.RegisteredLanguages() }

func unionApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	return unionApparentLoadsFrom(composedLanguages(), moduleToApparentName)
}

func unionApparentLoadsFrom(langs []language.Language, moduleToApparentName func(string) string) []rule.LoadInfo {
	var loads []rule.LoadInfo
	for _, lang := range langs {
		loads = append(loads, lang.(language.ModuleAwareLanguage).ApparentLoads(moduleToApparentName)...)
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

var knownDirectives = sync.OnceValue(func() map[string]bool {
	known := map[string]bool{common.FixtureTestonlyDirective: true}
	for _, lang := range composedLanguages() {
		for _, key := range lang.KnownDirectives() {
			known[key] = true
		}
	}
	return known
})

type dispatchLang struct {
	language.BaseLang
	errors   []common.GenerationError
	manifest *common.ManifestRecorder
	configs  []*config.Config
	snapshot common.BuildSnapshot
}

func NewLanguage() language.Language { return &dispatchLang{} }

var (
	exitProcess           = os.Exit
	stderr      io.Writer = os.Stderr
)

func (l *dispatchLang) Before(context.Context) {
	l.errors = nil
	common.Errors.Reset()
	l.snapshot.Reset()
	l.configs = nil
	l.manifest = nil
	if rec, err := common.LoadManifestRecorder("dispatch"); err != nil {
		l.fail(common.UnknownPackage, "%v", err)
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

func (*dispatchLang) KnownDirectives() []string { return []string{common.FixtureTestonlyDirective} }

func (l *dispatchLang) Configure(c *config.Config, rel string, f *rule.File) {
	if f == nil {
		return
	}
	if c != nil && len(c.Langs) > 0 && !containsString(c.Langs, languageName) {
		for _, d := range f.Directives {
			if d.Key == "lang" {
				l.fail(common.InPackage(rel), "lang: //%s: # gazelle:lang %s drops %s; add %s to the list", rel, strings.Join(c.Langs, ","), languageName, languageName)
				break
			}
		}
	}
	for _, d := range f.Directives {
		if d.Key != common.FixtureTestonlyDirective {
			continue
		}
		on, err := common.ParseBoolDirective(d.Value)
		if err != nil {
			l.fail(common.InPackage(rel), "%s: //%s: %v", common.FixtureTestonlyDirective, rel, err)
			continue
		}
		if c != nil {
			common.SetFixtureTestonly(c, on)
		}
	}
	declared := knownDirectives()
	for _, d := range f.Directives {
		if strings.HasPrefix(d.Key, "dx_") && !declared[d.Key] {
			l.fail(common.InPackage(rel), "%s: //%s: unknown directive # gazelle:%s", d.Key, rel, d.Key)
		}
	}
	known := ignoreLanguages()
	for _, d := range f.Directives {
		if d.Key != "dx_ignore_import" {
			continue
		}
		fields := strings.Fields(d.Value)
		if len(fields) != 2 {
			l.fail(common.InPackage(rel), "dx_ignore_import: //%s: malformed # gazelle:dx_ignore_import %s; want <language> <import>", rel, d.Value)
			continue
		}
		idx := sort.SearchStrings(known, fields[0])
		if idx == len(known) || known[idx] != fields[0] {
			l.fail(common.InPackage(rel), "dx_ignore_import: //%s: unknown language %q in # gazelle:dx_ignore_import %s; use one of: %s", rel, fields[0], d.Value, strings.Join(known, ", "))
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

func (l *dispatchLang) fail(pkg common.PackageRef, format string, args ...interface{}) {
	l.errors = append(l.errors, common.GenerationError{Language: languageName, Package: pkg, Message: fmt.Sprintf(format, args...)})
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
// when no language failed, and otherwise restores every visited BUILD file,
// reports every error once and exits 1. Gazelle 0.54 runs this hook after
// BUILD files are written, so the restore keeps failed runs side-effect free.
// A kill between the write and the restore is not covered.
func (l *dispatchLang) AfterResolvingDeps(context.Context) {
	common.Errors.Add(l.errors...)
	all := common.Errors.Take()
	if len(all) == 0 && l.manifest != nil {
		if err := l.manifest.Emit(l.collectUsedIgnores()); err != nil {
			common.Errors.Add(common.GenerationError{Language: languageName, Package: common.UnknownPackage, Message: err.Error()})
			all = common.Errors.Take()
		}
	}
	if len(all) > 0 {
		if err := l.snapshot.Restore(); err != nil {
			all = append(all, common.GenerationError{Language: languageName, Message: err.Error()})
		}
		if l.manifest != nil {
			if err := l.manifest.EmitFailed(all); err != nil {
				all = append(all, common.GenerationError{Language: languageName, Message: err.Error()})
			}
		}
		reportFailure(common.SortErrors(all))
	}
}

// reportFailure is the single failure exit for a generation run.
func reportFailure(all []common.GenerationError) {
	fmt.Fprintln(stderr, "Generation failed:\n"+strings.Join(common.Report(all), "\n"))
	exitProcess(1)
}

func (*dispatchLang) Name() string { return languageName }

func (*dispatchLang) Kinds() map[string]rule.KindInfo { return nil }

func (l *dispatchLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	if args.Config != nil {
		l.configs = append(l.configs, args.Config)
	}
	for _, msg := range checkAndroid(args) {
		l.fail(common.InPackage(args.Rel), "%s", msg)
	}
	if args.Config != nil {
		if err := l.snapshot.RecordDir(args.Dir, args.Config.ValidBuildFileNames); err != nil {
			l.fail(common.InPackage(args.Rel), "%v", err)
		}
	}
	if l.manifest != nil {
		l.manifest.Record(args, language.GenerateResult{})
	}
	return language.GenerateResult{}
}
