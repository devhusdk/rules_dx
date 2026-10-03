package rust

import (
	"fmt"
	"path"
	"sort"
	"strings"

	"github.com/ralvik/rules_dx/gazelle/common"
)

type CrateRoots struct {
	Dir       string
	LibRoot   string
	BinRoot   string
	TestRoots []string
}

func DiscoverCrateRoots(dir string, files []string) CrateRoots {
	roots := CrateRoots{Dir: dir}
	join := func(elem ...string) string {
		rel := path.Join(elem...)
		if dir == "" || dir == "." {
			return rel
		}
		return dir + "/" + rel
	}
	lib, bin := join("src", "lib.rs"), join("src", "main.rs")
	testPrefix := join("tests") + "/"
	for _, f := range files {
		switch {
		case f == lib:
			roots.LibRoot = f
		case f == bin:
			roots.BinRoot = f
		case strings.HasPrefix(f, testPrefix):
			rest := strings.TrimPrefix(f, testPrefix)
			if rest != "" && !strings.Contains(rest, "/") && strings.HasSuffix(rest, ".rs") {
				roots.TestRoots = append(roots.TestRoots, f)
			}
		}
	}
	sort.Strings(roots.TestRoots)
	return roots
}

func (r CrateRoots) HasRoots() bool {
	return r.LibRoot != "" || r.BinRoot != "" || len(r.TestRoots) > 0
}

type TestTarget struct {
	Name string
	Root string
}

type CrateShape struct {
	Dir         string
	Name        string
	LibTarget   string
	BinTarget   string
	Tests       []TestTarget
	LibUnitTest bool
	BinUnitTest bool
}

func ShapeCrate(roots CrateRoots, libUnitTest, binUnitTest bool) (*CrateShape, error) {
	if !roots.HasRoots() {
		return &CrateShape{Dir: roots.Dir}, nil
	}
	base := roots.Dir
	if base == "" || base == "." {
		return nil, fmt.Errorf("rust: crate directory %q has no source-only fallback name; an authoritative Cargo package name is required", roots.Dir)
	}
	name, err := common.Normalize(path.Base(base))
	if err != nil {
		return nil, fmt.Errorf("rust: crate directory %q: %w", roots.Dir, err)
	}
	shape := &CrateShape{Dir: roots.Dir, Name: name}
	var claimants []common.Claimant
	claim := func(target, source string) {
		claimants = append(claimants, common.Claimant{Name: target, Source: source})
	}
	if roots.LibRoot != "" {
		shape.LibTarget = name
		claim(name, roots.LibRoot)
	}
	if roots.BinRoot != "" {
		if roots.LibRoot != "" {
			shape.BinTarget = common.BinaryName(name)
		} else {
			shape.BinTarget = name
		}
		claim(shape.BinTarget, roots.BinRoot)
	}
	for _, root := range roots.TestRoots {
		stem, err := common.Normalize(strings.TrimSuffix(path.Base(root), ".rs"))
		if err != nil {
			return nil, fmt.Errorf("rust: integration-test root %s: %w", root, err)
		}
		target := IntegrationTestName(stem)
		shape.Tests = append(shape.Tests, TestTarget{Name: target, Root: root})
		claim(target, root)
	}
	if libUnitTest && roots.LibRoot != "" {
		target := common.UnitTestName(shape.LibTarget)
		shape.LibUnitTest = true
		claim(target, roots.LibRoot+" [unit]")
	}
	if binUnitTest && roots.BinRoot != "" {
		target := common.UnitTestName(shape.BinTarget)
		shape.BinUnitTest = true
		claim(target, roots.BinRoot+" [unit]")
	}
	if err := common.CheckCollisions(claimants); err != nil {
		return nil, err
	}
	return shape, nil
}

func LoadCrate(root string, read func(string) ([]byte, error), exists func(string) bool) (map[string]*FileFacts, error) {
	owned := make(map[string]*FileFacts)
	var visit func(file, moduleDir string, inheritedTest bool, stack []string) error
	visit = func(file, moduleDir string, inheritedTest bool, stack []string) error {
		for _, s := range stack {
			if s == file {
				return fmt.Errorf("rust: %s: module cycle through %s", root, strings.Join(append(stack, file), " -> "))
			}
		}
		if _, dup := owned[file]; dup {
			return fmt.Errorf("rust: %s: duplicate module inclusion of %s", root, file)
		}
		content, err := read(file)
		if err != nil {
			return fmt.Errorf("rust: %s: cannot read owned module %s: %v", root, file, err)
		}
		facts, err := ParseFacts(file, content)
		if err != nil {
			return err
		}
		if inheritedTest || facts.InnerCfgTest {
			facts.HasCfgTest = true
			for i := range facts.Modules {
				facts.Modules[i].CfgTest = true
			}
			for i := range facts.Uses {
				facts.Uses[i].CfgTest = true
			}
			for i := range facts.Externs {
				facts.Externs[i].CfgTest = true
			}
		}
		owned[file] = facts
		for _, m := range facts.Modules {
			if m.Inline {
				continue
			}
			var candidates []string
			if m.Path != "" {
				candidates = []string{joinRel(path.Dir(file), m.Path)}
			} else {
				candidates = []string{
					joinRel(moduleDir, m.Name+".rs"),
					joinRel(moduleDir, m.Name+"/mod.rs"),
				}
			}
			var found []string
			for _, c := range candidates {
				if exists(c) {
					found = append(found, c)
				}
			}
			switch len(found) {
			case 0:
				return fmt.Errorf("rust: %s: orphan module `%s` declared in %s: no %s; add the source or remove the declaration",
					root, m.Name, file, strings.Join(candidates, " or "))
			case 1:
				childDir := path.Dir(found[0])
				if path.Base(found[0]) != "mod.rs" {
					childDir = strings.TrimSuffix(found[0], ".rs")
				}
				if err := visit(found[0], childDir, inheritedTest || m.CfgTest, append(stack, file)); err != nil {
					return err
				}
			default:
				return fmt.Errorf("rust: %s: ambiguous module `%s` declared in %s: both %s exist; keep exactly one",
					root, m.Name, file, strings.Join(found, " and "))
			}
		}
		return nil
	}
	if err := visit(root, path.Dir(root), false, nil); err != nil {
		return nil, err
	}
	return owned, nil
}

func joinRel(fileDir, rel string) string {
	if strings.HasPrefix(rel, "/") {
		return strings.TrimPrefix(path.Clean(rel), "/")
	}
	if fileDir == "" || fileDir == "." {
		return path.Clean(rel)
	}
	return path.Clean(fileDir + "/" + rel)
}
