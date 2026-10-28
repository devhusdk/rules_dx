package julia

import (
	"fmt"
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

// UnresolvedHint tells users how to settle a dependency no rule defines.
const UnresolvedHint = "add a julia_library named after the package, or an exact # gazelle:resolve julia <Name> <label> (for example @julia_deps//:<Name>)"

var juliaKinds = map[string]rule.KindInfo{
	LibraryKind: {
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
	TestKind: {
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"main": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type juliaLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &juliaLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		ExtraKinds:     []string{TestKind},
		Kinds:          juliaKinds,
		DefsPackage:    "julia/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsStdLib:       IsStdLib,
		UnresolvedHint: UnresolvedHint,
		Generate:       generateRules,
	})}
}

// Imports indexes a julia_library under its rule name: rules_julia finds the
// entry point and precompiles only when the name matches src/<name>.jl.
func (l *juliaLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind || r.Name() == "" {
		return nil
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: r.Name()}}
}

// listSources returns the .jl files under root/dir, sorted. A directory with
// its own Project.toml is another package and is skipped; a directory with a
// BUILD file is an error because a label cannot cross into it.
func listSources(root, dir string) ([]string, error) {
	base := filepath.Join(root, dir)
	if _, err := os.Stat(base); err != nil {
		return nil, nil
	}
	var out []string
	err := filepath.WalkDir(base, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(root, p)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		if d.IsDir() {
			if p == base {
				return nil
			}
			if _, err := os.Stat(filepath.Join(p, ProjectFile)); err == nil {
				return filepath.SkipDir
			}
			for _, name := range []string{"BUILD", "BUILD.bazel"} {
				if _, err := os.Stat(filepath.Join(p, name)); err == nil {
					return fmt.Errorf("%s has its own %s, so its files cannot belong to this package; remove it or move the files", rel, name)
				}
			}
			return nil
		}
		if strings.HasSuffix(rel, ".jl") {
			out = append(out, rel)
		}
		return nil
	})
	sort.Strings(out)
	return out, err
}

// checkUUIDs rejects a dependency that has a standard library name but not
// its UUID: it is a registered package, which a stdlib skip would drop.
func checkUUIDs(path string, deps map[string]string) error {
	for _, name := range sortedKeys(deps) {
		if want, ok := StdlibUUID(name); ok && strings.ToLower(deps[name]) != want {
			return fmt.Errorf("%s: %s has uuid %s, but the standard library %s has %s; a dependency must not reuse the name of a standard library with another UUID", path, name, deps[name], name, want)
		}
	}
	return nil
}

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	if !slices.Contains(args.RegularFiles, ProjectFile) {
		return language.GenerateResult{}
	}
	fail := func(format string, a ...any) language.GenerateResult {
		rep.Fail(common.InPackage(args.Rel), "julia: %s: "+format, append([]any{args.Rel}, a...)...)
		return language.GenerateResult{}
	}
	projectPath := path(args.Rel, ProjectFile)
	content, err := common.ReadSource(filepath.Join(args.Dir, ProjectFile))
	if err != nil {
		return fail("read %s: %v", ProjectFile, err)
	}
	project, err := ParseProject(projectPath, content)
	if err != nil {
		return fail("%v", err)
	}
	if project.Name == "" {
		return language.GenerateResult{}
	}
	srcs, err := listSources(args.Dir, SrcDir)
	if err != nil {
		return fail("%v", err)
	}
	if len(srcs) == 0 {
		return common.MergeStaleOwned(args.File, language.GenerateResult{}, kinds, Owns)
	}
	entry := SrcDir + "/" + project.Name + ".jl"
	if !slices.Contains(srcs, entry) {
		return fail("%s has no %s, which the package %s needs as its entry file", ProjectFile, entry, project.Name)
	}
	tests, err := listSources(args.Dir, TestDir)
	if err != nil {
		return fail("%v", err)
	}
	if !slices.Contains(tests, TestEntry) {
		tests = nil
	}
	var override *Project
	if tests != nil {
		if raw, err := common.ReadSource(filepath.Join(args.Dir, TestDir, ProjectFile)); err == nil {
			p, err := ParseProject(path(args.Rel, TestDir, ProjectFile), raw)
			if err != nil {
				return fail("%v", err)
			}
			override = &p
		}
	}
	testDeps := project.TestDeps(override)
	testPath := projectPath
	if override != nil {
		testPath = path(args.Rel, TestDir, ProjectFile)
	}
	if err := checkUUIDs(projectPath, project.Deps); err != nil {
		return fail("%v", err)
	}
	if tests != nil {
		if err := checkUUIDs(testPath, testDeps); err != nil {
			return fail("%v", err)
		}
	}
	name := project.Name
	testName := name + "_test"
	claimants := []common.Claimant{{Name: name, Source: args.Rel, Kind: LibraryKind}}
	if tests != nil {
		claimants = append(claimants, common.Claimant{Name: testName, Source: args.Rel, Kind: TestKind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, nil); err != nil {
		return fail("%v; rename the handwritten rule or add # gazelle:exclude for this directory", err)
	}
	var result language.GenerateResult
	lib := rule.NewRule(LibraryKind, name)
	lib.SetAttr("srcs", srcs)
	lib.SetAttr("data", []string{ProjectFile})
	if common.IsFixturePath(args.Rel) {
		lib.SetAttr("testonly", true)
	}
	result.Gen = append(result.Gen, lib)
	result.Imports = append(result.Imports, common.WrapImportSet(sortedKeys(project.Deps)))
	if tests != nil {
		test := rule.NewRule(TestKind, testName)
		test.SetAttr("srcs", tests)
		if len(tests) > 1 {
			test.SetAttr("main", TestEntry)
		}
		result.Gen = append(result.Gen, test)
		result.Imports = append(result.Imports, common.WrapImportSet(append([]string{name}, sortedKeys(testDeps)...)))
	}
	return common.MergeStaleOwned(args.File, result, kinds, Owns)
}

func path(rel string, parts ...string) string {
	return strings.TrimPrefix(rel+"/"+strings.Join(parts, "/"), "/")
}
