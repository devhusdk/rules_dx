package dart

import (
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
// separated). Symlinked files and directories are followed, so a lib that
// links to shared code is still a library. A BUILD file below root is an
// error: one pub package is one Bazel package.
func dartFiles(dir, root string) ([]string, error) {
	var out []string
	if _, err := os.Stat(filepath.Join(dir, root)); err != nil {
		return nil, nil
	}
	err := walkDart(dir, root, map[string]bool{}, &out)
	sort.Strings(out)
	return out, err
}

// walkDart lists the .dart files under rel, a slash separated path below dir.
// active holds the real paths of the directories on the current walk, so a
// symlink back to one of them ends the walk instead of looping.
func walkDart(dir, rel string, active map[string]bool, out *[]string) error {
	p := filepath.Join(dir, filepath.FromSlash(rel))
	target, err := filepath.EvalSymlinks(p)
	if err != nil {
		return err
	}
	if active[target] {
		return nil
	}
	active[target] = true
	defer delete(active, target)
	entries, err := os.ReadDir(p)
	if err != nil {
		return err
	}
	for _, e := range entries {
		name := e.Name()
		child := rel + "/" + name
		info, err := os.Stat(filepath.Join(p, name))
		if err != nil {
			return err
		}
		if info.IsDir() {
			if strings.HasPrefix(name, ".") {
				continue
			}
			if hasBuildFile(filepath.Join(p, name)) {
				return &nestedBuildError{rel: child}
			}
			if err := walkDart(dir, child, active, out); err != nil {
				return err
			}
			continue
		}
		if strings.HasSuffix(name, ".dart") {
			*out = append(*out, child)
		}
	}
	return nil
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

	libKind, testKind := LibraryKind, TestKind
	if spec.Flutter {
		libKind, testKind = FlutterLibraryKind, FlutterTestKind
		bin = nil
	}
	var entries []*entry
	if len(lib) > 0 {
		pkgs, gen := scan(lib, true, false)
		entries = append(entries, &entry{kind: libKind, name: spec.Name, srcs: mergeSorted(lib, gen), imports: pkgs})
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
		entries = append(entries, &entry{kind: testKind, name: name, main: f, srcs: mergeSorted(testSrcs, gen), imports: pkgs})
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	var claims []common.Claimant
	for _, e := range entries {
		claims = append(claims, common.Claimant{Name: e.name, Source: path.Join(args.Rel, e.main), Kind: e.kind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claims); err != nil {
		fail("%v", err)
		return language.GenerateResult{}
	}
	result := language.GenerateResult{}
	for _, e := range entries {
		r := rule.NewRule(e.kind, e.name)
		if e.kind == libKind {
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
