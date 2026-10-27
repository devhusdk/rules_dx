package common

import (
	"path"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"golang.org/x/mod/modfile"
)

// goModManifest holds the module paths a go.mod requires.
type goModManifest struct {
	direct   []string
	indirect []string
}

func parseGoModManifest(content []byte, file string) (externalManifest, error) {
	mf, err := modfile.Parse(file, content, nil)
	if err != nil {
		return nil, err
	}
	local := map[string]bool{}
	for _, r := range mf.Replace {
		if r.New.Version == "" && modfile.IsDirectoryPath(r.New.Path) {
			local[r.Old.Path] = true
		}
	}
	m := &goModManifest{}
	for _, r := range mf.Require {
		if local[r.Mod.Path] {
			continue
		}
		if r.Indirect {
			m.indirect = append(m.indirect, r.Mod.Path)
		} else {
			m.direct = append(m.direct, r.Mod.Path)
		}
	}
	return m, nil
}

func longestModule(mods []string, imp string) string {
	best := ""
	for _, mod := range mods {
		if (imp == mod || strings.HasPrefix(imp, mod+"/")) && len(mod) > len(best) {
			best = mod
		}
	}
	return best
}

func (m *goModManifest) Lookup(_, imp string) ([]label.Label, string) {
	mod := longestModule(m.direct, imp)
	if mod == "" {
		if ind := longestModule(m.indirect, imp); ind != "" {
			return nil, "module " + ind + " is only an indirect require; require it directly and run go mod tidy"
		}
		return nil, ""
	}
	pkg := strings.TrimPrefix(strings.TrimPrefix(imp, mod), "/")
	return []label.Label{label.New(label.ImportPathToBazelRepoName(mod), pkg, goLibName(imp))}, ""
}

// goLibName is the go_library name Gazelle's import naming convention gives
// an import path.
func goLibName(imp string) string {
	name := path.Base(imp)
	if isMajorVersion(name) {
		if dir := path.Dir(imp); dir != "." {
			name = path.Base(dir)
		}
	}
	return strings.ReplaceAll(name, ".", "_")
}

func isMajorVersion(s string) bool {
	if len(s) < 2 || s[0] != 'v' {
		return false
	}
	for _, r := range s[1:] {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}
