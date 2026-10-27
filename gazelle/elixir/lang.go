package elixir

import (
	"io/fs"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

// UnresolvedHint tells users how to settle a module no rule defines.
const UnresolvedHint = "add an elixir_app that defines that module, a # gazelle:resolve elixir mapping on the module or a prefix of it (for example # gazelle:resolve elixir Jason @jason//:erlang_app)"

const unnamedHint = "name the handwritten elixir_app, for example name = \"erlang_app\", so dx generate can merge into it"

func ruleKind() rule.KindInfo {
	return rule.KindInfo{
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

var elixirKinds = map[string]rule.KindInfo{
	AppKind:  ruleKind(),
	TestKind: ruleKind(),
}

type elixirLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &elixirLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    AppKind,
		ExtraKinds:     []string{TestKind},
		Kinds:          elixirKinds,
		DefsPackage:    "elixir/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: func(src string) bool { return strings.HasSuffix(src, ".ex") },
		QualifiedIDs:   definedModules,
		IsStdLib:       IsStdLib,
		Candidates:     Candidates,
		UnresolvedHint: UnresolvedHint,
		Generate:       generateRules,
	})}
}

func definedModules(path string) []string {
	content, err := common.ReadSource(path)
	if err != nil {
		return nil
	}
	return ParseDefinitions(content)
}

// Imports skips unnamed rules: the macro names them erlang_app, which the
// rule index cannot see.
func (l *elixirLang) Imports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Name() == "" {
		return nil
	}
	return l.SingleLang.Imports(c, r, f)
}

func listSources(root, dir string, keep func(string) bool) ([]string, error) {
	base := filepath.Join(root, dir)
	if _, err := os.Stat(base); err != nil {
		return nil, nil
	}
	var out []string
	err := filepath.WalkDir(base, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() && p != base {
			if _, err := os.Stat(filepath.Join(p, MixFile)); err == nil {
				return filepath.SkipDir
			}
			return nil
		}
		rel, err := filepath.Rel(root, p)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		if !d.IsDir() && keep(rel) {
			out = append(out, rel)
		}
		return nil
	})
	sort.Strings(out)
	return out, err
}

// mixOutputDirs are the directories Mix fills next to a mix.exs: fetched Hex
// checkouts and build output. Their mix.exs files belong to other projects.
var mixOutputDirs = []string{"deps", "_build"}

// inMixOutput reports whether rel sits under deps/ or _build/ of an ancestor
// directory that has a mix.exs. A custom deps_path or build_path is not read;
// exclude it with # gazelle:exclude.
func inMixOutput(root, rel string) bool {
	if rel == "" {
		return false
	}
	parts := strings.Split(rel, "/")
	for i, part := range parts {
		if !slices.Contains(mixOutputDirs, part) {
			continue
		}
		parent := filepath.Join(append([]string{root}, parts[:i]...)...)
		if _, err := os.Stat(filepath.Join(parent, MixFile)); err == nil {
			return true
		}
	}
	return false
}

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	if !slices.Contains(args.RegularFiles, MixFile) || inMixOutput(args.Config.RepoRoot, args.Rel) {
		return language.GenerateResult{}
	}
	fail := func(format string, a ...any) language.GenerateResult {
		rep.Fail("elixir: %s: "+format, append([]any{args.Rel}, a...)...)
		return language.GenerateResult{}
	}
	manifest, err := common.ReadSource(filepath.Join(args.Dir, MixFile))
	if err != nil {
		return fail("read %s: %v", MixFile, err)
	}
	mix := ParseMix(manifest)
	if mix.Umbrella {
		return common.MergeStaleOwned(args.File, language.GenerateResult{}, kinds, Owns)
	}
	lib, err := listSources(args.Dir, LibDir, IsLibSource)
	if err != nil {
		return fail("%v", err)
	}
	tests, err := listSources(args.Dir, TestDir, IsTestSource)
	if err != nil {
		return fail("%v", err)
	}
	if len(lib) == 0 && len(tests) == 0 {
		return common.MergeStaleOwned(args.File, language.GenerateResult{}, kinds, Owns)
	}
	if len(tests) > 0 {
		helper := TestDir + "/" + TestHelper
		if _, err := os.Stat(filepath.Join(args.Dir, helper)); err == nil {
			tests = append(tests, helper)
			sort.Strings(tests)
		}
	}
	if mix.App == "" {
		return fail("%s has no literal app: atom; set app: :name in project/0", MixFile)
	}
	hint := ""
	if args.Rel == "" {
		hint = mix.App
	}
	name, err := common.PackageTargetName(args, hint)
	if err != nil {
		return fail("%v", err)
	}
	testName := name + "_test"
	if args.File != nil {
		var existingTests []*rule.Rule
		for _, r := range args.File.Rules {
			switch r.Kind() {
			case AppKind:
				if r.Name() == "" {
					return fail("unnamed elixir_app; %s", unnamedHint)
				}
				if r.AttrString("app_name") == mix.App {
					name = r.Name()
				}
			case TestKind:
				existingTests = append(existingTests, r)
			}
		}
		if len(existingTests) == 1 && underTestDir(existingTests[0]) {
			testName = existingTests[0].Name()
		}
	}
	var claimants []common.Claimant
	if len(lib) > 0 {
		claimants = append(claimants, common.Claimant{Name: name, Source: args.Rel, Kind: AppKind})
	}
	if len(tests) > 0 {
		claimants = append(claimants, common.Claimant{Name: testName, Source: args.Rel, Kind: TestKind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, nil); err != nil {
		return fail("%v; rename the handwritten rule or add # gazelle:exclude for this directory", err)
	}
	var result language.GenerateResult
	if len(lib) > 0 {
		imports, err := external(args.Dir, lib)
		if err != nil {
			return fail("%v", err)
		}
		r := rule.NewRule(AppKind, name)
		r.SetAttr("srcs", lib)
		r.SetAttr("app_name", mix.App)
		if mix.Version != "" {
			r.SetAttr("app_version", mix.Version)
		}
		if mix.Mod != "" {
			r.SetAttr("app_module", mix.Mod)
		}
		if common.IsFixturePath(args.Rel) {
			r.SetAttr("testonly", true)
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(imports))
	}
	if len(tests) > 0 {
		imports, err := external(args.Dir, tests)
		if err != nil {
			return fail("%v", err)
		}
		r := rule.NewRule(TestKind, testName)
		r.SetAttr("srcs", tests)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(imports))
	}
	return common.MergeStaleOwned(args.File, result, kinds, Owns)
}

func underTestDir(r *rule.Rule) bool {
	srcs := r.AttrStrings("srcs")
	if len(srcs) == 0 {
		return false
	}
	for _, src := range srcs {
		if !isLocal(src) || !strings.HasPrefix(src, TestDir+"/") {
			return false
		}
	}
	return true
}

// external returns the modules files reference that they do not define.
func external(root string, files []string) ([]string, error) {
	defs := make(map[string]bool)
	refs := make(map[string]bool)
	for _, file := range files {
		content, err := common.ReadSource(filepath.Join(root, file))
		if err != nil {
			return nil, err
		}
		info := analyze(content)
		for _, d := range info.defs {
			defs[d] = true
		}
		for _, r := range info.refs {
			refs[r] = true
		}
	}
	out := make([]string, 0, len(refs))
	for name := range refs {
		if !defs[name] {
			out = append(out, name)
		}
	}
	sort.Strings(out)
	return out, nil
}
