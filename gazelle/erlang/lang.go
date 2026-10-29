package erlang

import (
	"io/fs"
	"os"
	"path"
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
const UnresolvedHint = "add an erlang_app that defines that module, a # gazelle:resolve erlang mapping on the module (for example # gazelle:resolve erlang thoas @erlang_app_thoas//:erlang_app) or a dx_ignore_import"

const unnamedHint = "name the handwritten erlang_app, for example name = \"erlang_app\", so dx generate can merge into it"

func ruleKind(mergeable ...string) rule.KindInfo {
	merge := map[string]bool{}
	for _, a := range mergeable {
		merge[a] = true
	}
	return rule.KindInfo{NonEmptyAttrs: map[string]bool{"srcs": true}, MergeableAttrs: merge}
}

func kinds() map[string]rule.KindInfo {
	app := ruleKind("srcs", "hdrs", "deps", "nifs", "extra_apps")
	app.ResolveAttrs = map[string]bool{"deps": true}
	return map[string]rule.KindInfo{
		AppKind: app,
		NifKind: ruleKind("srcs", "hdrs"),
	}
}

type erlangLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &erlangLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    AppKind,
		ExtraKinds:     []string{NifKind},
		Kinds:          kinds(),
		DefsPackage:    "erlang/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ModuleName,
		IsImportSource: IsErlSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: UnresolvedHint,
		Generate:       generateRules,
	})}
}

// Imports skips unnamed rules: the macro names them erlang_app, which the
// rule index cannot see.
func (l *erlangLang) Imports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Name() == "" {
		return nil
	}
	return l.SingleLang.Imports(c, r, f)
}

// listSources returns the files under dir that keep accepts, relative to root.
// Directories that are another Erlang project are skipped.
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
		if d.IsDir() {
			if p != base {
				if _, err := os.Stat(filepath.Join(p, RebarConfig)); err == nil {
					return filepath.SkipDir
				}
				// A subdirectory with its own BUILD file is a separate package
				// that this one cannot list. c_src is exempt: the cc generator
				// removes the stale file it left there.
				if dir != NifDir && hasBuildFile(p) {
					return filepath.SkipDir
				}
			}
			return nil
		}
		rel, err := filepath.Rel(root, p)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		if keep(rel) {
			out = append(out, rel)
		}
		return nil
	})
	sort.Strings(out)
	return out, err
}

func hasBuildFile(dir string) bool {
	for _, n := range []string{"BUILD", "BUILD.bazel"} {
		if _, err := os.Stat(filepath.Join(dir, n)); err == nil {
			return true
		}
	}
	return false
}

// buildOutputDirs are the directories rebar3 fills next to a rebar.config.
var buildOutputDirs = []string{"deps", "_build"}

func inBuildOutput(root, rel string) bool {
	parts := strings.Split(rel, "/")
	for i, part := range parts {
		if rel == "" || !slices.Contains(buildOutputDirs, part) {
			continue
		}
		parent := filepath.Join(append([]string{root}, parts[:i]...)...)
		if _, err := os.Stat(filepath.Join(parent, RebarConfig)); err == nil {
			return true
		}
	}
	return false
}

func appSrcFile(dir string) string {
	matches, _ := filepath.Glob(filepath.Join(dir, SrcDir, "*.app.src"))
	sort.Strings(matches)
	if len(matches) == 0 {
		return ""
	}
	return matches[0]
}

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	appSrcPath := appSrcFile(args.Dir)
	if (appSrcPath == "" && !slices.Contains(args.RegularFiles, RebarConfig)) || inBuildOutput(args.Config.RepoRoot, args.Rel) {
		return language.GenerateResult{}
	}
	fail := func(format string, a ...any) language.GenerateResult {
		rep.Fail(common.InPackage(args.Rel), "erlang: %s: "+format, append([]any{args.Rel}, a...)...)
		return language.GenerateResult{}
	}
	var meta AppSrc
	if appSrcPath != "" {
		content, err := common.ReadSource(appSrcPath)
		if err != nil {
			return fail("read %s: %v", filepath.Base(appSrcPath), err)
		}
		meta = ParseAppSrc(content)
	}
	if meta.Name == "" && appSrcPath != "" {
		meta.Name = strings.TrimSuffix(filepath.Base(appSrcPath), ".app.src")
	}
	if meta.Name == "" {
		meta.Name = filepath.Base(args.Dir)
		if args.Rel == "" && args.Config.RepoName != "" {
			meta.Name = args.Config.RepoName
		}
	}
	erls, err := listSources(args.Dir, SrcDir, func(s string) bool { return IsAppSource(s) })
	if err != nil {
		return fail("%v", err)
	}
	hdrs, err := listSources(args.Dir, IncludeDir, IsAppHeader)
	if err != nil {
		return fail("%v", err)
	}
	nifSrcs, err := listSources(args.Dir, NifDir, IsNifSource)
	if err != nil {
		return fail("%v", err)
	}
	nifHdrs, err := listSources(args.Dir, NifDir, IsNifHeader)
	if err != nil {
		return fail("%v", err)
	}
	// rebar3 compiles src/*.xrl and src/*.yrl into modules, so their names are
	// defined even though this generator does not list the files yet.
	generated, err := listSources(args.Dir, SrcDir, func(s string) bool {
		return isLocal(s) && (strings.HasSuffix(s, ".xrl") || strings.HasSuffix(s, ".yrl"))
	})
	if err != nil {
		return fail("%v", err)
	}
	if !slices.ContainsFunc(erls, IsErlSource) {
		return common.MergeStaleOwned(args.File, language.GenerateResult{}, kinds, Owns)
	}
	name := AppTarget
	if args.File != nil {
		for _, r := range args.File.Rules {
			if r.Kind() != AppKind {
				continue
			}
			if r.Name() == "" {
				return fail("unnamed erlang_app; %s", unnamedHint)
			}
			if r.AttrString("app_name") == meta.Name {
				name = r.Name()
			}
		}
	}
	refs, err := scan(args.Dir, append(append([]string{}, erls...), hdrs...))
	if err != nil {
		return fail("%v", err)
	}
	nifName := ""
	if len(nifSrcs) > 0 {
		nifName = meta.Name + "_nif"
		if len(refs.NifNames) == 1 {
			nifName = refs.NifNames[0]
		}
		if nifName == name {
			return fail("the NIF name %q collides with the erlang_app target; rename the NIF library", nifName)
		}
	}
	claimants := []common.Claimant{{Name: name, Source: args.Rel, Kind: AppKind}}
	if nifName != "" {
		claimants = append(claimants, common.Claimant{Name: nifName, Source: args.Rel, Kind: NifKind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, nil); err != nil {
		return fail("%v; rename the handwritten rule or add # gazelle:exclude for this directory", err)
	}
	defined := make(map[string]bool)
	for _, src := range erls {
		if IsErlSource(src) {
			defined[ModuleName(src)] = true
		}
	}
	for _, src := range generated {
		defined[strings.TrimSuffix(path.Base(src), filepath.Ext(src))] = true
	}
	var imports, extraApps []string
	apps := map[string]bool{}
	for _, m := range refs.Modules {
		if defined[m] {
			continue
		}
		if app, ok := OTPApp(m); ok {
			apps[app] = true
			continue
		}
		imports = append(imports, m)
	}
	for _, a := range refs.Apps {
		if IsOTPApp(a) {
			apps[a] = true
		}
	}
	for a := range apps {
		if !implicitApp(a) {
			extraApps = append(extraApps, a)
		}
	}
	sort.Strings(extraApps)

	var result language.GenerateResult
	r := rule.NewRule(AppKind, name)
	r.SetAttr("srcs", erls)
	if len(hdrs) > 0 {
		r.SetAttr("hdrs", hdrs)
	}
	r.SetAttr("app_name", meta.Name)
	if meta.Version != "" {
		r.SetAttr("app_version", meta.Version)
	}
	if meta.Description != "" {
		r.SetAttr("app_description", meta.Description)
	}
	if meta.Module != "" {
		r.SetAttr("app_module", meta.Module)
	}
	if len(extraApps) > 0 {
		r.SetAttr("extra_apps", extraApps)
	}
	if nifName != "" {
		r.SetAttr("nifs", []string{":" + nifName})
	}
	if common.IsFixturePath(args.Rel) {
		r.SetAttr("testonly", true)
	}
	if nifName != "" {
		n := rule.NewRule(NifKind, nifName)
		n.SetAttr("srcs", nifSrcs)
		if len(nifHdrs) > 0 {
			n.SetAttr("hdrs", nifHdrs)
		}
		result.Gen = append(result.Gen, n)
		result.Imports = append(result.Imports, common.WrapImportSet(nil))
	}
	result.Gen = append(result.Gen, r)
	result.Imports = append(result.Imports, common.WrapImportSet(imports))
	return common.MergeStaleOwned(args.File, result, kinds, Owns)
}

// scan merges the references of every file.
func scan(root string, files []string) (Refs, error) {
	var out Refs
	for _, file := range files {
		content, err := common.ReadSource(filepath.Join(root, file))
		if err != nil {
			return Refs{}, err
		}
		refs := ParseRefs(content)
		out.Modules = append(out.Modules, refs.Modules...)
		out.Apps = append(out.Apps, refs.Apps...)
		out.NifNames = append(out.NifNames, refs.NifNames...)
	}
	for _, list := range []*[]string{&out.Modules, &out.Apps, &out.NifNames} {
		sort.Strings(*list)
		*list = slices.Compact(*list)
	}
	return out, nil
}

