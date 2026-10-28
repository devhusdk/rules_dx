package dart

import (
	"io/fs"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func hasBuildFile(dir string) bool {
	for _, n := range []string{"BUILD", "BUILD.bazel"} {
		if _, err := os.Stat(filepath.Join(dir, n)); err == nil {
			return true
		}
	}
	return false
}

// dartFiles lists the .dart files under root (relative to dir, slash
// separated). A BUILD file below root is an error: one pub package is one
// Bazel package.
func dartFiles(dir, root string) ([]string, error) {
	var out []string
	base := filepath.Join(dir, root)
	if _, err := os.Stat(base); err != nil {
		return nil, nil
	}
	err := filepath.WalkDir(base, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		rel, _ := filepath.Rel(dir, p)
		rel = filepath.ToSlash(rel)
		if d.IsDir() {
			if p != base && strings.HasPrefix(d.Name(), ".") {
				return fs.SkipDir
			}
			if p != base && hasBuildFile(p) {
				return &nestedBuildError{rel: rel}
			}
			return nil
		}
		if strings.HasSuffix(d.Name(), ".dart") {
			out = append(out, rel)
		}
		return nil
	})
	sort.Strings(out)
	return out, err
}

type nestedBuildError struct{ rel string }

func (e *nestedBuildError) Error() string {
	return e.rel + " has its own BUILD file; a pub package is one Bazel package, so remove it or add # gazelle:exclude " + e.rel
}

func isTestFile(file string) bool { return strings.HasSuffix(file, "_test.dart") }

func genruleOutputs(f *rule.File) map[string]bool {
	outs := map[string]bool{}
	if f == nil {
		return outs
	}
	for _, r := range f.Rules {
		if r.Kind() == "genrule" {
			for _, o := range r.AttrStrings("outs") {
				outs[o] = true
			}
		}
	}
	return outs
}

type entry struct {
	kind    string
	name    string
	main    string
	srcs    []string
	imports []string
	rule    *rule.Rule
}

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	fail := func(format string, a ...any) {
		rep.Fail(common.InPackage(args.Rel), "dart: "+args.Rel+": "+format, a...)
	}
	hasPubspec := false
	for _, f := range args.RegularFiles {
		if f == "pubspec.yaml" {
			hasPubspec = true
		}
	}
	if !hasPubspec {
		// Without a pubspec nothing is generated; rules whose files are all
		// gone are swept, and handwritten rules stay.
		return common.MergeStaleOwned(args.File, language.GenerateResult{}, kinds, func(r *rule.Rule) bool {
			refs := append(r.AttrStrings("srcs"), r.AttrString("main"))
			seen := false
			for _, ref := range refs {
				if ref == "" {
					continue
				}
				seen = true
				if _, err := os.Stat(filepath.Join(args.Dir, filepath.FromSlash(ref))); err == nil {
					return false
				}
			}
			return seen
		})
	}
	content, err := common.ReadSource(filepath.Join(args.Dir, "pubspec.yaml"))
	if err != nil {
		fail("read pubspec.yaml: %v", err)
		return language.GenerateResult{}
	}
	spec, err := ParsePubspec(content)
	if err != nil {
		fail("pubspec.yaml: %v", err)
		return language.GenerateResult{}
	}
	var lib, bin, test []string
	for _, group := range []struct {
		root string
		out  *[]string
	}{{"lib", &lib}, {"bin", &bin}, {"test", &test}} {
		files, err := dartFiles(args.Dir, group.root)
		if err != nil {
			fail("%v", err)
			return language.GenerateResult{}
		}
		*group.out = files
	}
	onDisk := map[string]bool{}
	for _, list := range [][]string{lib, bin, test} {
		for _, f := range list {
			onDisk[f] = true
		}
	}
	genOuts := genruleOutputs(args.File)

	// scan reads the directives of files and returns the pub packages they
	// use plus any generated files they pull in.
	scan := func(files []string, isLib, isTest bool) (pkgs, generated []string) {
		seenPkg := map[string]bool{}
		seenGen := map[string]bool{}
		for _, file := range files {
			src, err := common.ReadSource(filepath.Join(args.Dir, filepath.FromSlash(file)))
			if err != nil {
				fail("read %s: %v", file, err)
				continue
			}
			dirs, err := ParseDirectives(src)
			if err != nil {
				fail("%s: %v", file, err)
				continue
			}
			for _, d := range dirs {
				switch {
				case strings.HasPrefix(d.URI, "dart:"):
				case strings.HasPrefix(d.URI, "package:"):
					name, _, _ := strings.Cut(strings.TrimPrefix(d.URI, "package:"), "/")
					if name == "" {
						fail("%s: malformed URI %q", file, d.URI)
						continue
					}
					if name == spec.Name {
						if !isLib && !seenPkg[name] {
							seenPkg[name] = true
							pkgs = append(pkgs, name)
						}
						continue
					}
					if !spec.Deps[name] && !(isTest && spec.DevDeps[name]) {
						where := "dependencies"
						if isTest {
							where = "dependencies or dev_dependencies"
						}
						fail("%s imports package:%s, which pubspec.yaml does not declare; add it to %s", file, name, where)
						continue
					}
					if !seenPkg[name] {
						seenPkg[name] = true
						pkgs = append(pkgs, name)
					}
				case strings.Contains(d.URI, ":"):
					fail("%s: unsupported URI %q; only dart:, package: and relative URIs are supported", file, d.URI)
				default:
					target := path.Join(path.Dir(file), d.URI)
					switch {
					case strings.HasPrefix(target, "../") || target == "..":
						fail("%s: %s %q leaves the package", file, d.Kind, d.URI)
					case onDisk[target]:
					case genOuts[target]:
						if !seenGen[target] {
							seenGen[target] = true
							generated = append(generated, target)
						}
					default:
						if _, err := os.Stat(filepath.Join(args.Dir, filepath.FromSlash(target))); err == nil {
							continue
						}
						if d.Kind == "part" {
							fail("%s: part %q is not produced by any rule in this BUILD file; hand-write the generator rule", file, d.URI)
						} else {
							fail("%s: %s %q does not exist", file, d.Kind, d.URI)
						}
					}
				}
			}
		}
		sort.Strings(pkgs)
		sort.Strings(generated)
		return pkgs, generated
	}

	var entries []*entry
	if len(lib) > 0 {
		pkgs, gen := scan(lib, true, false)
		entries = append(entries, &entry{kind: LibraryKind, name: spec.Name, srcs: mergeSorted(lib, gen), imports: pkgs})
	}
	var binSrcs, testSrcs []string
	for _, f := range bin {
		if path.Dir(f) != "bin" {
			binSrcs = append(binSrcs, f)
		}
	}
	for _, f := range test {
		if !isTestFile(f) {
			testSrcs = append(testSrcs, f)
		}
	}
	for _, f := range bin {
		if path.Dir(f) != "bin" {
			continue
		}
		name := strings.TrimSuffix(path.Base(f), ".dart")
		if len(lib) > 0 && name == spec.Name {
			name += "_bin"
		}
		files := append([]string{f}, binSrcs...)
		pkgs, gen := scan(files, false, false)
		entries = append(entries, &entry{kind: BinaryKind, name: name, main: f, srcs: mergeSorted(binSrcs, gen), imports: pkgs})
	}
	for _, f := range test {
		if !isTestFile(f) {
			continue
		}
		name := strings.ReplaceAll(strings.TrimSuffix(strings.TrimPrefix(f, "test/"), ".dart"), "/", "_")
		files := append([]string{f}, testSrcs...)
		pkgs, gen := scan(files, false, true)
		entries = append(entries, &entry{kind: TestKind, name: name, main: f, srcs: mergeSorted(testSrcs, gen), imports: pkgs})
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	var claims []common.Claimant
	for _, e := range entries {
		claims = append(claims, common.Claimant{Name: e.name, Source: path.Join(args.Rel, e.main), Kind: e.kind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claims, nil); err != nil {
		fail("%v", err)
		return language.GenerateResult{}
	}
	result := language.GenerateResult{}
	for _, e := range entries {
		r := rule.NewRule(e.kind, e.name)
		if e.kind == LibraryKind {
			r.SetAttr("package_name", spec.Name)
		} else {
			r.SetAttr("main", e.main)
		}
		if len(e.srcs) > 0 {
			r.SetAttr("srcs", e.srcs)
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(e.imports))
	}
	return common.MergeStaleOwned(args.File, result, kinds, nil)
}

func mergeSorted(a, b []string) []string {
	out := append(append([]string(nil), a...), b...)
	sort.Strings(out)
	return out
}
