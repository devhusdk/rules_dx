package d

import (
	"fmt"
	"path"
	"regexp"
	"sort"
	"strings"
)

// LibraryKind is the rule kind generated for a directory of D modules.
const LibraryKind = "d_library"

// SupportedExts are the D source extensions: implementation and interface.
var SupportedExts = []string{".d", ".di"}

var identifier = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]*$`)

func stem(file string) string {
	return strings.TrimSuffix(strings.TrimSuffix(file, ".di"), ".d")
}

// ModuleOf returns the module a file defines: its declaration, or the file
// stem when it has none. package.d without a declaration is the directory.
func ModuleOf(pkgRel, file, declared string) (string, error) {
	if declared != "" {
		return declared, nil
	}
	s := stem(file)
	if s == "package" {
		if pkgRel == "" {
			return "", fmt.Errorf("%s declares no module and sits at the repo root; add a module declaration", file)
		}
		s = path.Base(pkgRel)
	}
	if !identifier.MatchString(s) {
		return "", fmt.Errorf("%s declares no module and its name %q is not a D identifier; add a module declaration or rename it", file, s)
	}
	return s, nil
}

// ImportRoot returns the package-relative import root of file: the directory
// the compiler searches so that module resolves to the file. It is "." when
// the package directory itself is the root. A declared module must be a
// segment-aligned suffix of the file path.
func ImportRoot(pkgRel, file, declared string) (string, error) {
	module, err := ModuleOf(pkgRel, file, declared)
	if err != nil {
		return "", err
	}
	segs := strings.Split(module, ".")
	dirSegs := []string{}
	if pkgRel != "" {
		dirSegs = strings.Split(pkgRel, "/")
	}
	up := len(segs) - 1
	if stem(file) == "package" {
		up = len(segs)
	} else if segs[len(segs)-1] != stem(file) {
		return "", mismatch(module, pkgRel, file)
	}
	if up > len(dirSegs) {
		return "", mismatch(module, pkgRel, file)
	}
	pkgSegs := segs[:len(segs)-1]
	if stem(file) == "package" {
		pkgSegs = segs
	}
	for i, seg := range pkgSegs {
		if dirSegs[len(dirSegs)-len(pkgSegs)+i] != seg {
			return "", mismatch(module, pkgRel, file)
		}
	}
	if up == 0 {
		return ".", nil
	}
	return strings.TrimSuffix(strings.Repeat("../", up), "/"), nil
}

func mismatch(module, pkgRel, file string) error {
	return fmt.Errorf("module %s in %s does not match its path; move the file or fix the module declaration", module, path.Join(pkgRel, file))
}

// PackageRoot returns the one import root shared by every file of a package.
// declared maps each file to its declared module ("" when it has none). Two
// files defining one module are an error: a .d file and its .di interface
// cannot both be sources of one library.
func PackageRoot(pkgRel string, declared map[string]string) (string, error) {
	files := make([]string, 0, len(declared))
	for f := range declared {
		files = append(files, f)
	}
	sort.Strings(files)
	root, rootFile := "", ""
	owners := make(map[string]string)
	for _, f := range files {
		module, err := ModuleOf(pkgRel, f, declared[f])
		if err != nil {
			return "", err
		}
		if other, ok := owners[module]; ok {
			return "", fmt.Errorf("%s and %s both define module %s; keep one of them", other, f, module)
		}
		owners[module] = f
		r, err := ImportRoot(pkgRel, f, declared[f])
		if err != nil {
			return "", err
		}
		if rootFile == "" {
			root, rootFile = r, f
			continue
		}
		if r != root {
			return "", fmt.Errorf("%s needs import root %q but %s needs %q; split the directory so one library has one import root", f, r, rootFile, root)
		}
	}
	return root, nil
}
